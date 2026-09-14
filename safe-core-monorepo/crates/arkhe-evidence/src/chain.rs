//! FI-011/FI-017 — hash-chained, tamper-evident evidence log.
//!
//! Distinct from `arkhe-web3-security::agents::evidence_bus::EvidenceBus`:
//! that one is a flat `Vec<AuditEvidence>` recording invariant-check
//! verdicts, with no linking between entries — appropriate for what it's
//! for, but not tamper-evident (nothing detects a record being silently
//! removed or reordered).
//!
//! # Two hashes, on purpose
//!
//! Each [`EvidenceRecord`] carries **two** BLAKE3 hashes, and they answer two
//! different questions:
//!
//! - [`hash`](EvidenceRecord::hash) — `hash(prev_hash ∥ len(payload) ∥
//!   payload)`. This is the catalog's exact formula (FI-011/FI-017) and it is
//!   what makes the log a *chain*: it binds each record's payload to its
//!   predecessor, so editing a payload, removing a record, or reordering two
//!   records breaks the chain from that point forward.
//! - [`record_hash`](EvidenceRecord::record_hash) — an **additive**,
//!   domain-separated binding over the record's own four fields:
//!   `BLAKE3(domain ∥ index ∥ timestamp ∥ prev_hash ∥ len(payload) ∥
//!   payload)`.
//!
//! They are two because the catalog formula is *specified and registered*:
//! rewriting it would stop implementing FI-011/FI-017. The metadata gap it
//! leaves — `index` and `timestamp` are not covered by `hash` — is therefore
//! closed **additively**, by a second hash over the same record, exactly the
//! practice the catalog documents elsewhere ("built additively instead"). The
//! distinct domain tag (`arkhe-evidence/record-hash/v1`) guarantees the two
//! hashes can never be confused: no `record_hash` equals a catalog `hash`,
//! and vice versa.
//!
//! A chain is intact only when **both** checks pass:
//! [`EvidenceChain::verify_chain`] runs them together. To validate a chain
//! against the catalog contract alone — `hash` plus the `prev_hash` linkage —
//! use [`EvidenceChain::verify_catalog_chain`], which deliberately ignores
//! `record_hash`.

use arkhe_core::hash::blake3_hash;
use arkhe_core::ArkheHash;
use tokio::sync::RwLock;

/// The `prev_hash` of the first record in a chain — an all-zero sentinel,
/// not a real BLAKE3 output (BLAKE3 never produces the all-zero digest for
/// any real input with overwhelming probability, but this is a fixed
/// convention, not a security property being relied on).
pub const GENESIS_HASH: ArkheHash = [0u8; 32];

/// Domain-separation tag for [`EvidenceRecord::record_hash`].
///
/// The catalog formula (`hash(prev_hash ∥ len(payload) ∥ payload)`) has no
/// domain tag; prefixing a distinct one here means a `record_hash` can never
/// equal a catalog `hash`, so the two verifications can never be substituted
/// for one another. The `v1` is part of the tag so a future encoding change
/// can pick a new tag instead of silently reinterpreting existing hashes.
const RECORD_HASH_DOMAIN: &[u8] = b"arkhe-evidence/record-hash/v1";

/// One link of the evidence chain.
///
/// Both `hash` and `record_hash` are filled by [`EvidenceChain::append`] and
/// re-checked by [`EvidenceChain::verify_chain`]. See the module docs for why
/// there are two hashes and how they differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRecord {
    /// Position of this record in the chain, assigned by `append`.
    pub index: u64,
    /// The record's opaque bytes — what the chain attests to.
    pub payload: Vec<u8>,
    /// Caller-supplied stamp; the chain does not interpret it.
    pub timestamp: u64,
    /// The predecessor's `hash`, or [`GENESIS_HASH`] for the first record.
    pub prev_hash: ArkheHash,
    /// The catalog chain hash: `hash(prev_hash ∥ len(payload) ∥ payload)`.
    pub hash: ArkheHash,
    /// `BLAKE3(domain ∥ index ∥ timestamp ∥ prev_hash ∥ len(payload) ∥
    /// payload)` — binds the fields `hash` does not cover. Never equal to
    /// `hash` (domain separation).
    pub record_hash: ArkheHash,
}

