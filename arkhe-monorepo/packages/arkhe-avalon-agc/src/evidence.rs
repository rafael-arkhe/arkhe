//! Attestable evidence for Safe-Core (Phase 3 pre-work).
//!
//! Two record types travel across the Python → Rust → Safe-Core boundary:
//!
//! * [`GainEvidence`] — what the AGC controller actually *did* with a request
//!   (DAC code applied, slew-limited or not, read-back truthfulness).
//! * [`CoherenceEvidence`] — what the Python ensemble *reported* about the
//!   thermodynamics (ε²_TUT, ε²_obs, bound satisfaction, DFT flag).
//!
//! Both are serialized to SHA3-256 digests that Safe-Core can pin. The hash
//! is over **canonical little-endian bytes** of the typed fields — not over a
//! JSON string — so the digest is stable regardless of serialization vendor.
//!
//! Float fields are validated against non-finite/inf poisoning via
//! [`Untrusted`](crate::Untrusted).

use crate::untrusted::Untrusted;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

/// Record of one gain application.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GainEvidence {
    /// Requested target voltage, mV.
    pub requested_mv: u16,
    /// Voltage actually applied after slew limiting, mV.
    pub applied_mv: u16,
    /// Raw DAC code written.
    pub dac_value: u16,
    /// DAC reference voltage, mV.
    pub dac_vref_mv: u16,
    /// DAC resolution, bits.
    pub dac_bits: u8,
    /// True if the slew limiter reduced the applied value below the request.
    pub slew_limited: bool,
    /// Whether the driver genuinely read back the same value.
    ///
    /// This is `None` when the driver has no read-back channel — and the
    /// digest differs accordingly (a "true" here without a real channel would
    /// violate [INV-DATA-01]).
    pub read_back_ok: Option<bool>,
    /// Request timestamp, monotonic ms.
    pub timestamp_ms: u64,
}

impl GainEvidence {
    /// Canonical SHA3-256 digest of the evidence.
    ///
    /// The digest covers every typed field, so a tamper with *any* field
    /// (including the `read_back_ok` honesty flag) changes the hash.
    #[must_use]
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(self.requested_mv.to_le_bytes());
        hasher.update(self.applied_mv.to_le_bytes());
        hasher.update(self.dac_value.to_le_bytes());
        hasher.update(self.dac_vref_mv.to_le_bytes());
        hasher.update([self.dac_bits]);
        hasher.update([u8::from(self.slew_limited)]);
        match self.read_back_ok {
            None => hasher.update([0xFF]),
            Some(ok) => hasher.update([0x00, u8::from(ok)]),
        }
        hasher.update(self.timestamp_ms.to_le_bytes());
        hasher.finalize().into()
    }
}

/// Thermodynamic coherence evidence reported by the Python ensemble.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CoherenceEvidence {
    /// TUT upper bound ε²_TUT from the ensemble.
    pub eps_tut: f64,
    /// Observed scaled variance ε²_obs.
    pub eps_obs: f64,
    /// Whether the TUT bound was satisfied (obs ≥ bound).
    pub bound_satisfied: bool,
    /// Coherence order parameter r (0..1).
    pub coherence_r: f64,
    /// Number of oscillator nodes.
    pub n_nodes: u32,
    /// Monotonic timestamp, ms.
    pub timestamp_ms: u64,
    /// Whether the DFT assumption was independently verified (INV-DFT-01).
    pub dft_verified: bool,
}

/// Errors from constructing trusted coherence evidence out of untrusted
/// Python floats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoherenceError {
    /// A float payload was `NaN` or infinite.
    NonFiniteFloat,
    /// `eps_tut` present but `dft_verified` is false.
    DftNotVerified,
    /// `n_nodes < 2`.
    TooFewNodes,
    /// coherence_r outside [0,1].
    CoherenceOutOfRange,
}

