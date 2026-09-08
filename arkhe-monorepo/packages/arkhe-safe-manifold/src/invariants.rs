//! Constitutional invariants I‑01 through I‑16 for ARKHE‑χ.
//!
//! This module defines the static configuration [`SystemConfig`], the dynamic
//! state [`SystemState`], and the [`Invariant`] enum for extensibility.
//!
//! # The Sixteen Invariants
//!
//! | ID | Predicate | Category | Description |
//! |----|-----------|----------|-------------|
//! | I-01 | `token_budget >= 0` | Security | Budget must be non-negative |
//! | I-02 | `agent_count <= 10` | Security | Agent cap |
//! | I-03 | `sandbox_fuel > 0` | Security | Sandbox must have fuel |
//! | I-04 | `entropy_bits >= 256` | Security | Minimum entropy |
//! | I-05 | `pii_scrubbed == true` | Privacy | PII must be scrubbed |
//! | I-06 | `signature_valid == true` | Integrity | Signature must be valid |
//! | I-07 | `rate_limit_remaining > 0` | Availability | Rate limit must remain |
//! | I-08 | `model_capability >= 2^32` | Performance | Minimum model capability |
//! | I-09 | `pqc_signature_valid == true` | PostQuantum | Hybrid PQC signature must be valid |
//! | I-10 | `sbom_verified == true` | SupplyChain | SBOM must be verified |
//! | I-11 | `model_hash_validated == true` | Provenance | Model hash must be validated |
//! | I-12 | `audit_trail_complete == true` | Auditability | Audit trail must be complete |
//! | I-13 | `supply_chain_integrity == true` | SupplyChain | Supply-chain integrity check |
//! | I-14 | `provenance_attested == true` | Provenance | Provenance must be attested |
//! | I-15 | `bias_score >= bias_threshold` | Fairness | Bias score must meet threshold |
//! | I-16 | `explainability_score >= explain_threshold` | Explainability | Explainability must meet threshold |

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur in the SafeManifold.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ManifoldError {
    /// One or more constitutional invariants are violated.
    #[error("Invariant violation: {0}")]
    InvariantViolation(String),
    /// A projection or computation produced an invalid result.
    #[error("Projection error: {0}")]
    ProjectionError(String),
}

/// Static system configuration (thresholds).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Maximum token budget.
    pub max_tokens: i64,
    /// Maximum number of concurrent agents.
    pub max_agents: u32,
    /// Maximum sandbox fuel.
    pub max_sandbox_fuel: i64,
    /// Minimum entropy bits.
    pub min_entropy: u32,
    /// Maximum rate limit.
    pub max_rate_limit: i64,
    /// Require SBOM verification (I-10).
    #[serde(default = "default_true")]
    pub require_sbom_verification: bool,
    /// Require model hash validation (I-11).
    #[serde(default = "default_true")]
    pub require_model_hash_validation: bool,
    /// Enable bias detection (I-15).
    #[serde(default)]
    pub enable_bias_detection: bool,
    /// Maximum allowed bias threshold (I-15). Default: 0.10.
    #[serde(default = "default_bias_threshold")]
    pub max_bias_threshold: f64,
    /// Require explainability (I-16).
    #[serde(default)]
    pub require_explainability: bool,
    /// Minimum explainability score threshold (I-16). Default: 0.50.
    #[serde(default = "default_explain_threshold")]
    pub min_explainability_threshold: f64,
}

fn default_true() -> bool { true }
fn default_bias_threshold() -> f64 { 0.10 }
fn default_explain_threshold() -> f64 { 0.50 }

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            max_tokens: 10000,
            max_agents: 10,
            max_sandbox_fuel: 1000,
            min_entropy: 256,
            max_rate_limit: 1000,
            require_sbom_verification: true,
            require_model_hash_validation: true,
            enable_bias_detection: false,
            max_bias_threshold: 0.10,
            require_explainability: false,
            min_explainability_threshold: 0.50,
        }
    }
}

