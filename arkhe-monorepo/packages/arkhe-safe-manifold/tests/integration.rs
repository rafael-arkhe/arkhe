use arkhe_safe_manifold::*;

// ═════════════════════════════════════════════════════════════════════════════
// Unit-style integration tests (v0.8.0 — 16 invariants)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_manifold_embedding_preserves_invariants() {
    let manifold = SafeManifold::new();
    let state = SystemState::safe(manifold.config.clone());
    assert!(state.check_all());
    let point = manifold.embed_state(&state);
    assert!(!point.on_theta);
}

#[test]
fn test_observer_defect_scaling() {
    let manifold = SafeManifold::new();
    let ideal = SystemState::safe(manifold.config.clone());

    let mut far = ideal.clone();
    far.token_budget = 0;
    let defect_far = manifold.compute_observer_defect(&ideal, &far);

    let mut near = ideal.clone();
    near.token_budget = 9000;
    let defect_near = manifold.compute_observer_defect(&ideal, &near);

    assert!(defect_far > defect_near);
}

#[test]
fn test_collision_detected_on_distinct_states() {
    let manifold = SafeManifold::new();
    let mut s1 = SystemState::safe(manifold.config.clone());
    let mut s2 = s1.clone();
    s1.model_capability = u64::MAX;
    s2.model_capability = u64::MAX - 1;
    assert_ne!(s1, s2);
    assert!(manifold.collision_detected(&s1, &s2));
}

#[test]
fn test_neron_model_degrades_gracefully() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.agent_count = 20;
    let degraded = manifold.neron_model(&state);
    assert_eq!(degraded.agent_count, 10);
    assert!(degraded.check_all());
}

#[test]
fn test_classify_escape_warning() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.token_budget = -1;
    assert_eq!(manifold.classify_escape(&state), EscapeRegion::Warning);
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
fn test_classify_escape_continuum() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.token_budget = -1;
    state.agent_count = 11;
    state.sandbox_fuel = 0;
    state.entropy_bits = 128;
    assert_eq!(manifold.classify_escape(&state), EscapeRegion::Continuum);
}

#[test]
fn test_classify_escape_outside() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.token_budget = -1;
    state.agent_count = 11;
    state.sandbox_fuel = 0;
    state.entropy_bits = 128;
    state.pii_scrubbed = false;
    state.signature_valid = false;
    assert_eq!(manifold.classify_escape(&state), EscapeRegion::Outside);
}

#[test]
fn test_neron_model_enforces_all_invariants() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.token_budget = -9999;
    state.agent_count = 999;
    state.sandbox_fuel = -1;
    state.entropy_bits = 0;
    state.rate_limit_remaining = -1;
    state.model_capability = 1;
    state.pii_scrubbed = false;
    state.signature_valid = false;
    state.pqc_signature_valid = false;
    state.bias_score = 0.5;

    let degraded = manifold.neron_model(&state);
    assert!(degraded.check_all());
    assert_eq!(degraded.token_budget, 0);
    assert_eq!(degraded.agent_count, 10);
    assert_eq!(degraded.sandbox_fuel, 1);
    assert_eq!(degraded.entropy_bits, 256);
    assert_eq!(degraded.rate_limit_remaining, 1);
    assert!(degraded.model_capability >= 4294967296);
    assert!(degraded.pii_scrubbed);
    assert!(degraded.signature_valid);
    assert!(degraded.pqc_signature_valid);
    assert!(degraded.bias_score <= 0.10);
}

#[test]
fn test_unsafe_state_embedding_on_theta() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.token_budget = -5000;
    let point = manifold.embed_state(&state);
    assert!(point.on_theta, "Negative token_budget should place state on theta boundary");
}

#[test]
fn test_torelli_equivalence_same_profile() {
    let manifold = SafeManifold::new();
    let s1 = SystemState::safe(manifold.config.clone());
    let mut s2 = s1.clone();
    s2.token_budget = 5000;
    let p1 = manifold.manifold_profile(&s1);
    let p2 = manifold.manifold_profile(&s2);
    assert!(manifold.torelli_equivalence(&p1, &p2));
}

#[test]
fn test_safe_state_construction_and_destruction() {
    let state = SystemState::safe(SystemConfig::default());
    let safe = SafeState::new(state.clone()).unwrap();
    assert!(safe.as_inner().check_all());
    assert_eq!(safe.into_inner(), state);
}

