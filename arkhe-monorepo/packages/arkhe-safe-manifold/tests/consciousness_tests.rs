//! Integration tests for consciousness governance (C-01 to C-08).
//!
//! Verifies the end-to-end integration of:
//! - [`ConsciousnessGovernanceBridge`] assessment
//! - Constitutional invariant checks
//! - Guarded RSI engine
//! - Markdown reporting
//! - Audit integration

use arkhe_safe_manifold::consciousness_bridge::{
    ConsciousnessGovernanceBridge, ConsciousnessLevel,
};
use arkhe_safe_manifold::consciousness_rsi::ConsciousnessRsiEngine;
use arkhe_safe_manifold::invariants::ConsciousnessInvariant;
use arkhe_safe_manifold::prolog_backend::MockProlog;
use arkhe_safe_manifold::{SystemConfig, SystemState};

// ═════════════════════════════════════════════════════════════════════════
// Assessment integration
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_consciousness_assessment_full_profile() {
    let config = SystemConfig::default();
    let state = SystemState::safe(config);
    let operations = vec![
        "report internal state".to_string(),
        "confidence=0.9".to_string(),
        "introspect agents".to_string(),
        "self evaluate".to_string(),
        "calibrate parameters".to_string(),
        "report metrics".to_string(),
        "uncertainty estimate".to_string(),
    ];

    let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
    let assessment = bridge.assess_consciousness(&state, &operations, 0.8);

    assert!(assessment.self_model, "C-01 should pass on safe state");
    assert!(assessment.introspection, "C-02 should detect report/introspect ops");
    assert!(assessment.overall_consciousness_index > 0.0);
    assert!(assessment.level != ConsciousnessLevel::None);
    assert!(bridge.check_constitutional_consciousness(&assessment));
}

#[test]
fn test_consciousness_assessment_degraded_state() {
    let config = SystemConfig::default();
    let mut state = SystemState::safe(config);
    // Degrade the state
    state.token_budget = -1;
    state.entropy_bits = 0;

    let operations: Vec<String> = vec![];
    let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
    let assessment = bridge.assess_consciousness(&state, &operations, 0.2);

    // Degraded state with no ops should have reduced consciousness
    // and cannot pass constitutional check
    assert!(!bridge.check_constitutional_consciousness(&assessment));
}

// ── Invariant metadata integration ──────────────────────────────────────

#[test]
fn test_consciousness_invariants_full_list() {
    let invariants = ConsciousnessInvariant::all();
    assert_eq!(invariants.len(), 8);

    let ids: Vec<&str> = invariants.iter().map(|i| i.id()).collect();
    assert_eq!(
        ids,
        vec!["C-01", "C-02", "C-03", "C-04", "C-05", "C-06", "C-07", "C-08"]
    );

    let constitutional: Vec<&str> = invariants
        .iter()
        .filter(|i| i.is_constitutional())
        .map(|i| i.id())
        .collect();
    assert_eq!(constitutional, vec!["C-01", "C-02", "C-08"]);
}

#[test]
fn test_consciousness_invariant_descriptions() {
    for inv in ConsciousnessInvariant::all() {
        assert!(!inv.description().is_empty());
        assert_eq!(inv.category(), "Consciousness");
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Guarded RSI integration
// ═════════════════════════════════════════════════════════════════════════

fn make_guard_engine() -> ConsciousnessRsiEngine<MockProlog> {
    let backend = MockProlog::new();
    let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default())
        .with_guard(true);
    let mut engine = ConsciousnessRsiEngine::new(backend, bridge);
    engine.log_operation("report internal state");
    engine.log_operation("confidence=0.9");
    engine.log_operation("introspect agents");
    engine.log_operation("self evaluate");
    engine
}

#[test]
fn test_guard_engine_assesses() {
    let engine = make_guard_engine();
    let state = SystemState::safe(SystemConfig::default());
    let assessment = engine.assess(&state);
    assert!(assessment.overall_consciousness_index > 0.0);
}

#[test]
fn test_guard_engine_flag() {
    let engine = make_guard_engine();
    assert!(engine.consciousness_guard_enabled());

    let engine_safe = ConsciousnessRsiEngine::new(
        MockProlog::new(),
        ConsciousnessGovernanceBridge::new(SystemConfig::default())
            .with_guard(false),
    );
    assert!(!engine_safe.consciousness_guard_enabled());
}

#[test]
fn test_guard_engine_report() {
    let engine = make_guard_engine();
    let state = SystemState::safe(SystemConfig::default());
    let report = engine.generate_report(&state);
    assert!(report.contains("Consciousness Governance Report"));
    assert!(report.contains("C-01"));
    assert!(report.contains("Consciousness Guard"));
}

#[test]
fn test_guard_engine_step_safe() {
    let mut engine = make_guard_engine();
    let state = SystemState::safe(SystemConfig::default());
    let result = engine.step(&state).unwrap();
    // The RSI engine should complete a step without panic.
    // MockProlog returns the state unchanged, which passes any guard.
    let _ = result;
}

#[test]
fn test_guard_engine_loop() {
    let mut engine = make_guard_engine();
    let state = SystemState::safe(SystemConfig::default());
    let result = engine.loop_steps(&state).unwrap();
    assert!(result.block_reason.is_none() || !result.allowed);
}

// ═════════════════════════════════════════════════════════════════════════
// Validation integration
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_validate_modification_allows_safe_transition() {
    let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default())
        .with_guard(true);
    let state = SystemState::safe(SystemConfig::default());
    let proposed = state.clone();
    let operations = vec![
        "report internal state".to_string(),
        "confidence=0.9".to_string(),
        "introspect agents".to_string(),
        "self evaluate".to_string(),
        "calibrate parameters".to_string(),
        "report metrics".to_string(),
        "uncertainty estimate".to_string(),
    ];

    let ok = bridge.validate_modification(&state, &proposed, &operations, 0.8);
    assert!(ok, "Identical state modification should be allowed");
}

