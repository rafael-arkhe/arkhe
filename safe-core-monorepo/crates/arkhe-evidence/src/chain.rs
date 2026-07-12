//! FI-011/FI-017 — hash-chained, tamper-evident evidence log.
//!
//! Distinct from `arkhe-web3-security::agents::evidence_bus::EvidenceBus`:
//! that one is a flat `Vec<AuditEvidence>` recording invariant-check
//! verdicts, with no linking between entries — appropriate for what it's
//! for, but not tamper-evident (nothing detects a record being silently
//! removed or reordered). This crate implements the catalog's exact
//! formula: `hash(prev_hash ∥ len(payload) ∥ payload)`, so any change to
//! any record, or removal/reordering of records, breaks the chain from
//! that point forward — detectable by [`EvidenceChain::verify_chain`].

use arkhe_core::hash::blake3_hash;
use arkhe_core::ArkheHash;
use tokio::sync::RwLock;

/// The `prev_hash` of the first record in a chain — an all-zero sentinel,
/// not a real BLAKE3 output (BLAKE3 never produces the all-zero digest for
/// any real input with overwhelming probability, but this is a fixed
/// convention, not a security property being relied on).
pub const GENESIS_HASH: ArkheHash = [0u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRecord {
    pub index: u64,
    pub payload: Vec<u8>,
    pub timestamp: u64,
    pub prev_hash: ArkheHash,
    pub hash: ArkheHash,
}

impl EvidenceRecord {
    /// `hash(prev_hash ∥ len(payload) ∥ payload)` — the length prefix
    /// prevents two different (prev_hash, payload) pairs whose payloads
    /// happen to share a byte-string boundary from colliding.
    fn compute_hash(prev_hash: &ArkheHash, payload: &[u8]) -> ArkheHash {
        let mut buf = Vec::with_capacity(32 + 8 + payload.len());
        buf.extend_from_slice(prev_hash);
        buf.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        buf.extend_from_slice(payload);
        blake3_hash(&buf)
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum ChainError {
    #[error("chain is broken at index {index}: recomputed hash does not match the stored hash")]
    HashMismatch { index: u64 },
    #[error("chain is broken at index {index}: stored prev_hash does not match the previous record's hash")]
    LinkBroken { index: u64 },
    #[error("no record at index {0}")]
    IndexOutOfBounds(u64),
}

#[derive(Default)]
pub struct EvidenceChain {
    records: RwLock<Vec<EvidenceRecord>>,
}

impl EvidenceChain {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a new record, linked to the current tail via `prev_hash`
    /// (or [`GENESIS_HASH`] if this is the first record).
    pub async fn append(&self, payload: Vec<u8>, timestamp: u64) -> EvidenceRecord {
        let mut records = self.records.write().await;
        let prev_hash = records.last().map(|r| r.hash).unwrap_or(GENESIS_HASH);
        let hash = EvidenceRecord::compute_hash(&prev_hash, &payload);
        let record = EvidenceRecord { index: records.len() as u64, payload, timestamp, prev_hash, hash };
        records.push(record.clone());
        record
    }

    pub async fn len(&self) -> usize {
        self.records.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.records.read().await.is_empty()
    }

    pub async fn record_at(&self, index: u64) -> Option<EvidenceRecord> {
        self.records.read().await.get(index as usize).cloned()
    }

    pub async fn latest_hash(&self) -> ArkheHash {
        self.records.read().await.last().map(|r| r.hash).unwrap_or(GENESIS_HASH)
    }

    /// Verifies the entire chain from genesis: for every record, recomputes
    /// its hash from its own `(prev_hash, payload)` and checks it matches
    /// the stored `hash`, and that `prev_hash` matches the actual previous
    /// record's hash (catching a record silently swapped in from a
    /// different position, not just a payload edit).
    pub async fn verify_chain(&self) -> Result<(), ChainError> {
        let records = self.records.read().await;
        Self::verify_prefix(&records, records.len())
    }

    /// FI-017 — "hash chain é verificável em qualquer ponto": verifies only
    /// the prefix up to and including `index`, without needing later
    /// records to exist or be valid yet.
    pub async fn verify_at(&self, index: u64) -> Result<(), ChainError> {
        let records = self.records.read().await;
        if index as usize >= records.len() {
            return Err(ChainError::IndexOutOfBounds(index));
        }
        Self::verify_prefix(&records, index as usize + 1)
    }

    /// Uses each record's actual position in `records` (via `.enumerate()`)
    /// for error reporting — deliberately **not** `record.index` (the
    /// record's own self-reported field). After tampering that reorders
    /// records, a stale/manipulated `index` field is exactly the kind of
    /// thing this function exists to catch, so it can't be trusted for
    /// reporting where the break actually is.
    fn verify_prefix(records: &[EvidenceRecord], up_to: usize) -> Result<(), ChainError> {
        let mut expected_prev = GENESIS_HASH;
        for (position, record) in records[..up_to].iter().enumerate() {
            let position = position as u64;
            if record.prev_hash != expected_prev {
                return Err(ChainError::LinkBroken { index: position });
            }
            let recomputed = EvidenceRecord::compute_hash(&record.prev_hash, &record.payload);
            if recomputed != record.hash {
                return Err(ChainError::HashMismatch { index: position });
            }
            expected_prev = record.hash;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn append_links_records_via_prev_hash() {
        let chain = EvidenceChain::new();
        let r1 = chain.append(b"first".to_vec(), 100).await;
        let r2 = chain.append(b"second".to_vec(), 200).await;
        assert_eq!(r1.prev_hash, GENESIS_HASH);
        assert_eq!(r2.prev_hash, r1.hash);
        assert_ne!(r1.hash, r2.hash);
    }

    #[tokio::test]
    async fn empty_chain_verifies() {
        let chain = EvidenceChain::new();
        assert!(chain.verify_chain().await.is_ok());
    }

    #[tokio::test]
    async fn fresh_multi_record_chain_verifies() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;
        chain.append(b"b".to_vec(), 2).await;
        chain.append(b"c".to_vec(), 3).await;
        assert!(chain.verify_chain().await.is_ok());
    }

    #[tokio::test]
    async fn tampering_a_records_payload_is_detected() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;
        chain.append(b"b".to_vec(), 2).await;

        {
            let mut records = chain.records.write().await;
            records[0].payload = b"tampered".to_vec();
        }

        assert_eq!(chain.verify_chain().await, Err(ChainError::HashMismatch { index: 0 }));
    }

    #[tokio::test]
    async fn swapping_two_records_breaks_the_link() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;
        chain.append(b"b".to_vec(), 2).await;

        {
            let mut records = chain.records.write().await;
            records.swap(0, 1);
        }

        // The swapped-in record at index 0 now has the wrong prev_hash
        // (genesis vs. what it actually had), so this is a LinkBroken, not
        // a HashMismatch — the record's own hash is still internally
        // consistent, it's just linked to the wrong predecessor.
        assert_eq!(chain.verify_chain().await, Err(ChainError::LinkBroken { index: 0 }));
    }

    #[tokio::test]
    async fn verify_at_checks_only_the_requested_prefix() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;
        chain.append(b"b".to_vec(), 2).await;
        chain.append(b"c".to_vec(), 3).await;

        assert!(chain.verify_at(1).await.is_ok());
        assert_eq!(chain.verify_at(10).await, Err(ChainError::IndexOutOfBounds(10)));
    }

    #[tokio::test]
    async fn verify_at_detects_tampering_within_the_prefix() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;
        chain.append(b"b".to_vec(), 2).await;

        {
            let mut records = chain.records.write().await;
            records[0].payload = b"tampered".to_vec();
        }

        assert!(chain.verify_at(1).await.is_err());
    }

    #[tokio::test]
    async fn latest_hash_is_genesis_for_empty_chain() {
        let chain = EvidenceChain::new();
        assert_eq!(chain.latest_hash().await, GENESIS_HASH);
    }

    #[tokio::test]
    async fn record_at_returns_the_correct_record() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;
        chain.append(b"b".to_vec(), 2).await;

        let r = chain.record_at(1).await.unwrap();
        assert_eq!(r.payload, b"b");
        assert!(chain.record_at(99).await.is_none());
    }
}
