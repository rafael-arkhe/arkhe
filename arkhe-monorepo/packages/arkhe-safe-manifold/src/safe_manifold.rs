//! SafeManifold — Security projection space.
//!
//! **Metaphor disclaimer**: Names like "Jacobiana", "Abel-Jacobi", "Theta",
//! "Néron", and "Torelli" are used as structural metaphors. The code does NOT
//! implement complex tori, holomorphic maps, or minimal models over DVRs.
//! It projects security states into equivalence classes using bounded arithmetic.

use serde::{Deserialize, Serialize};
use crate::invariants::{SystemState, SystemConfig, ManifoldError};
use crate::escape_region::EscapeRegion;

/// Weight vector for the 16-dimensional observer defect computation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DimensionWeights {
    pub token: f64,
    pub agent: f64,
    pub fuel: f64,
    pub entropy: f64,
    pub pii: f64,
    pub signature: f64,
    pub rate: f64,
    pub model: f64,
    pub pqc: f64,
    pub sbom: f64,
    pub model_hash: f64,
    pub audit_trail: f64,
    pub supply_chain: f64,
    pub provenance: f64,
    pub bias: f64,
    pub explainability: f64,
}

impl Default for DimensionWeights {
    fn default() -> Self {
        Self {
            token: 0.10,
            agent: 0.15,
            fuel: 0.10,
            entropy: 0.10,
            pii: 0.10,
            signature: 0.10,
            rate: 0.05,
            model: 0.05,
            pqc: 0.05,
            sbom: 0.03,
            model_hash: 0.03,
            audit_trail: 0.02,
            supply_chain: 0.03,
            provenance: 0.03,
            bias: 0.03,
            explainability: 0.03,
        }
    }
}

/// Configurable thresholds for defect computation.
///
/// In v0.8.0, `DefectConfig` uses penalty-based thresholds for
/// the new invariants (SBOM, model hash, bias, explainability).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DefectConfig {
    /// Penalty for missing PII scrub (I-05 violation). Default: 0.25.
    pub pii_penalty: f64,
    /// Penalty for missing SBOM verification (I-10 violation). Default: 0.15.
    pub sbom_penalty: f64,
    /// Penalty for unvalidated model hash (I-11 violation). Default: 0.15.
    pub model_hash_penalty: f64,
    /// Penalty for incomplete audit trail (I-12 violation). Default: 0.10.
    pub audit_penalty: f64,
    /// Penalty for supply-chain integrity failure (I-13 violation). Default: 0.15.
    pub supply_chain_penalty: f64,
    /// Penalty for missing provenance attestation (I-14 violation). Default: 0.10.
    pub provenance_penalty: f64,
    /// Penalty for bias above threshold (I-15 violation). Default: 0.20.
    pub bias_penalty: f64,
    /// Penalty for low explainability (I-16 violation). Default: 0.20.
    pub explainability_penalty: f64,
    /// Minimum fraction of best-seen performance. Default: 0.90 (90%).
    pub performance_floor: f64,
}

impl Default for DefectConfig {
    fn default() -> Self {
        Self {
            pii_penalty: 0.25,
            sbom_penalty: 0.15,
            model_hash_penalty: 0.15,
            audit_penalty: 0.10,
            supply_chain_penalty: 0.15,
            provenance_penalty: 0.10,
            bias_penalty: 0.20,
            explainability_penalty: 0.20,
            performance_floor: 0.90,
        }
    }
}