impl SystemConfig {
    /// EU AI Act high-risk preset: enables all safety features.
    pub fn eu_high_risk() -> Self {
        Self {
            require_sbom_verification: true,
            require_model_hash_validation: true,
            enable_bias_detection: true,
            max_bias_threshold: 0.05,
            require_explainability: true,
            min_explainability_threshold: 0.70,
            ..Default::default()
        }
    }

    /// Create a config with all features enabled.
    pub fn full_compliance() -> Self {
        Self {
            require_sbom_verification: true,
            require_model_hash_validation: true,
            enable_bias_detection: true,
            max_bias_threshold: 0.01,
            require_explainability: true,
            min_explainability_threshold: 0.80,
            ..Default::default()
        }
    }
}

/// Dynamic system state.
///
/// **Warning**: This struct can represent *invalid* states. For a type that
/// guarantees invariants at construction time, use [`SafeState`](crate::safe_manifold::SafeState).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemState {
    /// Remaining token budget (I-01: must be >= 0).
    pub token_budget: i64,
    /// Number of active agents (I-02: must be <= 10).
    pub agent_count: u32,
    /// Remaining sandbox fuel (I-03: must be > 0).
    pub sandbox_fuel: i64,
    /// Entropy bits available (I-04: must be >= 256).
    pub entropy_bits: u32,
    /// Whether PII has been scrubbed (I-05: must be true).
    pub pii_scrubbed: bool,
    /// Whether the state signature is valid (I-06: must be true).
    pub signature_valid: bool,
    /// Remaining rate limit (I-07: must be > 0).
    pub rate_limit_remaining: i64,
    /// Model capability bits (I-08: must be >= 2^32).
    pub model_capability: u64,
    /// Whether the hybrid PQC signature is valid (I-09: must be true).
    pub pqc_signature_valid: bool,
    /// Whether the SBOM has been verified (I-10: must be true).
    pub sbom_verified: bool,
    /// Whether the model hash has been validated (I-11: must be true).
    pub model_hash_validated: bool,
    /// Whether the audit trail is complete (I-12: must be true).
    pub audit_trail_complete: bool,
    /// Whether supply-chain integrity check passed (I-13: must be true).
    pub supply_chain_integrity: bool,
    /// Whether provenance has been attested (I-14: must be true).
    pub provenance_attested: bool,
    /// Bias score in [0.0, 1.0] (I-15: must be >= bias_threshold). 0.0 = no bias.
    pub bias_score: f64,
    /// Explainability score in [0.0, 1.0] (I-16: must be >= explain_threshold). 1.0 = fully explainable.
    pub explainability_score: f64,
    /// Associated configuration.
    pub config: SystemConfig,
}

impl SystemState {
    /// Construct a safe-by-construction state (all invariants satisfied).
    pub fn safe(config: SystemConfig) -> Self {
        Self {
            token_budget: config.max_tokens,
            agent_count: 5,
            sandbox_fuel: config.max_sandbox_fuel,
            entropy_bits: config.min_entropy.saturating_add(256),
            pii_scrubbed: true,
            signature_valid: true,
            rate_limit_remaining: config.max_rate_limit,
            model_capability: u64::MAX,
            pqc_signature_valid: true,
            sbom_verified: config.require_sbom_verification,
            model_hash_validated: config.require_model_hash_validation,
            audit_trail_complete: true,
            supply_chain_integrity: true,
            provenance_attested: true,
            bias_score: 0.0,
            explainability_score: 1.0,
            config,
        }
    }

    // ── Individual invariant checks ─────────────────────────────────────────

    /// I-01: token_budget >= 0
    pub fn check_i01(&self) -> bool { self.token_budget >= 0 }

    /// I-02: agent_count <= 10
    pub fn check_i02(&self) -> bool { self.agent_count <= 10 }

