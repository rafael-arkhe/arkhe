//! Ledger distribuído IPFS com validação cruzada de Zeno Veto (I194/I201).
//!
//! Este módulo não depende de uma lib IPFS externa: o cliente é definido por
//! [`IpfsBackend`] (trate de implementar para o seu nó — ex.: `ipfs-api`).
//! O fluxo completo (publica → puxa → valida cruzadamente → cache) é
//! implementado e testado com um backend in-memory.

use crate::hardware::undulator::RangingDelta;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// Backend IPFS abstrato.
pub trait IpfsBackend {
    type Error: std::error::Error + 'static;
    fn add(&self, data: &[u8]) -> Result<String, Self::Error>;
    fn cat(&self, cid: &str) -> Result<Vec<u8>, Self::Error>;
}

/// Bloco de handover sincronizado via IPFS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncHandover {
    pub phi: f64,
    pub source: String,
    pub timestamp: u64,
    pub node_id: String,
    pub ranging_delta_us: u64,
    pub zeno_passed: bool,
    pub decay_rate: f64,
    pub cross_validated: bool,
}

impl SyncHandover {
    pub fn new(
        phi: f64,
        source: impl Into<String>,
        timestamp: u64,
        node_id: impl Into<String>,
        ranging_delta_us: u64,
        decay_rate: f64,
    ) -> Self {
        let zeno_passed = true;
        Self {
            phi,
            source: source.into(),
            timestamp,
            node_id: node_id.into(),
            ranging_delta_us,
            zeno_passed,
            decay_rate,
            cross_validated: false,
        }
    }
}

/// Verificador de Zeno Veto para validação cruzada entre nós (I194).
#[derive(Debug, Clone, Copy)]
pub struct ZenoVerifier {
    /// Tolerância de divergência entre ranging dos nós.
    pub tolerance: f64,
}

impl Default for ZenoVerifier {
    fn default() -> Self {
        Self { tolerance: 0.9 }
    }
}

impl ZenoVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validação cruzada: ranging compatível e Φ ∈ [0,1].
    pub fn cross_validate(
        &self,
        phi: f64,
        remote_ranging: u64,
        local_ranging: u64,
    ) -> bool {
        if remote_ranging == 0 || local_ranging == 0 {
            return false;
        }
        let ratio = remote_ranging as f64 / local_ranging as f64;
        if ratio < self.tolerance || ratio > 1.0 / self.tolerance {
            return false;
        }
        (0.0..=1.0).contains(&phi)
    }
}

/// Backend IPFS in-memory para testes/CI.
#[derive(Debug, Default)]
pub struct InMemoryIpfs {
    store: Mutex<HashMap<String, Vec<u8>>>,
    next: Mutex<u64>,
}

impl InMemoryIpfs {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IpfsBackend for InMemoryIpfs {
    type Error = std::io::Error;

    fn add(&self, data: &[u8]) -> Result<String, Self::Error> {
        let mut n = self.store.lock().unwrap();
        let mut seq = self.next.lock().unwrap();
        *seq += 1;
        let cid = format!("QmInMemory{}", seq);
        n.insert(cid.clone(), data.to_vec());
        Ok(cid)
    }

    fn cat(&self, cid: &str) -> Result<Vec<u8>, Self::Error> {
        self.store
            .lock()
            .unwrap()
            .get(cid)
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "cid not found"))
    }
}

/// Ledger distribuído com cache local e validação cruzada.
pub struct DistributedLedger<B: IpfsBackend> {
    backend: B,
    node_id: String,
    local_ranging: RangingDelta,
    cache: Mutex<HashMap<String, Vec<SyncHandover>>>,
    verifier: ZenoVerifier,
}

impl<B: IpfsBackend> DistributedLedger<B> {
    pub fn new(backend: B, node_id: impl Into<String>, local_ranging: RangingDelta) -> Self {
        Self {
            backend,
            node_id: node_id.into(),
            local_ranging,
            cache: Mutex::new(HashMap::new()),
            verifier: ZenoVerifier::new(),
        }
    }