impl CoherenceEvidence {
    /// Validate untrusted float fields from Python and, on success, produce
    /// trusted evidence (hashable / sendable to Safe-Core).
    ///
    /// This is the *only* construction path that accepts external floats —
    /// mirroring `Untrusted<T>` policy.
    pub fn new_unvalidated(
        eps_tut: Untrusted<f64>,
        eps_obs: Untrusted<f64>,
        bound_satisfied: bool,
        coherence_r: Untrusted<f64>,
        n_nodes: u32,
        timestamp_ms: u64,
        dft_verified: bool,
    ) -> Result<Self, CoherenceError> {
        let eps_tut = eps_tut.to_finite().ok_or(CoherenceError::NonFiniteFloat)?;
        let eps_obs = eps_obs.to_finite().ok_or(CoherenceError::NonFiniteFloat)?;
        let coherence_r = coherence_r.to_finite().ok_or(CoherenceError::NonFiniteFloat)?;

        // Governance rules (loopseal / ethics family).
        if !dft_verified {
            return Err(CoherenceError::DftNotVerified);
        }
        if n_nodes < 2 {
            return Err(CoherenceError::TooFewNodes);
        }
        if !(0.0..=1.0).contains(&coherence_r) {
            return Err(CoherenceError::CoherenceOutOfRange);
        }

        Ok(Self {
            eps_tut,
            eps_obs,
            bound_satisfied,
            coherence_r,
            n_nodes,
            timestamp_ms,
            dft_verified,
        })
    }

    /// Build evidence directly from validated (already-trusted) values.
    ///
    /// Used by host-side consumers that computed everything locally. The
    /// caller guarantees the floats are finite.
    #[must_use]
    pub const fn from_trusted(
        eps_tut: f64,
        eps_obs: f64,
        bound_satisfied: bool,
        coherence_r: f64,
        n_nodes: u32,
        timestamp_ms: u64,
        dft_verified: bool,
    ) -> Self {
        Self {
            eps_tut,
            eps_obs,
            bound_satisfied,
            coherence_r,
            n_nodes,
            timestamp_ms,
            dft_verified,
        }
    }

    /// Canonical SHA3-256 digest. Also the Merkle leaf preimage for any
    /// batch attestation forest.
    #[must_use]
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(self.eps_tut.to_le_bytes());
        hasher.update(self.eps_obs.to_le_bytes());
        hasher.update([u8::from(self.bound_satisfied)]);
        hasher.update(self.coherence_r.to_le_bytes());
        hasher.update(self.n_nodes.to_le_bytes());
        hasher.update(self.timestamp_ms.to_le_bytes());
        hasher.update([u8::from(self.dft_verified)]);
        hasher.finalize().into()
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        let a = CoherenceEvidence::from_trusted(0.429, 0.031, true, 0.9, 16, 0, true);
        let b = CoherenceEvidence::from_trusted(0.429, 0.031, true, 0.9, 16, 0, true);
        assert_eq!(a.hash(), b.hash());
    }

    #[test]
    fn tamper_changes_hash() {
        let a = CoherenceEvidence::from_trusted(0.429, 0.031, true, 0.9, 16, 0, true);
        let b = CoherenceEvidence::from_trusted(0.429, 0.031, true, 0.9, 16, 1, true);
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn gate_rejects_unverified_dft() {
        let err = CoherenceEvidence::new_unvalidated(
            Untrusted::new(0.429),
            Untrusted::new(0.031),
            true,
            Untrusted::new(0.9),
            16,
            0,
            false, // DFT not verified -> must fail
        );
        assert_eq!(err, Err(CoherenceError::DftNotVerified));
    }

    #[test]
    fn gate_rejects_nan() {
        let err = CoherenceEvidence::new_unvalidated(
            Untrusted::new(f64::NAN),
            Untrusted::new(0.031),
            true,
            Untrusted::new(0.9),
            16,
            0,
            true,
        );
        assert_eq!(err, Err(CoherenceError::NonFiniteFloat));
    }

    #[test]
    fn gate_rejects_negative_coherence() {
        let err = CoherenceEvidence::new_unvalidated(
            Untrusted::new(0.429),
            Untrusted::new(0.031),
            true,
            Untrusted::new(-0.1),
            16,
            0,
            true,
        );
        assert_eq!(err, Err(CoherenceError::CoherenceOutOfRange));
    }

    #[test]
    fn gain_evidence_hash_changes_with_readback_flag() {
        let mut honest = GainEvidence {
            requested_mv: 600,
            applied_mv: 600,
            dac_value: 744,
            dac_vref_mv: 3300,
            dac_bits: 12,
            slew_limited: false,
            read_back_ok: None, // no channel
            timestamp_ms: 0,
        };
        let h_no_readback = honest.hash();
        honest.read_back_ok = Some(true); // claimed, but no channel!
        let h_claimed = honest.hash();
        assert_ne!(h_no_readback, h_claimed, "faked read-back must change digest");
    }
}