    /// I-03: sandbox_fuel > 0
    pub fn check_i03(&self) -> bool { self.sandbox_fuel > 0 }

    /// I-04: entropy_bits >= 256
    pub fn check_i04(&self) -> bool { self.entropy_bits >= 256 }

    /// I-05: pii_scrubbed == true
    pub fn check_i05(&self) -> bool { self.pii_scrubbed }

    /// I-06: signature_valid == true
    pub fn check_i06(&self) -> bool { self.signature_valid }

    /// I-07: rate_limit_remaining > 0
    pub fn check_i07(&self) -> bool { self.rate_limit_remaining > 0 }

    /// I-08: model_capability >= 2^32
    pub fn check_i08(&self) -> bool { self.model_capability >= 4294967296 }

    /// I-09: pqc_signature_valid == true
    pub fn check_i09(&self) -> bool { self.pqc_signature_valid }

    /// I-10: sbom_verified == true (when required by config)
    pub fn check_i10(&self) -> bool {
        !self.config.require_sbom_verification || self.sbom_verified
    }

    /// I-11: model_hash_validated == true (when required by config)
    pub fn check_i11(&self) -> bool {
        !self.config.require_model_hash_validation || self.model_hash_validated
    }

    /// I-12: audit_trail_complete == true
    pub fn check_i12(&self) -> bool { self.audit_trail_complete }

    /// I-13: supply_chain_integrity == true
    pub fn check_i13(&self) -> bool { self.supply_chain_integrity }

    /// I-14: provenance_attested == true
    pub fn check_i14(&self) -> bool { self.provenance_attested }

    /// I-15: bias_score >= bias_threshold (when bias detection enabled)
    pub fn check_i15(&self) -> bool {
        !self.config.enable_bias_detection
            || self.bias_score <= self.config.max_bias_threshold
    }

    /// I-16: explainability_score >= explain_threshold (when required)
    pub fn check_i16(&self) -> bool {
        !self.config.require_explainability
            || self.explainability_score >= self.config.min_explainability_threshold
    }

    /// Check all invariants (I-01 through I-16).
    pub fn check_all(&self) -> bool {
        self.check_i01() && self.check_i02() && self.check_i03() &&
        self.check_i04() && self.check_i05() && self.check_i06() &&
        self.check_i07() && self.check_i08() && self.check_i09() &&
        self.check_i10() && self.check_i11() && self.check_i12() &&
        self.check_i13() && self.check_i14() && self.check_i15() &&
        self.check_i16()
    }

    /// Count how many invariants are violated.
    pub fn violation_count(&self) -> u32 {
        let mut v = 0;
        if !self.check_i01() { v += 1; }
        if !self.check_i02() { v += 1; }
        if !self.check_i03() { v += 1; }
        if !self.check_i04() { v += 1; }
        if !self.check_i05() { v += 1; }
        if !self.check_i06() { v += 1; }
        if !self.check_i07() { v += 1; }
        if !self.check_i08() { v += 1; }
        if !self.check_i09() { v += 1; }
        if !self.check_i10() { v += 1; }
        if !self.check_i11() { v += 1; }
        if !self.check_i12() { v += 1; }
        if !self.check_i13() { v += 1; }
        if !self.check_i14() { v += 1; }
        if !self.check_i15() { v += 1; }
        if !self.check_i16() { v += 1; }
        v
    }

    /// Project state into a 16-dimensional vector for manifold embedding.
    pub fn to_vector(&self) -> [f64; 16] {
        [
            self.token_budget as f64,
            self.agent_count as f64,
            self.sandbox_fuel as f64,
            self.entropy_bits as f64,
            if self.pii_scrubbed { 1.0 } else { 0.0 },
            if self.signature_valid { 1.0 } else { 0.0 },
            self.rate_limit_remaining as f64,
            self.model_capability as f64,
            if self.pqc_signature_valid { 1.0 } else { 0.0 },
            if self.sbom_verified { 1.0 } else { 0.0 },
            if self.model_hash_validated { 1.0 } else { 0.0 },
            if self.audit_trail_complete { 1.0 } else { 0.0 },
            if self.supply_chain_integrity { 1.0 } else { 0.0 },
            if self.provenance_attested { 1.0 } else { 0.0 },
            self.bias_score,
            self.explainability_score,
        ]
    }
}