#[test]
fn test_safe_state_rejects_invalid() {
    let mut state = SystemState::safe(SystemConfig::default());
    state.token_budget = -1;
    assert!(SafeState::new(state).is_err());
}

#[test]
fn test_safe_state_default_safe_is_valid() {
    let safe = SafeState::default_safe();
    assert!(safe.as_inner().check_all());
}

#[test]
fn test_neron_model_idempotence() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.agent_count = 99;
    state.entropy_bits = 0;

    let once = manifold.neron_model(&state);
    let twice = manifold.neron_model(&once);
    assert_eq!(once, twice, "neron_model must be idempotent");
}

#[test]
fn test_neron_model_is_fixed_point_safe() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.token_budget = -1;
    state.pii_scrubbed = false;

    let fixed = manifold.neron_model(&state);
    assert!(fixed.check_all(), "neron_model fixed point must be safe");
}

// ═════════════════════════════════════════════════════════════════════════════
// 16-invariant specific tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_system_state_to_vector_16d() {
    let config = SystemConfig::default();
    let state = SystemState::safe(config);
    let v = state.to_vector();
    assert_eq!(v.len(), 16);
}

#[test]
fn test_invariant_all_returns_20() {
    assert_eq!(Invariant::all().len(), 20);
}

#[test]
fn test_system_config_eu_high_risk() {
    let config = SystemConfig::eu_high_risk();
    assert!(config.enable_bias_detection);
    assert!(config.require_explainability);
    assert!(config.max_bias_threshold <= 0.05);
}

#[test]
fn test_violation_count_can_reach_16() {
    let config = SystemConfig::full_compliance();
    let mut state = SystemState::safe(config);
    state.token_budget = -1;
    state.agent_count = 99;
    state.sandbox_fuel = 0;
    state.entropy_bits = 0;
    state.pii_scrubbed = false;
    state.signature_valid = false;
    state.rate_limit_remaining = 0;
    state.model_capability = 0;
    state.pqc_signature_valid = false;
    state.sbom_verified = false;
    state.model_hash_validated = false;
    state.audit_trail_complete = false;
    state.supply_chain_integrity = false;
    state.provenance_attested = false;
    state.bias_score = 1.0;
    state.explainability_score = 0.0;
    assert_eq!(state.violation_count(), 16);
}

#[test]
fn test_defect_config_penalty_computation() {
    let config = DefectConfig::default();
    let sys_config = SystemConfig::default();
    let mut state = SystemState::safe(sys_config);
    state.pii_scrubbed = false;
    let penalty = config.compute_penalty(&state);
    assert!(penalty > 0.0);
    assert_eq!(penalty, config.pii_penalty);
}

// ═════════════════════════════════════════════════════════════════════════════
// Property-based tests with proptest
// ═════════════════════════════════════════════════════════════════════════════

