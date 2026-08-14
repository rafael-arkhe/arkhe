//! Constitutional invariants for the Neurogenesis substrate:
//!
//! * **I16** — when growth is allowed and the threshold is low enough,
//!   multiple dendrite branches emerge (`branches.len() > 1`).
//! * **I18** — with parity–time symmetry active (balanced gain/loss), the
//!   total amplitude `Σ ψ_i²` stays bounded within a tight window.

use arkhe_neurogenesis::SimulationConfig;

#[test]
fn i16_growth_allowed_produces_multiple_branches() {
    let cfg = SimulationConfig {
        n: 16,
        dt: 0.01,
        t_max: 3.0,
        seed: 7,
        threshold: 0.3,
        growth_allowed: true,
        parity_time_sym: true,
    };
    let res = cfg.run();
    assert!(
        res.neuron.branches.len() > 1,
        "expected more than one dendrite branch, got {}",
        res.neuron.branches.len()
    );
    // Every branch must come with a recorded growth event and stay in range.
    assert_eq!(res.neuron.branches.len(), res.neuron.events.len());
    for &(parent, child) in &res.neuron.branches {
        assert!(parent < cfg.n, "parent out of range");
        assert!(child < res.neuron.nodes.len(), "child out of range");
    }
}

#[test]
fn i18_pt_symmetry_bounds_total_amplitude() {
    let cfg = SimulationConfig {
        n: 8,
        dt: 0.01,
        t_max: 0.3,
        seed: 8,
        threshold: 10.0,
        growth_allowed: false,
        parity_time_sym: true,
    };
    let res = cfg.run();
    assert!(res.amplitude_series.len() > 1, "time series must be non-empty");
    let initial = res.amplitude_series[0];
    assert!((initial - 1.0).abs() < 1e-9, "initial amplitude must be normalized");
    let peak_dev = res
        .amplitude_series
        .iter()
        .map(|x| (x - initial).abs() / initial)
        .fold(0.0_f64, f64::max);
    assert!(
        peak_dev < 0.03,
        "PT-balanced gain/loss must keep total amplitude within a tight window, peak relative deviation was {peak_dev}"
    );
}

#[test]
fn simulation_matches_documented_invariant_reference() {
    // Pin the deterministic reference produced by the seed stream, so a
    // regression in the LCG, the Hamiltonian or the integrator is caught.
    let cfg = SimulationConfig {
        n: 16,
        dt: 0.01,
        t_max: 3.0,
        seed: 7,
        threshold: 0.3,
        growth_allowed: true,
        parity_time_sym: true,
    };
    let res = cfg.run();
    assert_eq!(res.neuron.branches.len(), 4);
    assert_eq!(res.neuron.nodes.len(), cfg.n + 1 + res.neuron.branches.len());
}