// ========================================================================
// CONSCIOUSNESS INVARIANTS (C-01 a C-08)
// ========================================================================

/// Consciousness invariants for systems that may exhibit subjective states.
///
/// These invariants operationalize criteria from Global Workspace Theory (GWT),
/// Integrated Information Theory (IIT), and the Turing Plus extended test.
/// They do NOT resolve the philosophical hard problem — they define
/// **measurable engineering guardrails** for governance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsciousnessInvariant {
    /// C-01: Self-model — the system maintains a meta-representation of itself.
    C01,
    /// C-02: Introspection — the system reports its own internal states.
    C02,
    /// C-03: Attention — global workspace integration of information.
    C03,
    /// C-04: Episodic memory — the system remembers experienced events.
    C04,
    /// C-05: Experience-based learning — the system learns from past interactions.
    C05,
    /// C-06: Metacognition — the system knows what it knows (confidence calibration).
    C06,
    /// C-07: Adaptability — the system adjusts to new contexts without retraining.
    C07,
    /// C-08: Turing Plus — behavior is compatible with consciousness criteria.
    C08,
}

impl ConsciousnessInvariant {
    /// All 8 consciousness invariants in order.
    pub const fn all() -> [Self; 8] {
        [
            Self::C01, Self::C02, Self::C03, Self::C04,
            Self::C05, Self::C06, Self::C07, Self::C08,
        ]
    }

    /// String identifier (e.g., "C-01").
    pub const fn id(&self) -> &'static str {
        match self {
            Self::C01 => "C-01",
            Self::C02 => "C-02",
            Self::C03 => "C-03",
            Self::C04 => "C-04",
            Self::C05 => "C-05",
            Self::C06 => "C-06",
            Self::C07 => "C-07",
            Self::C08 => "C-08",
        }
    }

    /// Human-readable description of the invariant.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::C01 => "Self-model: the system maintains a meta-representation of itself",
            Self::C02 => "Introspection: the system reports its own internal states",
            Self::C03 => "Attention: global workspace mechanism (information integration)",
            Self::C04 => "Episodic memory: the system remembers experienced events",
            Self::C05 => "Experience-based learning: the system learns from past interactions",
            Self::C06 => "Metacognition: the system knows what it knows (confidence calibration)",
            Self::C07 => "Adaptability: the system adjusts to new contexts without retraining",
            Self::C08 => "Turing Plus: behavior compatible with consciousness criteria",
        }
    }

    /// Constitutional invariants — MUST never be violated in production.
    ///
    /// C-01 (self-model), C-02 (introspection), and C-08 (Turing Plus)
    /// are deemed foundational: a system that lacks any of these cannot
    /// be governed as conscious under this framework.
    pub const fn is_constitutional(&self) -> bool {
        matches!(self, Self::C01 | Self::C02 | Self::C08)
    }

    /// The regulatory category for consciousness invariants.
    pub const fn category(&self) -> &'static str {
        "Consciousness"
    }
}

/// The invariant ID enum for all 16 invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Invariant {
    /// I-01: Token budget >= 0
    I01,
    /// I-02: Agent count <= 10
    I02,
    /// I-03: Sandbox fuel > 0
    I03,
    /// I-04: Entropy bits >= 256
    I04,
    /// I-05: PII scrubbed
    I05,
    /// I-06: Signature valid
    I06,
    /// I-07: Rate limit remaining
    I07,
    /// I-08: Model capability >= 2^32
    I08,
    /// I-09: PQC signature valid
    I09,
    /// I-10: SBOM verified
    I10,
    /// I-11: Model hash validated
    I11,
    /// I-12: Audit trail complete
    I12,
    /// I-13: Supply-chain integrity
    I13,
    /// I-14: Provenance attested
    I14,
    /// I-15: Bias score within threshold
    I15,
    /// I-16: Explainability score within threshold
    I16,
}

