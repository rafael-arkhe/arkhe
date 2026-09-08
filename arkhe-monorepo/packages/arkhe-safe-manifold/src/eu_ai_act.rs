//! EU AI Act — Risk Classification and Compliance.
//!
//! Maps AI system risk tiers to invariant requirements.
//!
//! # Risk Tiers (Article 5)
//!
//! | Tier | Description | Requirements |
//! |------|-------------|-------------|
//! | Unacceptable | Banned practices | Cannot deploy |
//! | High | High-risk AI systems | Full compliance required |
//! | Limited | Limited-risk systems | Transparency obligations |
//! | Minimal | Minimal-risk systems | No specific obligations |
//! | NotAnAiSystem | Not an AI system | No requirements |

use serde::{Deserialize, Serialize};
use crate::invariants::{Invariant, SystemState};

/// EU AI Act risk classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EuAiActRisk {
    Unacceptable,
    High,
    Limited,
    Minimal,
    NotAnAiSystem,
}

impl EuAiActRisk {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Unacceptable => "unacceptable",
            Self::High => "high",
            Self::Limited => "limited",
            Self::Minimal => "minimal",
            Self::NotAnAiSystem => "not_ai",
        }
    }

    /// Whether this risk tier permits deployment.
    pub fn allows_deployment(&self) -> bool {
        !matches!(self, Self::Unacceptable)
    }

    /// Required invariants for this risk tier.
    pub fn required_invariants(&self) -> Vec<Invariant> {
        match self {
            Self::Unacceptable => vec![],
            Self::High => Invariant::all(),
            Self::Limited => vec![
                Invariant::I05, Invariant::I06, Invariant::I09,
                Invariant::I12, Invariant::I13,
            ],
            Self::Minimal => vec![Invariant::I06],
            Self::NotAnAiSystem => vec![],
        }
    }
}

/// Article 5 prohibited practices (unacceptable risk).
pub const PROHIBITED_PRACTICES: &[&str] = &[
    "subliminal_manipulation",
    "exploitation_vulnerability",
    "social_scoring",
    "real_time_remote_biometric",
    "emotion_recognition_law_enforcement",
    "scraping_facial_images",
];

/// Risk classification result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskClassification {
    pub risk: EuAiActRisk,
    pub articles: Vec<String>,
    pub missing_invariants: Vec<String>,
    pub can_deploy: bool,
}

impl RiskClassification {
    /// Classify a system state against EU AI Act requirements.
    pub fn classify(state: &SystemState, risk: EuAiActRisk) -> Self {
        let required = risk.required_invariants();
        let articles: Vec<String> = required
            .iter()
            .filter_map(|inv| inv.eu_ai_act_article().map(|a| a.to_string()))
            .collect();
        let missing: Vec<String> = required
            .iter()
            .filter(|inv| !inv.check(state))
            .map(|inv| inv.id().to_string())
            .collect();

        Self {
            risk,
            articles,
            missing_invariants: missing,
            can_deploy: risk.allows_deployment(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::SystemConfig;

    #[test]
    fn eu_ai_act_safe_state_high_risk() {
        let config = SystemConfig::default();
        let state = SystemState::safe(config);
        let classification = RiskClassification::classify(&state, EuAiActRisk::High);
        assert!(classification.can_deploy);
        assert!(classification.missing_invariants.is_empty());
    }

    #[test]
    fn eu_ai_act_unacceptable_blocks() {
        let config = SystemConfig::default();
        let state = SystemState::safe(config);
        let classification = RiskClassification::classify(&state, EuAiActRisk::Unacceptable);
        assert!(!classification.can_deploy);
    }

    #[test]
    fn eu_ai_act_prohibited_practices_count() {
        assert!(PROHIBITED_PRACTICES.len() >= 5);
    }
}