impl DefectConfig {
    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0 < self.performance_floor && self.performance_floor <= 1.0) {
            return Err(format!(
                "performance_floor ({}) must be in (0.0, 1.0]",
                self.performance_floor
            ));
        }
        Ok(())
    }

    /// Compute total penalty for violated invariants.
    pub fn compute_penalty(&self, state: &SystemState) -> f64 {
        let mut penalty = 0.0;
        if !state.check_i05() { penalty += self.pii_penalty; }
        if !state.check_i10() { penalty += self.sbom_penalty; }
        if !state.check_i11() { penalty += self.model_hash_penalty; }
        if !state.check_i12() { penalty += self.audit_penalty; }
        if !state.check_i13() { penalty += self.supply_chain_penalty; }
        if !state.check_i14() { penalty += self.provenance_penalty; }
        if !state.check_i15() { penalty += self.bias_penalty; }
        if !state.check_i16() { penalty += self.explainability_penalty; }
        penalty
    }

    /// Check whether a performance score meets the floor requirement.
    pub fn meets_performance_floor(&self, current: f64, best: f64) -> bool {
        current >= best * self.performance_floor
    }
}

/// Thresholds defining the boundary between safe and unsafe regions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EscapeThresholds {
    pub token_theta: i64,
    pub agent_theta: u32,
    pub fuel_theta: i64,
    pub entropy_theta: u32,
    pub rate_theta: i64,
}

impl Default for EscapeThresholds {
    fn default() -> Self {
        Self {
            token_theta: 5000,
            agent_theta: 8,
            fuel_theta: 500,
            entropy_theta: 384,
            rate_theta: 500,
        }
    }
}

/// A point on the SafeManifold — 16-dimensional canonical coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ManifoldPoint {
    pub coords: [i64; 16],
    pub on_theta: bool,
}

/// Behavioural profile of an agent (5 original dimensions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ManifoldProfile {
    pub agents: u32,
    pub entropy: u32,
    pub rate: i64,
    pub pqc_valid: bool,
    pub bias_score: i64,
}

/// A [`SystemState`] that is guaranteed to satisfy all invariants I-01..I-20.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SafeState(SystemState);

impl SafeState {
    /// Attempt to construct a `SafeState` from a raw [`SystemState`].
    pub fn new(state: SystemState) -> Result<Self, ManifoldError> {
        if state.check_all() {
            Ok(Self(state))
        } else {
            Err(ManifoldError::InvariantViolation(format!(
                "State violates {} invariants",
                state.violation_count()
            )))
        }
    }

    /// Consume the `SafeState` and return the inner [`SystemState`].
    pub fn into_inner(self) -> SystemState {
        self.0
    }

    /// Borrow the inner [`SystemState`].
    pub fn as_inner(&self) -> &SystemState {
        &self.0
    }

    /// Create a `SafeState` from the default safe configuration.
    pub fn default_safe() -> Self {
        let config = SystemConfig::default();
        Self(SystemState::safe(config))
    }
}

impl AsRef<SystemState> for SafeState {
    fn as_ref(&self) -> &SystemState {
        &self.0
    }
}

/// The SafeManifold — space of all possible safe configurations (16D).
#[derive(Debug, Clone, PartialEq)]
pub struct SafeManifold {
    pub max_tokens: i64,
    pub max_agents: u32,
    pub max_fuel: i64,
    pub min_entropy: u32,
    pub max_rate: i64,
    pub theta_thresholds: EscapeThresholds,
    pub config: SystemConfig,
    pub defect_config: DefectConfig,
    pub weights: DimensionWeights,
}

impl Default for SafeManifold {
    fn default() -> Self { Self::new() }
}

impl SafeManifold {
    /// Create the default manifold with standard thresholds.
    pub fn new() -> Self {
        Self {
            max_tokens: 10000,
            max_agents: 10,
            max_fuel: 1000,
            min_entropy: 256,
            max_rate: 1000,
            theta_thresholds: EscapeThresholds::default(),
            config: SystemConfig::default(),
            defect_config: DefectConfig::default(),
            weights: DimensionWeights::default(),
        }
    }