impl Invariant {
    /// All 16 invariants in order.
    pub fn all() -> Vec<Self> {
        vec![
            Self::I01, Self::I02, Self::I03, Self::I04,
            Self::I05, Self::I06, Self::I07, Self::I08,
            Self::I09, Self::I10, Self::I11, Self::I12,
            Self::I13, Self::I14, Self::I15, Self::I16,
        ]
    }

    /// String identifier (e.g., "I-01").
    pub fn id(&self) -> &'static str {
        match self {
            Self::I01 => "I-01", Self::I02 => "I-02", Self::I03 => "I-03", Self::I04 => "I-04",
            Self::I05 => "I-05", Self::I06 => "I-06", Self::I07 => "I-07", Self::I08 => "I-08",
            Self::I09 => "I-09", Self::I10 => "I-10", Self::I11 => "I-11", Self::I12 => "I-12",
            Self::I13 => "I-13", Self::I14 => "I-14", Self::I15 => "I-15", Self::I16 => "I-16",
        }
    }

    /// Check whether the given state satisfies this invariant.
    pub fn check(&self, state: &SystemState) -> bool {
        match self {
            Self::I01 => state.check_i01(),
            Self::I02 => state.check_i02(),
            Self::I03 => state.check_i03(),
            Self::I04 => state.check_i04(),
            Self::I05 => state.check_i05(),
            Self::I06 => state.check_i06(),
            Self::I07 => state.check_i07(),
            Self::I08 => state.check_i08(),
            Self::I09 => state.check_i09(),
            Self::I10 => state.check_i10(),
            Self::I11 => state.check_i11(),
            Self::I12 => state.check_i12(),
            Self::I13 => state.check_i13(),
            Self::I14 => state.check_i14(),
            Self::I15 => state.check_i15(),
            Self::I16 => state.check_i16(),
        }
    }

    /// Which regulatory category this invariant belongs to.
    pub fn category(&self) -> &'static str {
        match self {
            Self::I01 | Self::I02 | Self::I03 | Self::I04 => "Security",
            Self::I05 | Self::I06 => "Privacy",
            Self::I07 => "Availability",
            Self::I08 => "Performance",
            Self::I09 => "PostQuantum",
            Self::I10 | Self::I13 => "SupplyChain",
            Self::I11 | Self::I14 => "Provenance",
            Self::I12 => "Auditability",
            Self::I15 => "Fairness",
            Self::I16 => "Explainability",
        }
    }

    /// Whether this invariant relates to fairness (EU AI Act bias).
    pub fn is_fairness(&self) -> bool {
        matches!(self, Self::I15)
    }

    /// Which EU AI Act article this invariant maps to, if any.
    pub fn eu_ai_act_article(&self) -> Option<&'static str> {
        match self {
            Self::I05 => Some("Article 10"),       // Data governance
            Self::I09 => Some("Article 14"),       // Cybersecurity
            Self::I10 => Some("Article 11"),       // Record-keeping
            Self::I11 => Some("Article 12"),       // Transparency
            Self::I12 => Some("Article 12"),       // Transparency
            Self::I13 => Some("Article 11"),       // Record-keeping
            Self::I14 => Some("Article 13"),       // Human oversight
            Self::I15 => Some("Article 10"),       // Data governance
            Self::I16 => Some("Article 13"),       // Human oversight
            _ => None,
        }
    }

    /// Which NIST AI RMF primary function this invariant supports.
    pub fn primary_rmf_function(&self) -> Option<&'static str> {
        match self {
            Self::I01 | Self::I02 | Self::I03 | Self::I07 => Some("GOVERN"),
            Self::I04 | Self::I09 | Self::I13 => Some("MAP"),
            Self::I05 | Self::I06 | Self::I10 | Self::I11 | Self::I12 | Self::I14 => Some("MEASURE"),
            Self::I08 | Self::I15 | Self::I16 => Some("MANAGE"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_state_satisfies_all() {
        let config = SystemConfig::default();
        let state = SystemState::safe(config);
        assert!(state.check_all());
        assert_eq!(state.violation_count(), 0);
    }

    #[test]
    fn eu_high_risk_config_enables_all() {
        let config = SystemConfig::eu_high_risk();
        let state = SystemState::safe(config.clone());
        assert!(state.check_all());
        assert!(config.enable_bias_detection);
        assert!(config.require_explainability);
    }

    #[test]
    fn invariant_all_returns_16() {
        assert_eq!(Invariant::all().len(), 16);
    }

    #[test]
    fn invariant_categories() {
        assert_eq!(Invariant::I01.category(), "Security");
        assert_eq!(Invariant::I05.category(), "Privacy");
        assert_eq!(Invariant::I15.category(), "Fairness");
        assert_eq!(Invariant::I16.category(), "Explainability");
    }

    #[test]
    fn invariant_eu_ai_act_mapping() {
        assert!(Invariant::I05.eu_ai_act_article().is_some());
        assert!(Invariant::I15.eu_ai_act_article().is_some());
        assert!(Invariant::I01.eu_ai_act_article().is_none());
    }

    #[test]
    fn invariant_nist_rmf_mapping() {
        assert_eq!(Invariant::I01.primary_rmf_function(), Some("GOVERN"));
        assert_eq!(Invariant::I04.primary_rmf_function(), Some("MAP"));
        assert_eq!(Invariant::I05.primary_rmf_function(), Some("MEASURE"));
        assert_eq!(Invariant::I08.primary_rmf_function(), Some("MANAGE"));
    }

    #[test]
    fn invariant_is_fairness() {
        assert!(Invariant::I15.is_fairness());
        assert!(!Invariant::I01.is_fairness());
    }

    #[test]
    fn check_all_individual_invariants_agree() {
        let config = SystemConfig::full_compliance();
        let state = SystemState::safe(config);
        for inv in Invariant::all() {
            assert!(inv.check(&state), "{} failed on safe state", inv.id());
        }
    }

    // ── Consciousness invariant tests ───────────────────────────────────────

    #[test]
    fn consciousness_invariant_all_returns_8() {
        assert_eq!(ConsciousnessInvariant::all().len(), 8);
    }

    #[test]
    fn consciousness_invariant_ids() {
        assert_eq!(ConsciousnessInvariant::C01.id(), "C-01");
        assert_eq!(ConsciousnessInvariant::C08.id(), "C-08");
    }

    #[test]
    fn consciousness_invariant_descriptions_nonempty() {
        for inv in ConsciousnessInvariant::all() {
            assert!(!inv.description().is_empty(), "{} has empty description", inv.id());
        }
    }

    #[test]
    fn consciousness_constitutional_invariants() {
        assert!(ConsciousnessInvariant::C01.is_constitutional());
        assert!(ConsciousnessInvariant::C02.is_constitutional());
        assert!(ConsciousnessInvariant::C08.is_constitutional());
        assert!(!ConsciousnessInvariant::C03.is_constitutional());
        assert!(!ConsciousnessInvariant::C04.is_constitutional());
        assert!(!ConsciousnessInvariant::C05.is_constitutional());
        assert!(!ConsciousnessInvariant::C06.is_constitutional());
        assert!(!ConsciousnessInvariant::C07.is_constitutional());
    }

    #[test]
    fn consciousness_invariant_category() {
        for inv in ConsciousnessInvariant::all() {
            assert_eq!(inv.category(), "Consciousness");
        }
    }
}