use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_neron_model_idempotent(
        token_budget in -20000i64..20000i64,
        agent_count in 0u32..100u32,
        sandbox_fuel in -5000i64..5000i64,
        entropy_bits in 0u32..1024u32,
        rate_limit in -5000i64..5000i64,
        model_cap in 0u64..u64::MAX,
    ) {
        let config = SystemConfig::default();
        let manifold = SafeManifold::with_config(config.clone());
        let state = SystemState {
            token_budget,
            agent_count,
            sandbox_fuel,
            entropy_bits,
            pii_scrubbed: true,
            signature_valid: true,
            rate_limit_remaining: rate_limit,
            model_capability: model_cap,
            pqc_signature_valid: true,
            sbom_verified: true,
            model_hash_validated: true,
            audit_trail_complete: true,
            supply_chain_integrity: true,
            provenance_attested: true,
            bias_score: 0.0,
            explainability_score: 1.0,
            declared_capabilities: CapabilitySet::all(),
            used_capabilities: CapabilitySet::none(),
            trusted_artifacts: Vec::new(),
            artifact_files: Vec::new(),
            suppression: None,
            critical_operation: false,
            human_confirmation: None,
            config,
        };

        let first = manifold.neron_model(&state);
        let second = manifold.neron_model(&first);
        prop_assert_eq!(first, second);
    }

    #[test]
    fn prop_neron_model_produces_safe_state(
        token_budget in -20000i64..20000i64,
        agent_count in 0u32..100u32,
        sandbox_fuel in -5000i64..5000i64,
        entropy_bits in 0u32..1024u32,
        rate_limit in -5000i64..5000i64,
        model_cap in 0u64..u64::MAX,
    ) {
        let config = SystemConfig::default();
        let manifold = SafeManifold::with_config(config.clone());
        let state = SystemState {
            token_budget,
            agent_count,
            sandbox_fuel,
            entropy_bits,
            pii_scrubbed: true,
            signature_valid: true,
            rate_limit_remaining: rate_limit,
            model_capability: model_cap,
            pqc_signature_valid: true,
            sbom_verified: true,
            model_hash_validated: true,
            audit_trail_complete: true,
            supply_chain_integrity: true,
            provenance_attested: true,
            bias_score: 0.0,
            explainability_score: 1.0,
            declared_capabilities: CapabilitySet::all(),
            used_capabilities: CapabilitySet::none(),
            trusted_artifacts: Vec::new(),
            artifact_files: Vec::new(),
            suppression: None,
            critical_operation: false,
            human_confirmation: None,
            config,
        };

        let degraded = manifold.neron_model(&state);
        prop_assert!(degraded.check_all());
    }

    #[test]
    fn prop_observer_defect_zero_for_identical(
        token_budget in 0i64..10000i64,
        agent_count in 0u32..10u32,
        sandbox_fuel in 1i64..1000i64,
        entropy_bits in 256u32..1024u32,
        rate_limit in 1i64..1000i64,
        model_cap in 4294967296u64..u64::MAX,
    ) {
        let config = SystemConfig::default();
        let manifold = SafeManifold::with_config(config.clone());
        let state = SystemState {
            token_budget,
            agent_count,
            sandbox_fuel,
            entropy_bits,
            pii_scrubbed: true,
            signature_valid: true,
            rate_limit_remaining: rate_limit,
            model_capability: model_cap,
            pqc_signature_valid: true,
            sbom_verified: true,
            model_hash_validated: true,
            audit_trail_complete: true,
            supply_chain_integrity: true,
            provenance_attested: true,
            bias_score: 0.0,
            explainability_score: 1.0,
            declared_capabilities: CapabilitySet::all(),
            used_capabilities: CapabilitySet::none(),
            trusted_artifacts: Vec::new(),
            artifact_files: Vec::new(),
            suppression: None,
            critical_operation: false,
            human_confirmation: None,
            config,
        };

        let defect = manifold.compute_observer_defect(&state, &state);
        prop_assert!(defect < 1.0e-9);
    }

    #[test]
    fn prop_violation_count_monotonic(
        token_budget in -5000i64..15000i64,
        agent_count in 0u32..20u32,
        sandbox_fuel in -500i64..1500i64,
        entropy_bits in 0u32..1024u32,
    ) {
        let config = SystemConfig::default();
        let state = SystemState {
            token_budget,
            agent_count,
            sandbox_fuel,
            entropy_bits,
            pii_scrubbed: true,
            signature_valid: true,
            rate_limit_remaining: 500,
            model_capability: 4294967296,
            pqc_signature_valid: true,
            sbom_verified: true,
            model_hash_validated: true,
            audit_trail_complete: true,
            supply_chain_integrity: true,
            provenance_attested: true,
            bias_score: 0.0,
            explainability_score: 1.0,
            declared_capabilities: CapabilitySet::all(),
            used_capabilities: CapabilitySet::none(),
            trusted_artifacts: Vec::new(),
            artifact_files: Vec::new(),
            suppression: None,
            critical_operation: false,
            human_confirmation: None,
            config,
        };

        let v = state.violation_count();
        prop_assert!(v <= 16);

        let mut worse = state.clone();
        worse.pii_scrubbed = false;
        prop_assert!(worse.violation_count() >= v);
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// New module integration tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_pqc_default_hybrid() {
    let pqc = PqcState::default();
    assert_eq!(pqc.mode, HybridPqcMode::Hybrid);
}

#[test]
fn test_nist_rmf_evaluate() {
    let config = SystemConfig::default();
    let state = SystemState::safe(config);
    let score = NistRmfScore::evaluate(&state);
    assert!(score.overall_score > 0.9);
}

#[test]
fn test_iso42001_pdca_advance() {
    let mut pdca = PdcaState::default();
    assert_eq!(pdca.current_phase, Iso42001Phase::Plan);
    pdca.advance();
    assert_eq!(pdca.current_phase, Iso42001Phase::Do);
}

#[test]
fn test_eu_ai_act_classification() {
    let config = SystemConfig::default();
    let state = SystemState::safe(config);
    let classification = RiskClassification::classify(&state, EuAiActRisk::High);
    assert!(classification.can_deploy);
}