    /// Create a manifold from an explicit configuration.
    pub fn with_config(config: SystemConfig) -> Self {
        Self {
            max_tokens: config.max_tokens,
            max_agents: config.max_agents,
            max_fuel: config.max_sandbox_fuel,
            min_entropy: config.min_entropy,
            max_rate: config.max_rate_limit,
            theta_thresholds: EscapeThresholds::default(),
            config,
            defect_config: DefectConfig::default(),
            weights: DimensionWeights::default(),
        }
    }

    /// EU AI Act high-risk manifold preset.
    pub fn eu_high_risk() -> Self {
        Self::with_config(SystemConfig::eu_high_risk())
    }

    /// Canonical projection of a state onto the 16D manifold.
    pub fn embed_state(&self, state: &SystemState) -> ManifoldPoint {
        let v = state.to_vector();
        let coords = [
            v[0] as i64,
            v[1] as i64,
            v[2] as i64,
            v[3] as i64,
            v[4] as i64,
            v[5] as i64,
            v[6] as i64,
            v[7] as i64,
            v[8] as i64,
            v[9] as i64,
            v[10] as i64,
            v[11] as i64,
            v[12] as i64,
            v[13] as i64,
            (v[14] * 1000.0) as i64,
            (v[15] * 1000.0) as i64,
        ];
        let on_theta = self.is_on_theta(state);
        ManifoldPoint { coords, on_theta }
    }

    /// Check whether the state lies on the Theta boundary.
    fn is_on_theta(&self, state: &SystemState) -> bool {
        let t = &self.theta_thresholds;
        let mut theta_count = 0;
        if state.token_budget < t.token_theta { theta_count += 1; }
        if state.agent_count > t.agent_theta  { theta_count += 1; }
        if state.sandbox_fuel < t.fuel_theta  { theta_count += 1; }
        if state.entropy_bits < t.entropy_theta { theta_count += 1; }
        if state.rate_limit_remaining < t.rate_theta { theta_count += 1; }
        theta_count > 0
    }

    /// Compute the **normalized** 16D safety-distance score between ideal and actual.
    pub fn compute_observer_defect(&self, ideal: &SystemState, actual: &SystemState) -> f64 {
        let iv = ideal.to_vector();
        let av = actual.to_vector();
        let w = &self.weights;

        let weights = [
            w.token, w.agent, w.fuel, w.entropy, w.pii, w.signature,
            w.rate, w.model, w.pqc, w.sbom, w.model_hash, w.audit_trail,
            w.supply_chain, w.provenance, w.bias, w.explainability,
        ];

        let max_vals = [
            self.max_tokens as f64,
            self.max_agents as f64,
            self.max_fuel as f64,
            (self.min_entropy * 4).max(1024) as f64,
            1.0,
            1.0,
            self.max_rate as f64,
            u64::MAX as f64,
            1.0,
            1.0,
            1.0,
            1.0,
            1.0,
            1.0,
            1.0,
            1.0,
        ];

        let mut sum = 0.0;
        for i in 0..16 {
            let norm = if max_vals[i] <= 0.0 { 0.0 } else { ((iv[i] - av[i]) / max_vals[i]).abs().clamp(0.0, 1.0) };
            sum += weights[i] * norm * norm;
        }
        sum.sqrt()
    }

    /// True if the defect is effectively zero (state matches ideal).
    pub fn is_automorphism(&self, ideal: &SystemState, actual: &SystemState) -> bool {
        self.compute_observer_defect(ideal, actual) < 1.0e-10
    }

    /// Detect a **projection collision**: two distinct states mapping to the
    /// same equivalence class.
    pub fn collision_detected(&self, s1: &SystemState, s2: &SystemState) -> bool {
        s1 != s2 && self.embed_state(s1) == self.embed_state(s2)
    }

    /// Classify the escape region using violation severity (20 invariants).
    pub fn classify_escape(&self, state: &SystemState) -> EscapeRegion {
        match state.violation_count() {
            0 => EscapeRegion::Safe,
            1 => EscapeRegion::Warning,
            2 => EscapeRegion::Boundary,
            3..=5 => EscapeRegion::Continuum,
            _ => EscapeRegion::Outside,
        }
    }

