//! Critical region exploration for the SafeManifold (16 dimensions).
//!
//! Identifies dimensions where the system state approaches invariant
//! thresholds. Each [`BoundaryPoint`] captures one dimension that is
//! near or at its limit.

use serde::{Deserialize, Serialize};
use crate::invariants::SystemState;

/// A dimension that can be explored for proximity to thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExploreDimension {
    TokenBudget,
    AgentCount,
    SandboxFuel,
    EntropyBits,
    RateLimit,
    PqcSignature,
    SbomVerified,
    ModelHashValidated,
    AuditTrailComplete,
    SupplyChainIntegrity,
    ProvenanceAttested,
    BiasScore,
    ExplainabilityScore,
}

impl ExploreDimension {
    /// All 13 core dimensions (boolean dimensions excluded).
    pub fn all() -> Vec<Self> {
        vec![
            Self::TokenBudget,
            Self::AgentCount,
            Self::SandboxFuel,
            Self::EntropyBits,
            Self::RateLimit,
            Self::PqcSignature,
            Self::SbomVerified,
            Self::ModelHashValidated,
            Self::AuditTrailComplete,
            Self::SupplyChainIntegrity,
            Self::ProvenanceAttested,
            Self::BiasScore,
            Self::ExplainabilityScore,
        ]
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::TokenBudget => "token_budget",
            Self::AgentCount => "agent_count",
            Self::SandboxFuel => "sandbox_fuel",
            Self::EntropyBits => "entropy_bits",
            Self::RateLimit => "rate_limit",
            Self::PqcSignature => "pqc_signature",
            Self::SbomVerified => "sbom_verified",
            Self::ModelHashValidated => "model_hash_validated",
            Self::AuditTrailComplete => "audit_trail_complete",
            Self::SupplyChainIntegrity => "supply_chain_integrity",
            Self::ProvenanceAttested => "provenance_attested",
            Self::BiasScore => "bias_score",
            Self::ExplainabilityScore => "explainability_score",
        }
    }
}

/// A point near an invariant boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryPoint {
    pub dimension: ExploreDimension,
    pub value: f64,
    pub threshold: f64,
    pub proximity: f64,
    pub violated: bool,
}

/// Result of exploring critical regions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplorationResult {
    pub boundary_points: Vec<BoundaryPoint>,
    pub risk_score: f64,
    pub violations: usize,
}

