//! Demonstration of the SafeManifold security projection (v0.8.0 — 16 invariants).

use arkhe_safe_manifold::*;

fn main() {
    println!("═══════════════════════════════════════════════════════════════");
    println!("  ARKHE-χ SafeManifold — v0.8.0 (16 invariants)");
    println!("═══════════════════════════════════════════════════════════════\n");

    let manifold = SafeManifold::new();
    let ideal = SystemState::safe(manifold.config.clone());

    // ── 1. SafeState (Parse, Don't Validate) ───────────────────────────────
    println!("1. SafeState — 16 invariants guaranteed at construction");
    let safe = SafeState::new(ideal.clone()).expect("state must be valid");
    println!("   SafeState created: check_all = {}\n", safe.as_inner().check_all());

    // ── 2. Canonical 16D projection ────────────────────────────────────────
    println!("2. 16D canonical projection (embed_state)");
    let point = manifold.embed_state(&ideal);
    println!("   ManifoldPoint: on_theta = {}\n", point.on_theta);

    // ── 3. Normalized 16D safety-distance score ────────────────────────────
    println!("3. 16D observer defect (compute_observer_defect)");
    let mut actual = ideal.clone();
    actual.token_budget = 8000;
    actual.agent_count = 9;
    let defect = manifold.compute_observer_defect(&ideal, &actual);
    println!("   Defect (ideal vs actual): {:.6}\n", defect);

    // ── 4. Escape classification ───────────────────────────────────────────
    println!("4. Escape classification (16 invariants)");
    let region = manifold.classify_escape(&actual);
    println!("   Region: {:?}\n", region);

    // ── 5. Graceful degradation (Néron model) ──────────────────────────────
    println!("5. Graceful degradation — enforce ALL 16 invariants");
    let mut unsafe_state = ideal.clone();
    unsafe_state.token_budget = -1;
    unsafe_state.agent_count = 99;
    unsafe_state.pqc_signature_valid = false;
    unsafe_state.bias_score = 0.5;
    let degraded = manifold.neron_model(&unsafe_state);
    println!("   Degraded: check_all() = {}\n", degraded.check_all());

    // ── 6. EU AI Act risk classification ───────────────────────────────────
    println!("6. EU AI Act — High-risk classification");
    let classification = RiskClassification::classify(&ideal, EuAiActRisk::High);
    println!("   Can deploy: {}", classification.can_deploy);
    println!("   Missing invariants: {}\n", classification.missing_invariants.len());

    // ── 7. NIST AI RMF trustworthiness scoring ────────────────────────────
    println!("7. NIST AI RMF — Trustworthiness scoring");
    let nist_score = NistRmfScore::evaluate(&ideal);
    println!("   Overall score: {:.2}\n", nist_score.overall_score);

    // ── 8. ISO 42001 PDCA cycle ───────────────────────────────────────────
    println!("8. ISO 42001 — PDCA cycle tracking");
    let pdca = PdcaState::default();
    println!("   Phase: {:?}, Cycles: {}\n", pdca.current_phase, pdca.cycle_count);

    // ── 9. Hybrid PQC mode ────────────────────────────────────────────────
    println!("9. Hybrid PQC — Migration tracking");
    let pqc = PqcState::default();
    println!("   Mode: {:?}, Phase: {}\n", pqc.mode, pqc.migration_phase);

    // ── 10. Supply-chain verification ──────────────────────────────────────
    println!("10. Supply-chain — SBOM verification");
    let mut verifier = SupplyChainVerifier::new();
    verifier.add_entry(SbomEntry::new("libc", "2.31").verified());
    verifier.add_entry(SbomEntry::new("openssl", "3.0").verified());
    let result = verifier.verification_status();
    println!("    All verified: {}\n", result.all_verified);

    // ── 11. Bias detection ────────────────────────────────────────────────
    println!("11. Bias detection — Fairness metrics");
    let config = SystemConfig::eu_high_risk();
    let report = FairnessReport::evaluate(&ideal, config.max_bias_threshold);
    println!("    All passed: {}, Violations: {}\n", report.all_passed, report.violations);

    // ── 12. Explainability ─────────────────────────────────────────────────
    println!("12. Explainability — Model interpretability");
    let explain = ExplainabilityReport::evaluate(&ideal);
    println!("    Score: {}, Meets threshold: {}\n", explain.score, explain.meets_threshold);

    // ── 13. 16D vector projection ──────────────────────────────────────────
    println!("13. 16D vector projection (to_vector)");
    let v = ideal.to_vector();
    println!("    Vector length: {}\n", v.len());

    // ── 14. DefectConfig penalty computation ────────────────────────────────
    println!("14. DefectConfig — Penalty-based defect computation");
    let defect_config = DefectConfig::default();
    let penalty = defect_config.compute_penalty(&ideal);
    println!("    Penalty on safe state: {}\n", penalty);

    // ── 15. Invariant metadata ─────────────────────────────────────────────
    println!("15. Invariant metadata — EU AI Act & NIST RMF mappings");
    for inv in Invariant::all() {
        let eu = inv.eu_ai_act_article().unwrap_or("—");
        let nist = inv.primary_rmf_function().unwrap_or("—");
        println!("    {} | {} | {} | {}", inv.id(), inv.category(), eu, nist);
    }
    println!();

    // ── 16. Idempotence of neron_model ─────────────────────────────────────
    println!("16. Idempotence of neron_model");
    let once = manifold.neron_model(&unsafe_state);
    let twice = manifold.neron_model(&once);
    println!("    neron_model(s) == neron_model(neron_model(s))? {}\n", once == twice);

    println!("═══════════════════════════════════════════════════════════════");
    println!("  v0.8.0 demonstration completed successfully.");
    println!("═══════════════════════════════════════════════════════════════");
}