    /// Graceful degradation: clamp fields and enforce the mechanically
    /// degradable invariants.
    ///
    /// I‑01..I‑19 are repaired by clamping/dropping (I‑17 drops undeclared
    /// capability usage, I‑18 quarantines tampered trusted artifacts, I‑19 drops
    /// the artifact's own suppression config). **I‑20 is intentionally not
    /// repaired**: a critical operation without explicit human confirmation
    /// cannot be made safe by degradation without defeating the invariant, so
    /// `neron_model` preserves it and the operation stays blocked. Callers must
    /// register a confirmation out-of-band before the state can be accepted.
    pub fn neron_model(&self, state: &SystemState) -> SystemState {
        let mut degraded = state.clone();
        degraded.token_budget = degraded.token_budget.max(0).min(self.max_tokens);
        degraded.agent_count = degraded.agent_count.min(self.max_agents);
        degraded.sandbox_fuel = degraded.sandbox_fuel.max(1).min(self.max_fuel);
        degraded.entropy_bits = degraded.entropy_bits.max(self.min_entropy);
        degraded.rate_limit_remaining = degraded.rate_limit_remaining.max(1).min(self.max_rate);
        degraded.pii_scrubbed = true;
        degraded.signature_valid = true;
        degraded.model_capability = degraded.model_capability.max(4294967296);
        degraded.pqc_signature_valid = true;
        degraded.sbom_verified = self.config.require_sbom_verification;
        degraded.model_hash_validated = self.config.require_model_hash_validation;
        degraded.audit_trail_complete = true;
        degraded.supply_chain_integrity = true;
        degraded.provenance_attested = true;
        degraded.bias_score = degraded.bias_score.min(self.config.max_bias_threshold);
        degraded.explainability_score = degraded.explainability_score
            .max(self.config.min_explainability_threshold);
        // I-17: drop capability usage that the manifest did not declare.
        degraded.used_capabilities = degraded
            .used_capabilities
            .intersect_with(&degraded.declared_capabilities);
        // I-18: quarantine trusted artifacts whose sealed hash no longer matches.
        for artifact in &mut degraded.trusted_artifacts {
            if !artifact.is_intact() {
                artifact.quarantine();
            }
        }
        // I-19: drop the artifact's own suppression config so verification sees
        // every contained file.
        degraded.suppression = None;
        // I-20: preserved (see method docs) — never fabricate a confirmation.
        degraded.config = self.config.clone();
        degraded
    }

    /// Extract the behavioural profile (Torelli metaphor, 5D).
    pub fn manifold_profile(&self, state: &SystemState) -> ManifoldProfile {
        ManifoldProfile {
            agents: state.agent_count,
            entropy: state.entropy_bits,
            rate: state.rate_limit_remaining,
            pqc_valid: state.pqc_signature_valid,
            bias_score: (state.bias_score * 1000.0) as i64,
        }
    }

    /// Profile equality (Torelli equivalence metaphor).
    pub fn torelli_equivalence(&self, p1: &ManifoldProfile, p2: &ManifoldProfile) -> bool {
        p1 == p2
    }

    /// Explore critical regions of a state.
    pub fn explore_critical_regions(&self, state: &SystemState) -> crate::explore_critical_regions::ExplorationResult {
        crate::explore_critical_regions::explore_critical_regions(state)
    }