impl EvidenceRecord {
    /// `hash(prev_hash ∥ len(payload) ∥ payload)` — the catalog's formula,
    /// unchanged. The length prefix prevents two different (prev_hash,
    /// payload) pairs whose payloads happen to share a byte-string boundary
    /// from colliding.
    fn compute_hash(prev_hash: &ArkheHash, payload: &[u8]) -> ArkheHash {
        let mut buf = Vec::with_capacity(32 + 8 + payload.len());
        buf.extend_from_slice(prev_hash);
        buf.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        buf.extend_from_slice(payload);
        blake3_hash(&buf)
    }

    /// `BLAKE3(domain ∥ index ∥ timestamp ∥ prev_hash ∥ len(payload) ∥
    /// payload)`.
    ///
    /// Additive to [`compute_hash`](Self::compute_hash): that one implements
    /// the catalog's formula and must not change; this one closes the gap it
    /// leaves by binding `index` and `timestamp` as well, under a distinct
    /// domain tag. Both `u64`s are encoded fixed-width big-endian so no two
    /// field values can produce the same byte string, `prev_hash` is used raw
    /// (it is already exactly 32 bytes), and the payload keeps the same
    /// length prefix as the catalog formula, for the same boundary reason.
    fn compute_record_hash(
        index: u64,
        timestamp: u64,
        prev_hash: &ArkheHash,
        payload: &[u8],
    ) -> ArkheHash {
        let mut buf = Vec::with_capacity(RECORD_HASH_DOMAIN.len() + 8 + 8 + 32 + 8 + payload.len());
        buf.extend_from_slice(RECORD_HASH_DOMAIN);
        buf.extend_from_slice(&index.to_be_bytes());
        buf.extend_from_slice(&timestamp.to_be_bytes());
        buf.extend_from_slice(prev_hash);
        buf.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        buf.extend_from_slice(payload);
        blake3_hash(&buf)
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum ChainError {
    #[error("chain is broken at index {index}: recomputed hash does not match the stored catalog hash")]
    HashMismatch { index: u64 },
    #[error(
        "chain is broken at index {index}: recomputed record hash does not match the stored record_hash (index/timestamp/prev_hash/payload)"
    )]
    RecordHashMismatch { index: u64 },
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
    /// (or [`GENESIS_HASH`] if this is the first record). Fills in **both**
    /// hashes: the catalog `hash` and the additive `record_hash`.
    ///
    /// The linkage still runs over `hash`, not `record_hash`, so the catalog
    /// chain remains byte-for-byte the structure FI-011/FI-017 specifies.
    pub async fn append(&self, payload: Vec<u8>, timestamp: u64) -> EvidenceRecord {
        let mut records = self.records.write().await;
        let index = records.len() as u64;
        let prev_hash = records.last().map(|r| r.hash).unwrap_or(GENESIS_HASH);
        let hash = EvidenceRecord::compute_hash(&prev_hash, &payload);
        let record_hash =
            EvidenceRecord::compute_record_hash(index, timestamp, &prev_hash, &payload);
        let record = EvidenceRecord {
            index,
            payload,
            timestamp,
            prev_hash,
            hash,
            record_hash,
        };
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

    /// Verifies the whole chain from genesis, checking **both** hashes: for
    /// every record the catalog `hash` (recomputed from `(prev_hash,
    /// payload)`) and the additive `record_hash` (recomputed from `(index,
    /// timestamp, prev_hash, payload)`), plus the `prev_hash` linkage to the
    /// actual predecessor.
    ///
    /// The catalog check runs first, so a payload edit is still reported as
    /// [`ChainError::HashMismatch`] exactly as before; a record whose `index`
    /// or `timestamp` was edited passes the catalog check and is reported as
    /// [`ChainError::RecordHashMismatch`] instead. A chain is intact only
    /// when both checks pass.
    pub async fn verify_chain(&self) -> Result<(), ChainError> {
        let records = self.records.read().await;
        Self::verify_catalog_prefix(&records, records.len())?;
        Self::verify_record_prefix(&records, records.len())
    }

    /// Verifies only the catalog chain (FI-011/FI-017): each record's `hash`
    /// against `hash(prev_hash ∥ len(payload) ∥ payload)`, plus the
    /// `prev_hash` linkage — the exact contract the catalog registers, and
    /// nothing else.
    ///
    /// Deliberately does **not** check `record_hash`, so it stays true to the
    /// catalog formula alone. To validate a chain against the catalog
    /// contract while auditing the metadata binding too, use
    /// [`verify_chain`](Self::verify_chain); the two are exposed separately so
    /// it is explicit which of the two verifications a caller wants.
    pub async fn verify_catalog_chain(&self) -> Result<(), ChainError> {
        let records = self.records.read().await;
        Self::verify_catalog_prefix(&records, records.len())
    }

    /// FI-017 — "hash chain é verificável em qualquer ponto": verifies only
    /// the prefix up to and including `index`, without needing later
    /// records to exist or be valid yet. Like [`verify_chain`](Self::verify_chain),
    /// checks both the catalog `hash`/linkage and the additive `record_hash`
    /// within that prefix.
    pub async fn verify_at(&self, index: u64) -> Result<(), ChainError> {
        let records = self.records.read().await;
        if index as usize >= records.len() {
            return Err(ChainError::IndexOutOfBounds(index));
        }
        let up_to = index as usize + 1;
        Self::verify_catalog_prefix(&records, up_to)?;
        Self::verify_record_prefix(&records, up_to)
    }

    /// The catalog verification (FI-011/FI-017): `prev_hash` linkage and the
    /// `hash(prev_hash ∥ len(payload) ∥ payload)` formula, exactly as
    /// registered.
    ///
    /// Uses each record's actual position in `records` (via `.enumerate()`)
    /// for error reporting — deliberately **not** `record.index` (the
    /// record's own self-reported field). After tampering that reorders
    /// records, a stale/manipulated `index` field is exactly the kind of
    /// thing this function exists to catch, so it can't be trusted for
    /// reporting where the break actually is.
    fn verify_catalog_prefix(records: &[EvidenceRecord], up_to: usize) -> Result<(), ChainError> {
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

    /// The additive check: recomputes each record's `record_hash` over
    /// `(index, timestamp, prev_hash, payload)` and compares. This is what
    /// catches an edit to `index` or `timestamp`, which the catalog formula
    /// cannot see.
    ///
    /// Runs after, and separately from, [`verify_catalog_prefix`](Self::verify_catalog_prefix)
    /// so each verification keeps its own diagnostic. Uses the actual
    /// position (not `record.index`) for the same reason the catalog check
    /// does: the reported index must not come from the field under suspicion.
    fn verify_record_prefix(records: &[EvidenceRecord], up_to: usize) -> Result<(), ChainError> {
        for (position, record) in records[..up_to].iter().enumerate() {
            let recomputed = EvidenceRecord::compute_record_hash(
                record.index,
                record.timestamp,
                &record.prev_hash,
                &record.payload,
            );
            if recomputed != record.record_hash {
                return Err(ChainError::RecordHashMismatch {
                    index: position as u64,
                });
            }
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
        // Both hashes are filled, and neither is ever the catalog hash of
        // its own record (domain separation).
        assert_ne!(r1.hash, r1.record_hash);
        assert_ne!(r2.hash, r2.record_hash);
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

        // Still the catalog check that fires: the payload is in its formula.
        assert_eq!(chain.verify_chain().await, Err(ChainError::HashMismatch { index: 0 }));
    }

    #[tokio::test]
    async fn tampering_a_records_timestamp_is_detected() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;
        chain.append(b"b".to_vec(), 2).await;

        {
            let mut records = chain.records.write().await;
            records[0].timestamp = 999;
        }

        // The catalog check passes — `timestamp` is not in its formula — so
        // this is specifically the additive record-hash check firing. Before
        // that check existed, this whole call returned `Ok`.
        assert_eq!(
            chain.verify_chain().await,
            Err(ChainError::RecordHashMismatch { index: 0 })
        );
    }

    #[tokio::test]
    async fn tampering_a_records_index_is_detected() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;
        chain.append(b"b".to_vec(), 2).await;

        {
            let mut records = chain.records.write().await;
            records[1].index = 7;
        }

        assert_eq!(
            chain.verify_chain().await,
            Err(ChainError::RecordHashMismatch { index: 1 })
        );
    }

    #[tokio::test]
    async fn tampering_a_stored_record_hash_is_detected() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;

        {
            let mut records = chain.records.write().await;
            records[0].record_hash = [0xAB; 32];
        }

        assert_eq!(
            chain.verify_chain().await,
            Err(ChainError::RecordHashMismatch { index: 0 })
        );
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

    #[tokio::test]
    async fn verify_at_detects_metadata_tampering_within_the_prefix() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;
        chain.append(b"b".to_vec(), 2).await;

        {
            let mut records = chain.records.write().await;
            records[0].timestamp = 999;
        }

        assert_eq!(
            chain.verify_at(1).await,
            Err(ChainError::RecordHashMismatch { index: 0 })
        );
    }

