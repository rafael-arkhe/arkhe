//! NIST AI Risk Management Framework — Trustworthiness Scoring.
//!
//! Implements trustworthiness characteristic scoring against the
//! 7 NIST AI RMF characteristics, mapped to supporting invariants.

use serde::{Deserialize, Serialize};
use crate::invariants::{Invariant, SystemState};

/// NIST AI RMF trustworthiness characteristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrustworthinessCharacteristic {
    Valid,
    Reliable,
    Robust,
    Safe,
    Secure,
    Transparent,
    Accountable,
}

impl TrustworthinessCharacteristic {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Valid, Self::Reliable, Self::Robust,
            Self::Safe, Self::Secure, Self::Transparent, Self::Accountable,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Reliable => "reliable",
            Self::Robust => "robust",
            Self::Safe => "safe",
            Self::Secure => "secure",
            Self::Transparent => "transparent",
            Self::Accountable => "accountable",
        }
    }

    /// Invariants that support this characteristic.
    pub fn supporting_invariants(&self) -> Vec<Invariant> {
        match self {
            Self::Valid => vec![Invariant::I08, Invariant::I11],
            Self::Reliable => vec![Invariant::I03, Invariant::I07],
            Self::Robust => vec![Invariant::I01, Invariant::I02, Invariant::I04],
            Self::Safe => vec![Invariant::I05, Invariant::I09, Invariant::I13],
            Self::Secure => vec![Invariant::I06, Invariant::I09, Invariant::I13],
            Self::Transparent => vec![Invariant::I12, Invariant::I16],
            Self::Accountable => vec![Invariant::I14, Invariant::I15],
        }
    }
}

/// Score for a single trustworthiness characteristic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacteristicScore {
    pub characteristic: TrustworthinessCharacteristic,
    pub score: f64,
    pub invariants_met: usize,
    pub invariants_total: usize,
}

/// NIST AI RMF trustworthiness score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NistRmfScore {
    pub scores: Vec<CharacteristicScore>,
    pub overall_score: f64,
}

impl NistRmfScore {
    /// Compute trustworthiness scores against a system state.
    pub fn evaluate(state: &SystemState) -> Self {
        let mut scores = Vec::new();
        let mut total_score = 0.0;

        for char in TrustworthinessCharacteristic::all() {
            let invariants = char.supporting_invariants();
            let total = invariants.len();
            let met = invariants.iter().filter(|inv| inv.check(state)).count();
            let score = if total > 0 { met as f64 / total as f64 } else { 1.0 };
            total_score += score;
            scores.push(CharacteristicScore {
                characteristic: char,
                score,
                invariants_met: met,
                invariants_total: total,
            });
        }

        let overall_score = total_score / TrustworthinessCharacteristic::all().len() as f64;
        Self { scores, overall_score }
    }

    /// Check if overall score meets minimum threshold.
    pub fn meets_threshold(&self, threshold: f64) -> bool {
        self.overall_score >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::SystemConfig;

    #[test]
    fn nist_rmf_evaluate_safe_state() {
        let config = SystemConfig::default();
        let state = SystemState::safe(config);
        let score = NistRmfScore::evaluate(&state);
        assert!(score.overall_score > 0.9);
    }

    #[test]
    fn nist_rmf_characteristics_count() {
        assert_eq!(TrustworthinessCharacteristic::all().len(), 7);
    }

    #[test]
    fn nist_rmf_meets_threshold() {
        let config = SystemConfig::default();
        let state = SystemState::safe(config);
        let score = NistRmfScore::evaluate(&state);
        assert!(score.meets_threshold(0.9));
    }
}
