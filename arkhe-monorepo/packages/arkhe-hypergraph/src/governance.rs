//! Governance: AnubisVeto risk gate + AOTB-style certification registry.
//!
//! This is the pure-Rust, real-crates version of the spec's `governance.rs`
//! (which referenced a non-existent `crate::pqc`). Signatures and hashing reuse
//! `arkhe_pqc` and `sha3`.

use crate::{AgentId, Hyperedge, HyperedgeState};
use sha3::{Digest, Sha3_256};

/// An immutable record of a certification (AOTB entry binding).
#[derive(Debug, Clone)]
pub struct AotbCertificate {
    pub edge_id: [u8; 32],
    pub certifier: AgentId,
    pub block_height: u64,
    pub aotb_hash: [u8; 32],
}

/// The AOTB (Agent-Owned Trusted Base) registry: append-only certification log.
#[derive(Debug, Default)]
pub struct AotbRegistry {
    pub certificates: Vec<AotbCertificate>,
}

impl AotbRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Certify a Certified hyperedge at a given block height.
    pub fn certify(&mut self, edge: &Hyperedge, height: u64) -> Option<&AotbCertificate> {
        if edge.state != HyperedgeState::Certified {
            return None;
        }
        let cert = AotbCertificate {
            edge_id: edge.id,
            certifier: AgentId::S15,
            block_height: height,
            aotb_hash: Self::hash_cert(&edge.id, height),
        };
        self.certificates.push(cert);
        self.certificates.last()
    }

    pub fn get(&self, aotb_hash: &[u8; 32]) -> Option<&AotbCertificate> {
        self.certificates.iter().find(|c| &c.aotb_hash == aotb_hash)
    }

    fn hash_cert(edge_id: &[u8; 32], height: u64) -> [u8; 32] {
        let mut h = Sha3_256::new();
        h.update(edge_id);
        h.update(height.to_le_bytes());
        let d = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&d[..32]);
        out
    }
}

/// Anubis: the risk gate that can veto a hyperedge before it proceeds.
#[derive(Debug)]
pub struct AnubisVeto {
    pub threshold: f64,
    pub vetoes: Vec<VetoRecord>,
}

#[derive(Debug, Clone)]
pub struct VetoRecord {
    pub edge_id: [u8; 32],
    pub risk_score: f64,
    pub reason: String,
}

impl AnubisVeto {
    pub fn new(threshold: f64) -> Self {
        Self { threshold, vetoes: Vec::new() }
    }

    /// Compute a risk score for a hyperedge and decide whether to veto.
    ///
    /// Risk = focal score + payload contribution. Returns the risk and whether
    /// it exceeded the threshold.
    pub fn evaluate(&mut self, edge: &Hyperedge, payload_risk: f64) -> VetoResult {
        let total = edge.focal_score + payload_risk;
        let should_veto = total > self.threshold;
        if should_veto {
            self.vetoes.push(VetoRecord {
                edge_id: edge.id,
                risk_score: total,
                reason: format!(
                    "focal {:.3} + payload {:.3} > threshold {:.3}",
                    edge.focal_score, payload_risk, self.threshold
                ),
            });
        }
        VetoResult { risk: total, veto: should_veto }
    }

    /// Surrender-based payload risk estimate: gate high-opacity payloads.
    pub fn payload_risk(payload: &crate::ScientificPayload) -> f64 {
        use crate::ScientificPayload::*;
        match payload {
            Hypothesis { .. } => 0.1,
            LeanProof { .. } => 0.0,
            SimulationResult { fidelity, .. } => (1.0 - fidelity).max(0.0),
            ExperimentalReadout { .. } => 0.3,
            VetoRequest { risk_score, .. } => *risk_score,
            Certificate { .. } => 0.0,
        }
    }
}

/// Outcome of an Anubis evaluation.
#[derive(Debug, Clone, Copy)]
pub struct VetoResult {
    pub risk: f64,
    pub veto: bool,
}

impl Default for AnubisVeto {
    fn default() -> Self {
        Self::new(0.75)
    }
}