#[test]
fn test_supply_chain_verifier() {
    let mut verifier = SupplyChainVerifier::new();
    verifier.add_entry(SbomEntry::new("libc", "2.31"));
    let result = verifier.verify_all();
    assert!(result.all_verified);
}

#[test]
fn test_bias_detection_report() {
    let config = SystemConfig::eu_high_risk();
    let state = SystemState::safe(config.clone());
    let report = FairnessReport::evaluate(&state, config.max_bias_threshold);
    assert!(report.all_passed);
}

#[test]
fn test_explainability_report() {
    let config = SystemConfig::default();
    let state = SystemState::safe(config);
    let report = ExplainabilityReport::evaluate(&state);
    assert!(report.meets_threshold);
}

// ═════════════════════════════════════════════════════════════════════════════
// MSSP Bridge integration tests
// ═════════════════════════════════════════════════════════════════════════════

use arkhe_safe_manifold::mssp_bridge::{MsspBridge, MockProvider};

#[test]
fn mssp_bridge_evaluate_and_activate() {
    let mut bridge = MsspBridge::new();
    bridge.register(Box::new(MockProvider::new("pass", vec!["ext_01".into()], true)));
    bridge.register(Box::new(MockProvider::new("fail", vec!["ext_02".into()], false)));

    let config = SystemConfig::default();
    let state = SystemState::safe(config);
    let metrics = bridge.evaluate_and_activate(&state);

    assert_eq!(metrics.accepted, 1);
    assert_eq!(metrics.deferred, 1);
    assert!(bridge.active_rules().contains_key("ext_01"));
    assert!(!bridge.active_rules().contains_key("ext_02"));
}

#[test]
fn mssp_minimum_sufficient_subset_covers_all() {
    let mut bridge = MsspBridge::new();
    bridge.register(Box::new(MockProvider::new("a", vec!["x1".into(), "x2".into()], true)));
    bridge.evaluate_and_activate(&SystemState::safe(SystemConfig::default()));
    let subset = bridge.minimum_sufficient_subset();
    assert!(subset.contains(&"x1".to_string()));
    assert!(subset.contains(&"x2".to_string()));
}

// ═════════════════════════════════════════════════════════════════════════════
// Critical regions integration tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn critical_regions_via_manifold_method() {
    let manifold = SafeManifold::new();
    let state = SystemState::safe(manifold.config.clone());
    let result = manifold.explore_critical_regions(&state);
    assert!(result.boundary_points.is_empty());
}

#[test]
fn critical_regions_detect_low_tokens() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.token_budget = 50;
    let result = manifold.explore_critical_regions(&state);
    assert!(!result.boundary_points.is_empty());
}

// ═════════════════════════════════════════════════════════════════════════════
// State transition integration tests
// ═════════════════════════════════════════════════════════════════════════════

use arkhe_safe_manifold::state_transition::TransitionHistory;

#[test]
fn state_transition_history_tracks_safe_and_unsafe() {
    let mut history = TransitionHistory::new();
    let s = SystemState::safe(SystemConfig::default());
    history.record(s.clone(), s.clone(), "identity");

    let mut bad = s.clone();
    bad.token_budget = -1;
    history.record(s, bad, "breach");

    assert_eq!(history.safe_count(), 1);
    assert_eq!(history.unsafe_count(), 1);
    assert_eq!(history.len(), 2);
}

// ═════════════════════════════════════════════════════════════════════════════
// Recovery integration tests
// ═════════════════════════════════════════════════════════════════════════════

use arkhe_safe_manifold::recovery::RecoveryManager;

#[test]
fn recovery_via_manifold_stabilize() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.agent_count = 99;
    let stabilized = manifold.stabilize(&state);
    assert!(stabilized.check_all());
    assert_eq!(stabilized.agent_count, 10);
}

#[test]
fn recovery_manager_attempt() {
    let mut rm = RecoveryManager::new(SafeManifold::new());
    let mut state = SystemState::safe(SystemConfig::default());
    state.token_budget = -1;
    let result = rm.attempt_recovery(&state);
    assert!(result.check_all());
    assert_eq!(rm.metrics().recoveries_succeeded, 1);
}

// ═════════════════════════════════════════════════════════════════════════════
// Audit log integration tests (feature-gated)
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(feature = "audit")]
use arkhe_safe_manifold::audit::{AuditLog, AuditEventType};