    pub fn with_verifier(mut self, verifier: ZenoVerifier) -> Self {
        self.verifier = verifier;
        self
    }

    /// Publica um handover no IPFS após validação Zeno local (I194).
    pub fn publish(
        &self,
        mut handover: SyncHandover,
    ) -> Result<String, Box<dyn std::error::Error>> {
        if !handover.zeno_passed {
            return Err("Zeno Veto violado localmente".into());
        }
        handover.cross_validated = self.verifier.cross_validate(
            handover.phi,
            handover.ranging_delta_us,
            self.local_ranging.as_micros(),
        );
        let data = serde_json::to_vec(&handover)?;
        let cid = self.backend.add(&data)?;
        let mut cache = self.cache.lock().unwrap();
        cache
            .entry(handover.node_id.clone())
            .or_default()
            .push(handover);
        Ok(cid)
    }

    /// Puxa um handover por CID e valida cruzadamente (I201).
    pub fn pull_and_validate(&self, cid: &str) -> Result<SyncHandover, Box<dyn std::error::Error>> {
        let data = self.backend.cat(cid)?;
        let mut handover: SyncHandover = serde_json::from_slice(&data)?;

        let ok = self.verifier.cross_validate(
            handover.phi,
            handover.ranging_delta_us,
            self.local_ranging.as_micros(),
        );
        if !ok {
            return Err("Falha na validação cruzada de Zeno".into());
        }
        handover.cross_validated = true;

        let mut cache = self.cache.lock().unwrap();
        cache
            .entry(handover.node_id.clone())
            .or_default()
            .push(handover.clone());

        Ok(handover)
    }

    /// Handovers em cache por nó remoto.
    pub fn cache_for(&self, node_id: &str) -> Vec<SyncHandover> {
        self.cache.lock().unwrap().get(node_id).cloned().unwrap_or_default()
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_and_pull_roundtrip() {
        let ipfs = InMemoryIpfs::new();
        let ledger = DistributedLedger::new(ipfs, "LZ", RangingDelta(1000));

        let h = SyncHandover::new(0.95, "CatedralOS", 1_700_000_000, "LZ", 1000, 0.001);
        let cid = ledger.publish(h).unwrap();
        let got = ledger.pull_and_validate(&cid).unwrap();
        assert!(got.cross_validated);
        assert!((got.phi - 0.95).abs() < 1e-12);
    }

    #[test]
    fn test_cross_validation_rejects_incompatible_ranging() {
        let ipfs = InMemoryIpfs::new();
        let ledger = DistributedLedger::new(ipfs, "LZ", RangingDelta(1000));

        // A node with wildly different ranging.
        let h = SyncHandover::new(0.9, "Astra", 1_700_000_000, "Astra", 100_000, 0.001);
        let cid = ledger.publish(h).unwrap();
        let err = ledger.pull_and_validate(&cid).unwrap_err();
        assert!(err.to_string().contains("validação cruzada"));
    }

    #[test]
    fn test_zeno_veto_blocks_publish() {
        let ipfs = InMemoryIpfs::new();
        let ledger = DistributedLedger::new(ipfs, "LZ", RangingDelta(1000));
        let mut h = SyncHandover::new(0.9, "x", 1, "n", 1000, 0.001);
        h.zeno_passed = false;
        assert!(ledger.publish(h).is_err());
    }

    #[test]
    fn test_cache_accumulates() {
        let ipfs = InMemoryIpfs::new();
        let ledger = DistributedLedger::new(ipfs, "LZ", RangingDelta(1000));
        let cid1 = ledger
            .publish(SyncHandover::new(0.9, "a", 1, "Astra", 1000, 0.001))
            .unwrap();
        let cid2 = ledger
            .publish(SyncHandover::new(0.8, "b", 2, "Astra", 1000, 0.001))
            .unwrap();
        let _ = cid1;
        let _ = cid2;
        assert_eq!(ledger.cache_for("Astra").len(), 2);
    }
}