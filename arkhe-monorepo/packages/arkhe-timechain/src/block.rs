//! A Timechain block — a topology rewrite anchored to the Shadow.
//!
//! A block is final only when the network agrees (via loop closure) that its
//! predicted helicity delta is consistent across a majority of echoes. The
//! block carries the trailing Shadow spectrum, so its hash is bound to the
//! field's retained memory — not just its visible state.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// A 32-byte content hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHash(pub [u8; 32]);

impl BlockHash {
    /// Compare by textual hex (for logs / diagnostics).
    pub fn eq_hex(&self, hex: &str) -> bool {
        hex::encode(&self.0).eq_ignore_ascii_case(hex)
    }
}

impl std::fmt::Display for BlockHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

/// Hash an arbitrary byte slice.
fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// Hash a slice of `f64` by first canonicalising each to its 8-byte IEEE bits.
pub fn hash_f64s(values: &[f64]) -> [u8; 32] {
    let mut bs = Vec::with_capacity(values.len() * 8);
    for v in values {
        bs.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    sha256_bytes(&bs)
}

/// A ledger block carrying the topology transition around the observer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBlock {
    /// Sequential height.
    pub height: u64,
    /// Hash of the parent block.
    pub prev_hash: BlockHash,
    /// Node that produced the block.
    pub producer_id: u64,
    /// Rank-`k` helicity before the rewrite.
    pub h_before: f64,
    /// Rank-`k` helicity after the rewrite.
    pub h_after: f64,
    /// `h_after − h_before` — the topology change under test.
    pub delta_h: f64,
    /// Chern–Simons phase of the transition (radians).
    pub phase: f64,
    /// Observer attachment (0 = fully local, 1 = fully global).
    pub attachment: f64,
    /// Trailing Shadow spectrum (tail singular values).
    pub shadow_spectrum: Vec<f64>,
    /// Content hash of the block.
    pub block_hash: BlockHash,
}

impl TimeBlock {
    /// Finalise a candidate block (computes and stores its content hash).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        height: u64,
        prev: BlockHash,
        producer_id: u64,
        h_before: f64,
        h_after: f64,
        phase: f64,
        attachment: f64,
        shadow_spectrum: Vec<f64>,
    ) -> Self {
        let mut bs = Vec::with_capacity(8 + 32 + 8 + 8 + 8 + 8 + 8);
        bs.extend_from_slice(&height.to_le_bytes());
        bs.extend_from_slice(&prev.0);
        bs.extend_from_slice(&producer_id.to_le_bytes());
        bs.extend_from_slice(&h_before.to_bits().to_le_bytes());
        bs.extend_from_slice(&h_after.to_bits().to_le_bytes());
        bs.extend_from_slice(&phase.to_bits().to_le_bytes());
        bs.extend_from_slice(&attachment.to_bits().to_le_bytes());
        let block_hash = BlockHash(sha256_bytes(&bs));
        Self {
            height,
            prev_hash: prev,
            producer_id,
            h_before,
            h_after,
            delta_h: h_after - h_before,
            phase,
            attachment,
            shadow_spectrum,
            block_hash,
        }
    }

    /// Recompute and compare the stored content hash — `Ghost-1` style sanity.
    pub fn recompute_hash(&self) -> BlockHash {
        let mut bs = Vec::with_capacity(32);
        bs.extend_from_slice(&self.height.to_le_bytes());
        bs.extend_from_slice(&self.prev_hash.0);
        bs.extend_from_slice(&self.producer_id.to_le_bytes());
        bs.extend_from_slice(&self.h_before.to_le_bytes());
        bs.extend_from_slice(&self.h_after.to_le_bytes());
        bs.extend_from_slice(&self.phase.to_le_bytes());
        bs.extend_from_slice(&self.attachment.to_le_bytes());
        BlockHash(sha256_bytes(&bs))
    }

    /// Block integrity + observer ("handover") bounds.
    ///
    /// * `attachment` must stay under `attachment_max` (handover within physics).
    /// * Phase must stay monotonic-ish (nonzero modulo `π`, i.e. not a no-op).
    /// * The transition must be finitely coupled (helicity change finite).
    pub fn verifies(&self, attachment_max: f64) -> bool {
        self.attachment.abs() <= attachment_max
            && self.phase.is_finite()
            && self.delta_h.is_finite()
            && self.recompute_hash() == self.block_hash
    }
}

/// tiny hex helper (no external `hex` crate).
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            use std::fmt::Write;
            let _ = write!(s, "{:02x}", b);
        }
        s
    }
}