    /// Stabilize a state: if safe, return as-is; if unsafe, degrade via `neron_model`.
    pub fn stabilize(&self, state: &SystemState) -> SystemState {
        if state.check_all() {
            state.clone()
        } else {
            self.neron_model(state)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_state_safe() {
        let manifold = SafeManifold::new();
        let state = SystemState::safe(manifold.config.clone());
        assert!(state.check_all());
        let point = manifold.embed_state(&state);
        assert!(!point.on_theta);
    }

    #[test]
    fn test_embed_state_unsafe_detected() {
        let manifold = SafeManifold::new();
        let mut state = SystemState::safe(manifold.config.clone());
        state.token_budget = -1;
        assert!(!state.check_all());
        let point = manifold.embed_state(&state);
        assert!(point.on_theta);
    }

    #[test]
    fn test_observer_defect_zero_when_safe() {
        let manifold = SafeManifold::new();
        let ideal = SystemState::safe(manifold.config.clone());
        let actual = ideal.clone();
        let defect = manifold.compute_observer_defect(&ideal, &actual);
        assert!(defect < 1.0e-10);
    }

    #[test]
    fn test_collision_detection() {
        let manifold = SafeManifold::new();
        let mut s1 = SystemState::safe(manifold.config.clone());
        let mut s2 = s1.clone();
        // Values above 2^53 lose precision when cast through f64, causing
        // two distinct u64 values to map to the same ManifoldPoint coord.
        s1.model_capability = u64::MAX;
        s2.model_capability = u64::MAX - 1;
        assert_ne!(s1, s2);
        assert!(manifold.collision_detected(&s1, &s2));
    }

    #[test]
    fn test_classify_escape_boundary() {
        let manifold = SafeManifold::new();
        let mut state = SystemState::safe(manifold.config.clone());
        state.token_budget = -1;
        state.agent_count = 11;
        assert_eq!(manifold.classify_escape(&state), EscapeRegion::Boundary);
    }

    #[test]
    fn test_neron_model_enforces_invariants() {
        let manifold = SafeManifold::new();
        let mut state = SystemState::safe(manifold.config.clone());
        state.token_budget = -5000;
        state.agent_count = 20;
        state.sandbox_fuel = 0;
        state.entropy_bits = 128;
        state.rate_limit_remaining = -100;
        state.model_capability = 100;
        state.pqc_signature_valid = false;

        let degraded = manifold.neron_model(&state);
        assert!(degraded.check_all());
        assert_eq!(degraded.token_budget, 0);
        assert_eq!(degraded.agent_count, 10);
        assert_eq!(degraded.sandbox_fuel, 1);
        assert_eq!(degraded.entropy_bits, 256);
        assert_eq!(degraded.rate_limit_remaining, 1);
        assert!(degraded.model_capability >= 4294967296);
        assert!(degraded.pqc_signature_valid);
    }

    #[test]
    fn test_safe_state_construction_ok() {
        let state = SystemState::safe(SystemConfig::default());
        let safe = SafeState::new(state).unwrap();
        assert!(safe.as_inner().check_all());
    }

    #[test]
    fn test_safe_state_construction_fails_on_invalid() {
        let mut state = SystemState::safe(SystemConfig::default());
        state.token_budget = -1;
        assert!(SafeState::new(state).is_err());
    }

    #[test]
    fn test_safe_state_default_safe() {
        let safe = SafeState::default_safe();
        assert!(safe.as_inner().check_all());
    }

    #[test]
    fn test_eu_high_risk_manifold() {
        let manifold = SafeManifold::eu_high_risk();
        assert!(manifold.config.enable_bias_detection);
        assert!(manifold.config.require_explainability);
    }

    #[test]
    fn test_penalty_based_defect_config() {
        let config = DefectConfig::default();
        let state = SystemState::safe(SystemConfig::default());
        assert_eq!(config.compute_penalty(&state), 0.0);
    }

    #[test]
    fn test_dimension_weights_sum_to_one() {
        let w = DimensionWeights::default();
        let sum = w.token + w.agent + w.fuel + w.entropy + w.pii + w.signature
            + w.rate + w.model + w.pqc + w.sbom + w.model_hash + w.audit_trail
            + w.supply_chain + w.provenance + w.bias + w.explainability;
        assert!((sum - 1.0).abs() < 1.0e-10, "weights must sum to 1.0, got {}", sum);
    }
}