    /// Recomputes the catalog's formula by hand, from the original
    /// definition, and confirms the stored `hash` still matches. This is the
    /// test that pins FI-011/FI-017: the additive record-hash work did not
    /// alter the formula the catalog registers.
    #[tokio::test]
    async fn stored_hash_still_matches_the_catalog_formula() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;
        chain.append(b"b".to_vec(), 2).await;

        let r0 = chain.record_at(0).await.unwrap();
        let r1 = chain.record_at(1).await.unwrap();

        // `hash(prev_hash ∥ len(payload) ∥ payload)`, spelled out here rather
        // than calling into the crate, so this cannot silently drift with the
        // implementation it is meant to pin.
        let catalog = |prev_hash: &ArkheHash, payload: &[u8]| {
            let mut buf = Vec::new();
            buf.extend_from_slice(prev_hash);
            buf.extend_from_slice(&(payload.len() as u64).to_be_bytes());
            buf.extend_from_slice(payload);
            blake3_hash(&buf)
        };

        assert_eq!(r0.hash, catalog(&GENESIS_HASH, b"a"));
        assert_eq!(r1.hash, catalog(&r0.hash, b"b"));
        assert_eq!(r1.prev_hash, r0.hash, "the linkage the formula feeds is unchanged");
    }

    /// The record hash binds all four fields, and the domain tag keeps it
    /// disjoint from the catalog hash.
    #[tokio::test]
    async fn record_hash_binds_all_four_fields_and_is_domain_separated() {
        let base = EvidenceRecord::compute_record_hash(0, 1, &GENESIS_HASH, b"a");

        assert_ne!(
            base,
            EvidenceRecord::compute_record_hash(1, 1, &GENESIS_HASH, b"a"),
            "index is covered"
        );
        assert_ne!(
            base,
            EvidenceRecord::compute_record_hash(0, 2, &GENESIS_HASH, b"a"),
            "timestamp is covered"
        );
        assert_ne!(
            base,
            EvidenceRecord::compute_record_hash(0, 1, &[1u8; 32], b"a"),
            "prev_hash is covered"
        );
        assert_ne!(
            base,
            EvidenceRecord::compute_record_hash(0, 1, &GENESIS_HASH, b"b"),
            "payload is covered"
        );

        let same_fields_catalog = EvidenceRecord::compute_hash(&GENESIS_HASH, b"a");
        assert_ne!(
            base, same_fields_catalog,
            "the domain tag keeps record_hash disjoint from the catalog hash"
        );
    }

    /// `verify_catalog_chain` is the catalog contract alone: it accepts a
    /// chain whose metadata was edited, while `verify_chain` rejects the same
    /// chain. That difference is the reason both are exposed.
    #[tokio::test]
    async fn catalog_only_verification_ignores_metadata_tampering() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;

        {
            let mut records = chain.records.write().await;
            records[0].timestamp = 999;
        }

        assert!(chain.verify_catalog_chain().await.is_ok());
        assert_eq!(
            chain.verify_chain().await,
            Err(ChainError::RecordHashMismatch { index: 0 })
        );
    }

    #[tokio::test]
    async fn catalog_only_verification_still_catches_a_payload_edit() {
        let chain = EvidenceChain::new();
        chain.append(b"a".to_vec(), 1).await;

        {
            let mut records = chain.records.write().await;
            records[0].payload = b"tampered".to_vec();
        }

        assert_eq!(
            chain.verify_catalog_chain().await,
            Err(ChainError::HashMismatch { index: 0 })
        );
    }
}
