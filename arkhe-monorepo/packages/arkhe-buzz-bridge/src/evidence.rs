//! Evidence bundle types exchanged via the Buzz relay.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CertificationStatus {
    Supported,
    Rejected,
    Inconclusive,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DivergenceReport {
    pub structural: Option<f64>,
    pub observational: Option<f64>,
    pub invariant_violations: Vec<String>,
    pub threshold: f64,
    pub has_divergence: bool,
}

impl DivergenceReport {
    pub fn none(threshold: f64) -> Self {
        Self {
            structural: None,
            observational: None,
            invariant_violations: Vec::new(),
            threshold,
            has_divergence: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceBundle {
    pub id: String,
    pub hypothesis: String,
    pub baseline_hash: String,
    pub pump_sequence: Vec<String>,
    pub probe: String,
    pub counterfactual: String,
    pub observations_forward: Vec<String>,
    pub observations_reverse: Vec<String>,
    pub divergences: DivergenceReport,
    pub witness: Option<String>,
    pub certification: CertificationStatus,
    pub timestamp: String,
}

impl EvidenceBundle {
    /// Cryptographic digest of the bundle, used to bind the firewall
    /// `TRANSLATES_TO_PRIMITIVE` edge to the actual content (T8).
    pub fn digest(&self) -> [u8; 32] {
        use sha3::{Digest, Sha3_256};
        let json = serde_json::to_vec(self).unwrap_or_default();
        Sha3_256::digest(&json).into()
    }

    pub fn digest_hex(&self) -> String {
        hex::encode(self.digest())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_digest_is_stable() {
        let a = EvidenceBundle {
            id: "eb-1".into(),
            hypothesis: "h".into(),
            baseline_hash: "b".into(),
            pump_sequence: vec!["p".into()],
            probe: "x".into(),
            counterfactual: "y".into(),
            observations_forward: vec!["o".into()],
            observations_reverse: vec![],
            divergences: DivergenceReport::none(0.5),
            witness: None,
            certification: CertificationStatus::Pending,
            timestamp: "2026-08-02T00:00:00Z".into(),
        };
        assert_eq!(a.digest(), a.digest());
        assert_eq!(a.digest_hex().len(), 64);
    }
}