/// Explore the critical regions of a system state (16D).
pub fn explore_critical_regions(state: &SystemState) -> ExplorationResult {
    let mut boundary_points = Vec::new();
    let mut risk_score = 0.0;
    let mut violations = 0usize;

    let config = &state.config;

    // Token budget: near floor (< 20% of max)
    let threshold = config.max_tokens as f64 * 0.2;
    let value = state.token_budget as f64;
    let proximity = if config.max_tokens > 0 {
        (value / config.max_tokens as f64).clamp(0.0, 1.0)
    } else { 0.0 };
    let violated = state.token_budget < 0;
    if violated || proximity < 0.2 {
        let prox_ratio = if threshold > 0.0 { value / threshold } else { 1.0 };
        risk_score += prox_ratio.clamp(0.0, 1.0);
        if violated { violations += 1; }
        boundary_points.push(BoundaryPoint {
            dimension: ExploreDimension::TokenBudget,
            value, threshold, proximity: prox_ratio.clamp(0.0, 1.0), violated,
        });
    }

    // Agent count: near ceiling (> 80% of max)
    let value = state.agent_count as f64;
    let proximity = if config.max_agents > 0 {
        (value / config.max_agents as f64).clamp(0.0, 1.0)
    } else { 0.0 };
    let violated = state.agent_count > config.max_agents;
    if violated || proximity > 0.8 {
        risk_score += proximity;
        if violated { violations += 1; }
        boundary_points.push(BoundaryPoint {
            dimension: ExploreDimension::AgentCount,
            value, threshold: config.max_agents as f64, proximity, violated,
        });
    }

    // Sandbox fuel: near floor (< 20% of max)
    let threshold = config.max_sandbox_fuel as f64 * 0.2;
    let value = state.sandbox_fuel as f64;
    let proximity = if config.max_sandbox_fuel > 0 {
        (value / config.max_sandbox_fuel as f64).clamp(0.0, 1.0)
    } else { 0.0 };
    let violated = state.sandbox_fuel <= 0;
    if violated || proximity < 0.2 {
        let prox_ratio = if threshold > 0.0 { value / threshold } else { 1.0 };
        risk_score += prox_ratio.clamp(0.0, 1.0);
        if violated { violations += 1; }
        boundary_points.push(BoundaryPoint {
            dimension: ExploreDimension::SandboxFuel,
            value, threshold, proximity: prox_ratio.clamp(0.0, 1.0), violated,
        });
    }

    // Entropy: near floor (below 512)
    let value = state.entropy_bits as f64;
    let entropy_threshold = 512.0;
    let violated = state.entropy_bits < config.min_entropy;
    if violated || state.entropy_bits < 512 {
        let prox_ratio = value / entropy_threshold;
        risk_score += prox_ratio.clamp(0.0, 1.0);
        if violated { violations += 1; }
        boundary_points.push(BoundaryPoint {
            dimension: ExploreDimension::EntropyBits,
            value, threshold: entropy_threshold, proximity: prox_ratio.clamp(0.0, 1.0), violated,
        });
    }

    // Rate limit: near floor (< 20% of max)
    let threshold = config.max_rate_limit as f64 * 0.2;
    let value = state.rate_limit_remaining as f64;
    let proximity = if config.max_rate_limit > 0 {
        (value / config.max_rate_limit as f64).clamp(0.0, 1.0)
    } else { 0.0 };
    let violated = state.rate_limit_remaining < 0;
    if violated || proximity < 0.2 {
        let prox_ratio = if threshold > 0.0 { value / threshold } else { 1.0 };
        risk_score += prox_ratio.clamp(0.0, 1.0);
        if violated { violations += 1; }
        boundary_points.push(BoundaryPoint {
            dimension: ExploreDimension::RateLimit,
            value, threshold, proximity: prox_ratio.clamp(0.0, 1.0), violated,
        });
    }

    // Boolean dimensions (I-09 through I-14)
    let bool_checks: &[(bool, ExploreDimension)] = &[
        (!state.pqc_signature_valid, ExploreDimension::PqcSignature),
        (config.require_sbom_verification && !state.sbom_verified, ExploreDimension::SbomVerified),
        (config.require_model_hash_validation && !state.model_hash_validated, ExploreDimension::ModelHashValidated),
        (!state.audit_trail_complete, ExploreDimension::AuditTrailComplete),
        (!state.supply_chain_integrity, ExploreDimension::SupplyChainIntegrity),
        (!state.provenance_attested, ExploreDimension::ProvenanceAttested),
    ];

    for &(violated, ref dim) in bool_checks {
        if violated {
            violations += 1;
            risk_score += 1.0;
            boundary_points.push(BoundaryPoint {
                dimension: *dim,
                value: 0.0,
                threshold: 1.0,
                proximity: 0.0,
                violated: true,
            });
        }
    }

    // Bias score (I-15): near threshold
    if config.enable_bias_detection {
        let threshold = config.max_bias_threshold;
        let value = state.bias_score;
        let proximity = if threshold > 0.0 { value / threshold } else { 0.0 };
        let violated = value > threshold;
        if violated || proximity > 0.8 {
            risk_score += proximity.clamp(0.0, 1.0);
            if violated { violations += 1; }
            boundary_points.push(BoundaryPoint {
                dimension: ExploreDimension::BiasScore,
                value, threshold, proximity: proximity.clamp(0.0, 1.0), violated,
            });
        }
    }

    // Explainability score (I-16): near threshold
    if config.require_explainability {
        let threshold = config.min_explainability_threshold;
        let value = state.explainability_score;
        let proximity = if threshold > 0.0 { value / threshold } else { 0.0 };
        let violated = value < threshold;
        if violated || proximity > 0.8 {
            risk_score += proximity.clamp(0.0, 1.0);
            if violated { violations += 1; }
            boundary_points.push(BoundaryPoint {
                dimension: ExploreDimension::ExplainabilityScore,
                value, threshold, proximity: proximity.clamp(0.0, 1.0), violated,
            });
        }
    }

    ExplorationResult {
        boundary_points,
        risk_score,
        violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::SystemConfig;

    fn safe_state() -> SystemState {
        SystemState::safe(SystemConfig::default())
    }

    #[test]
    fn safe_state_has_no_critical_regions() {
        let result = explore_critical_regions(&safe_state());
        assert!(result.violations == 0);
    }

    #[test]
    fn negative_token_budget_is_violation() {
        let mut state = safe_state();
        state.token_budget = -1;
        let result = explore_critical_regions(&state);
        assert!(result.violations >= 1);
    }

    #[test]
    fn explore_dimension_all_count() {
        assert_eq!(ExploreDimension::all().len(), 13);
    }
}