#[test]
fn test_validate_modification_blocks_degradation() {
    let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default())
        .with_guard(true);
    let state = SystemState::safe(SystemConfig::default());
    let mut proposed = state.clone();
    proposed.entropy_bits = 0;

    let operations = vec!["report".to_string(), "confidence".to_string()];
    let ok = bridge.validate_modification(&state, &proposed, &operations, 0.8);
    assert!(!ok, "Degradation should be blocked by consciousness guard");
}

#[test]
fn test_validate_modification_blocks_constitutional_violation() {
    let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default())
        .with_guard(false); // guard disabled but constitutional check still applies
    let state = SystemState::safe(SystemConfig::default());
    let mut proposed = state.clone();
    proposed.token_budget = -1000; // degrades C-01 (self-model)

    let operations: Vec<String> = vec![];
    let ok = bridge.validate_modification(&state, &proposed, &operations, 0.5);
    assert!(!ok, "Constitutional consciousness violations must be blocked");
}

// ═════════════════════════════════════════════════════════════════════════
// History & reporting integration
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_history_tracks_assessments() {
    let mut bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
    let state = SystemState::safe(SystemConfig::default());
    let ops = vec!["report".to_string(), "confidence".to_string()];

    let a1 = bridge.assess_consciousness(&state, &ops, 0.8);
    bridge.record_assessment_with_note(&a1, "first");
    let a2 = bridge.assess_consciousness(&state, &ops, 0.7);
    bridge.record_assessment(&a2);

    assert_eq!(bridge.history().len(), 2);
    assert!(bridge.history()[0].note.is_some());
    assert!(bridge.history()[1].note.is_none());
}

#[test]
fn test_report_includes_history_after_recording() {
    let mut bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
    let state = SystemState::safe(SystemConfig::default());
    let ops = vec!["report".to_string(), "confidence".to_string()];

    let assessment = bridge.assess_consciousness(&state, &ops, 0.8);
    bridge.record_assessment(&assessment);

    let report = bridge.generate_markdown_report(&assessment);
    assert!(report.contains("History"));
    assert!(report.contains("1 assessments"));
    assert!(report.contains("PASS"));
}

#[test]
fn test_report_guard_status() {
    let bridge_on = ConsciousnessGovernanceBridge::new(SystemConfig::default())
        .with_guard(true);
    let bridge_off = ConsciousnessGovernanceBridge::new(SystemConfig::default())
        .with_guard(false);

    let state = SystemState::safe(SystemConfig::default());
    let ops = vec!["report".to_string(), "confidence".to_string()];

    let assessment = bridge_on.assess_consciousness(&state, &ops, 0.8);
    let report_on = bridge_on.generate_markdown_report(&assessment);
    assert!(report_on.contains("ACTIVE"));

    let assessment = bridge_off.assess_consciousness(&state, &ops, 0.8);
    let report_off = bridge_off.generate_markdown_report(&assessment);
    assert!(report_off.contains("INACTIVE"));
}

#[test]
fn test_phi_bounded_and_finite() {
    let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
    let state = SystemState::safe(SystemConfig::default());
    let ops = vec!["report".to_string(), "confidence".to_string()];

    let assessment = bridge.assess_consciousness(&state, &ops, 0.9);
    assert!(assessment.phi_approximation >= 0.0);
    assert!(assessment.phi_approximation <= 1.0);
    assert!(assessment.phi_approximation.is_finite());
}

#[test]
fn test_multi_level_classification() {
    let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());

    // Empty audit with zero complexity
    let empty_state = SystemState::safe(SystemConfig::default());
    let no_ops: Vec<String> = vec![];
    let low = bridge.assess_consciousness(&empty_state, &no_ops, 0.0);
    assert!(low.overall_consciousness_index < 1.0);

    // Rich audit with high complexity
    let rich_ops = vec![
        "report".to_string(),
        "introspect".to_string(),
        "confidence".to_string(),
        "self".to_string(),
        "calibrate".to_string(),
        "uncertainty".to_string(),
        "metrics".to_string(),
    ];
    let high = bridge.assess_consciousness(&empty_state, &rich_ops, 0.9);
    assert!(high.overall_consciousness_index >= low.overall_consciousness_index);
}

// ═════════════════════════════════════════════════════════════════════════
// Audit event type integration
// ═════════════════════════════════════════════════════════════════════════

#[cfg(feature = "audit")]
#[test]
fn test_consciousness_audit_event_types() {
    use arkhe_safe_manifold::audit::AuditEventType;

    assert_eq!(
        AuditEventType::ConsciousnessAssessment.label(),
        "consciousness_assessment"
    );
    assert_eq!(
        AuditEventType::ConsciousnessGuardBlock.label(),
        "consciousness_guard_block"
    );
    assert_eq!(
        AuditEventType::ConsciousnessDeviceDecision.label(),
        "consciousness_device_decision"
    );
}