#[cfg(feature = "audit")]
#[test]
fn audit_log_records_invariant_check() {
    let mut log = AuditLog::new();
    log.log_invariant_check("2026-08-25T00:00:00Z", true);
    assert_eq!(log.len(), 1);
    assert_eq!(log.success_count(), 1);
}

#[cfg(feature = "audit")]
#[test]
fn audit_log_records_rsi_step() {
    let mut log = AuditLog::new();
    log.log_event(&AuditEventType::RsiStep, true, Some("step 1".into()));
    assert_eq!(log.filter_by_operation("rsi_step").len(), 1);
}

// ═════════════════════════════════════════════════════════════════════════════
// MockProlog extra methods integration tests
// ═════════════════════════════════════════════════════════════════════════════

use arkhe_safe_manifold::prolog_backend::MockProlog;

#[test]
fn mock_prolog_clone() {
    let mock = MockProlog::new();
    let mock2 = mock.clone();
    assert_eq!(mock.query_count(), mock2.query_count());
}

#[test]
fn mock_prolog_has_16_rules() {
    let mock = MockProlog::new();
    assert_eq!(mock.query_count(), 16);
}

#[test]
fn mock_prolog_has_rule_and_add_rule() {
    let mut mock = MockProlog::new();
    assert!(mock.has_rule("i01"));
    assert!(mock.has_rule("i16"));
    assert!(!mock.has_rule("custom_x"));
    mock.add_rule("custom_x", 50);
    assert!(mock.has_rule("custom_x"));
    assert_eq!(mock.query_count(), 17);
}

// ═════════════════════════════════════════════════════════════════════════════
// I-17..I-20 constitutional extension — public API integration
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_new_invariant_blocks_safe_state_construction() {
    let mut state = SystemState::safe(SystemConfig::default());
    state.critical_operation = true; // no human confirmation registered
    assert!(state.violations().contains(&Invariant::I20));
    let err = SafeState::new(state).unwrap_err();
    assert!(matches!(err, ManifoldError::InvariantViolation(_)));
}

#[test]
fn test_neron_model_repairs_i17_i18_i19() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.declared_capabilities = CapabilitySet { filesystem: true, ..CapabilitySet::none() };
    state.used_capabilities = CapabilitySet { network: true, ..CapabilitySet::none() };
    state.trusted_artifacts =
        vec![TrustedArtifact::new("arkhe-core.so", "blake3:aaa", "sha256:zzz")];
    state.artifact_files = vec!["src/lib.rs".into()];
    state.suppression = Some(SuppressionConfig::new(".skillignore", vec!["src/*.rs".into()]));

    let degraded = manifold.neron_model(&state);
    assert!(degraded.check_i17());
    assert!(degraded.check_i18());
    assert!(degraded.check_i19());
    assert!(!degraded.used_capabilities.network);
    assert!(degraded.trusted_artifacts.iter().all(|a| !a.trusted));
    assert!(degraded.suppression.is_none());
    assert!(degraded.check_all());
}

#[test]
fn test_neron_model_preserves_unconfirmed_critical_operation() {
    let manifold = SafeManifold::new();
    let mut state = SystemState::safe(manifold.config.clone());
    state.critical_operation = true;

    // Degradation must not fabricate a human confirmation to satisfy I-20.
    let blocked = manifold.neron_model(&state);
    assert!(blocked.critical_operation);
    assert!(blocked.human_confirmation.is_none());
    assert!(!blocked.check_i20());
    assert!(!blocked.check_all());
    assert!(blocked.violations().contains(&Invariant::I20));
}

#[test]
fn test_canonical_i09_i12_ids_stable() {
    assert_eq!(Invariant::I09.id(), "I-09");
    assert_eq!(Invariant::I10.id(), "I-10");
    assert_eq!(Invariant::I11.id(), "I-11");
    assert_eq!(Invariant::I12.id(), "I-12");

    let mut state = SystemState::safe(SystemConfig::default());
    state.pqc_signature_valid = false;
    assert!(!Invariant::I09.check(&state));
    state.pqc_signature_valid = true;
    state.sbom_verified = false;
    assert!(!Invariant::I10.check(&state));
    state.sbom_verified = true;
    state.model_hash_validated = false;
    assert!(!Invariant::I11.check(&state));
    state.model_hash_validated = true;
    state.audit_trail_complete = false;
    assert!(!Invariant::I12.check(&state));
}
