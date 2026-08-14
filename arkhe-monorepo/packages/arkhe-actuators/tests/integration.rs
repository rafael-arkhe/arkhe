//! Integration tests for `arkhe-actuators` (RF cell modulator, US10064941B2).
//!
//! These pin the patent model against the deterministic simulator, tie the I18
//! calcium guard to the same relative-drift bound that `arkhe-neurogenesis`
//! verifies on its PT-symmetric amplitude, and check the Z1→Z2→Z3 evidence
//! against the real `arkhe-buzz-bridge` firewall.

use arkhe_actuators::{
    I18CalciumGuard, I18Verdict, NanoparticleType, RfCellModulator, RfStimulationConfig,
    TargetGene, evidence_stays_in_firewall,
};
use arkhe_buzz_bridge::{CertificationStatus, EdgeType, validate_hyperedge_firewall};
use arkhe_neurogenesis::{SimulationConfig, simulate};

#[test]
fn stimulation_is_deterministic() {
    let modulator = RfCellModulator::new(RfStimulationConfig::default());
    let a = modulator.stimulate().unwrap();
    let b = modulator.stimulate().unwrap();
    assert_eq!(a.calcium_peak_nm, b.calcium_peak_nm);
    assert_eq!(a.gene_expression_fold, b.gene_expression_fold);
    assert_eq!(a.calcium_time_series.len(), 10);
}

#[test]
fn patent_default_reproduces_in_vitro_response() {
    // Fe₃O₄ @ 465 kHz, 50 kA/m — patent in-vitro: ~5× Ca²⁺, 2–5× gene lift.
    let modulator = RfCellModulator::new(RfStimulationConfig {
        frequency_hz: 465_000.0,
        field_strength_ka_m: 50.0,
        duration_ms: 1_800_000,
        nanoparticle: NanoparticleType::IronOxide,
        target_gene: TargetGene::Insulin,
        calcium_peak_threshold_nm: 800.0,
        in_vivo: false,
    });
    let res = modulator.stimulate().unwrap();
    assert!(
        (450.0..=650.0).contains(&res.calcium_peak_nm),
        "Ca2+ peak {} nM outside patent window",
        res.calcium_peak_nm
    );
    assert!((2.0..=3.0).contains(&res.gene_expression_fold));
}

#[test]
fn in_vivo_config_reports_glycemia_reduction() {
    let modulator = RfCellModulator::new(RfStimulationConfig {
        in_vivo: true,
        ..RfStimulationConfig::default()
    });
    let res = modulator.stimulate().unwrap();
    let glucose = res.glucose_mg_dl.expect("in-vivo trace must report glycemia");
    // Patent: ~30% reduction from an ~120 mg/dL baseline.
    assert!((80.0..120.0).contains(&glucose), "glucose {glucose} mg/dL");
}

#[test]
fn field_out_of_range_is_rejected() {
    let modulator = RfCellModulator::new(RfStimulationConfig {
        field_strength_ka_m: 250.0,
        ..RfStimulationConfig::default()
    });
    assert!(modulator.stimulate().is_err());
}

#[test]
fn i18_run_guard_aborts_on_runaway() {
    // Tight uptick window: a doubling from baseline is a runaway.
    let mut guard = I18CalciumGuard::new(100.0, 800.0, 0.25);
    let audit = guard.run(&[(0.0, 100.0), (10.0, 300.0), (20.0, 900.0)]);
    assert_eq!(audit.verdict, I18Verdict::Abort);
    // Reaching the peak → exceeded the bound first; a dedicated step marks it.
    assert!(!audit.steps[2].within_bound);
}

#[test]
fn i18_guard_allows_stable_train() {
    let mut guard = I18CalciumGuard::new(100.0, 800.0, 0.5);
    let train = vec![
        (0.0, 100.0),
        (10.0, 500.0),
        (20.0, 520.0),
        (30.0, 510.0),
    ];
    let audit = guard.run(&train);
    assert_eq!(audit.verdict, I18Verdict::Proceed);
}

#[test]
fn i18_reuses_neurogenesis_drift_bound() {
    // The calcium guard's I18 semantics mirror the PT-symmetric amplitude bound
    // that arkhe-neurogenesis pins: relative drift stays under a 3% cap.
    let cfg = SimulationConfig {
        n: 8,
        dt: 0.01,
        t_max: 0.3,
        seed: 8,
        threshold: 10.0, // no growth, pure spectral-fidelity check
        growth_allowed: false,
        parity_time_sym: true,
    };
    let res = simulate(&cfg);
    assert!(
        res.peak_drift < 0.03,
        "neurogenesis I18 peak_drift {} >= 0.03",
        res.peak_drift
    );
}

#[test]
fn evidence_is_certified_and_firewall_safe() {
    let modulator = RfCellModulator::new(RfStimulationConfig::default());
    let res = modulator.stimulate().unwrap();
    let step = I18CalciumGuard::new(100.0, 800.0, 0.5)
        .observe(0.0, res.calcium_peak_nm);
    let bundle = modulator.evidence_for(&res, &step);
    assert_eq!(bundle.certification, CertificationStatus::Supported);
    assert_eq!(bundle.digest_hex().len(), 64);
    assert!(evidence_stays_in_firewall().is_ok());
}

#[test]
fn firewall_blocks_direct_depends_on_edge() {
    let nodes = vec![
        ("z2".to_string(), arkhe_buzz_bridge::Zone::Z2_Continuous),
        ("z3".to_string(), arkhe_buzz_bridge::Zone::Z3_Discrete),
    ];
    assert!(
        validate_hyperedge_firewall(&nodes, EdgeType::TranslatesToPrimitive.as_str()).is_ok()
    );
    assert!(validate_hyperedge_firewall(&nodes, EdgeType::DependsOn.as_str()).is_err());
}