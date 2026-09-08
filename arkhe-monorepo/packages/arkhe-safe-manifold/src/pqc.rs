//! Hybrid Post-Quantum Cryptography (PQC) state tracking.
//!
//! Tracks the migration from classical (RSA/ECDSA) to post-quantum
//! (ML-KEM/ML-DSA) signature schemes via hybrid mode.
//!
//! # Migration Phases
//!
//! | Phase | Mode | Description |
//! |-------|------|-------------|
//! | 0 | ClassicalOnly | RSA/ECDSA only |
//! | 1 | Hybrid | Both classical and PQC signatures |
//! | 2 | PqcOnly | ML-KEM/ML-DSA only |

use serde::{Deserialize, Serialize};
use crate::invariants::SystemConfig;

/// Hybrid PQC operational mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HybridPqcMode {
    ClassicalOnly,
    Hybrid,
    PqcOnly,
}

impl HybridPqcMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ClassicalOnly => "classical_only",
            Self::Hybrid => "hybrid",
            Self::PqcOnly => "pqc_only",
        }
    }
}

/// PQC system state tracking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PqcState {
    pub mode: HybridPqcMode,
    pub classical_signature_valid: bool,
    pub pqc_signature_valid: bool,
    pub migration_phase: u8,
    pub key_rotation_count: u64,
}

impl Default for PqcState {
    fn default() -> Self {
        Self {
            mode: HybridPqcMode::Hybrid,
            classical_signature_valid: true,
            pqc_signature_valid: true,
            migration_phase: 1,
            key_rotation_count: 0,
        }
    }
}

impl PqcState {
    /// Check if PQC invariants are satisfied.
    pub fn check_pqc_invariants(&self, config: &SystemConfig) -> bool {
        if !config.require_model_hash_validation {
            return true;
        }
        match self.mode {
            HybridPqcMode::ClassicalOnly => self.classical_signature_valid,
            HybridPqcMode::Hybrid => self.classical_signature_valid && self.pqc_signature_valid,
            HybridPqcMode::PqcOnly => self.pqc_signature_valid,
        }
    }

    /// Advance to next migration phase.
    pub fn advance_phase(&mut self) {
        if self.migration_phase < 2 {
            self.migration_phase += 1;
            self.mode = match self.migration_phase {
                0 => HybridPqcMode::ClassicalOnly,
                1 => HybridPqcMode::Hybrid,
                _ => HybridPqcMode::PqcOnly,
            };
        }
    }

    /// Rotate keys and increment counter.
    pub fn rotate_keys(&mut self) {
        self.key_rotation_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pqc_default_is_hybrid() {
        let state = PqcState::default();
        assert_eq!(state.mode, HybridPqcMode::Hybrid);
    }

    #[test]
    fn pqc_advance_phase() {
        let mut state = PqcState::default();
        assert_eq!(state.migration_phase, 1);
        state.advance_phase();
        assert_eq!(state.migration_phase, 2);
        assert_eq!(state.mode, HybridPqcMode::PqcOnly);
    }

    #[test]
    fn pqc_advance_phase_caps_at_2() {
        let mut state = PqcState::default();
        state.advance_phase();
        state.advance_phase();
        assert_eq!(state.migration_phase, 2);
    }

    #[test]
    fn pqc_hybrid_requires_both() {
        let config = SystemConfig::default();
        let mut state = PqcState::default();
        assert!(state.check_pqc_invariants(&config));
        state.classical_signature_valid = false;
        assert!(!state.check_pqc_invariants(&config));
    }

    #[test]
    fn pqc_rotate_keys() {
        let mut state = PqcState::default();
        assert_eq!(state.key_rotation_count, 0);
        state.rotate_keys();
        assert_eq!(state.key_rotation_count, 1);
    }
}
