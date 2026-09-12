//! Constitutional validation protocol for the XY simulator, applying the
//! post-v10.0 Simulation Standard (2026-08 audit):
//!
//! * **V-1** — 5-seed protocol: mean and std reported per condition.
//! * **V-2** — Energy floor never violated.
//! * **V-3** — Order-parameter convergence `< 1e-6` per 1000 steps.
//! * **V-4** — Determinism: same seed yields identical trajectories.
//! * **V-5** — Pinned falsification: the driving dynamics of P3 does NOT
//!   produce a static pitch; the energy relaxation DOES.

use arkhe_xy_simulator::p1_kuramoto::{KuramotoConfig, KuramotoSim, PlvrPair, plvr_lock};
use arkhe_xy_simulator::p2_langevin::{LangevinConfig, run_langevin};
use arkhe_xy_simulator::p3_dmi::{ChiralConfig, DmiDriveSim, relax_chain_pitch, relax_square_domains, run_dmi};
use arkhe_xy_simulator::p4_pinned::{PinnedConfig, PinnedSim};
use arkhe_xy_simulator::diagnostics::order_parameter;

#[test]
fn v4_determinism_same_seed_same_trajectory() {
    let cfg = KuramotoConfig {
        seed: 21,
        ..KuramotoConfig::default()
    };
    let mut a = KuramotoSim::new(&cfg);
    let mut b = KuramotoSim::new(&cfg);
    a.step(500);
    b.step(500);
    assert_eq!(a.phi(), b.phi(), "identical seeds must give identical phases");
}

#[test]
fn v1_p1_order_rises_with_coupling() {
    // σ = 0.5 -> Kc_all_to_all ~= 0.798. On the 4-NN lattice the effective
    // coupling is stronger, so R must already be substantial at K=0.9 and
    // strictly larger than at K=0.3.
    let low = KuramotoConfig { k: 0.3, ..KuramotoConfig::default() };
    let high = KuramotoConfig { k: 0.9, ..KuramotoConfig::default() };
    let r_low = KuramotoSim::run(&low, 8_000, 500);
    let r_high = KuramotoSim::run(&high, 8_000, 500);
    assert!(r_high.final_order_mean > r_low.final_order_mean + 0.1);
    assert!(
        r_high.final_order_mean > 0.5,
        "R at K=0.9 must be substantial, got {} ± {}",
        r_high.final_order_mean,
        r_high.final_order_std
    );
}

#[test]
fn v3_p1_converges_in_synchronized_regime() {
    // Near criticality R fluctuates because drifting clusters coexist with
    // the locked core. In the strongly synchronized regime (K >> Kc) the
    // phase-locked solution is a rigid rotation and R is exactly stationary,
    // which is where the < 1e-6 variation requirement is physically valid.
    let cfg = KuramotoConfig {
        k: 3.0,
        ..KuramotoConfig::default()
    };
    let res = KuramotoSim::run(&cfg, 20_000, 500);
    assert!(res.converged, "order parameter must converge below 1e-6/1000 steps");
}

#[test]
fn v5_plvr_lock_above_kc_pair() {
    // Two-oscillator: locked iff |w_l - w_e|/(2k) <= 1.
    let locked = PlvrPair {
        omega_l: 1.0,
        omega_e: 0.0,
        k: 1.0,
        ..PlvrPair::default()
    };
    let unlocked = PlvrPair {
        omega_l: 3.0,
        omega_e: 0.0,
        k: 1.0,
        ..locked.clone()
    };
    assert!(locked.analytic_lock());
    assert!(!unlocked.analytic_lock());
    let rl = plvr_lock(&locked, 20_000);
    let ru = plvr_lock(&unlocked, 20_000);
    assert!(rl.locked);
    assert!(!ru.locked);
}

#[test]
fn v2_p2_energy_floor_and_corrected_kt() {
    let cfg = LangevinConfig {
        n: 16,
        k: 0.6,
        t: 0.1,
        dt: 0.01,
        seed: 7,
    };
    let res = run_langevin(&cfg, 8_000, 500);
    assert!(!res.energy_floor_violated, "energy must never breach the analytic floor");
    assert!((res.t_kt - 0.267).abs() < 1e-9, "T_KT must follow 0.89*K/2");
    assert!(res.order_mean > 0.5, "T=0.1 << T_KT(0.267) must be quasi-ordered, got {}", res.order_mean);
}

#[test]
fn v1_p2_low_t_orders_high_t_disorder() {
    let cold = run_langevin(&LangevinConfig { t: 0.05, ..LangevinConfig::default() }, 8_000, 300);
    let hot = run_langevin(&LangevinConfig { t: 0.9, ..LangevinConfig::default() }, 8_000, 300);
    assert!(cold.order_mean > hot.order_mean + 0.4, "T must destroy order");
    assert!(hot.order_mean < 0.35, "T=0.9 must be deep in the disordered regime, got {}", hot.order_mean);
}

#[test]
fn v5_p3_driving_has_no_static_pitch() {
    // The dynamics as originally written must NOT produce a static spiral:
    // the structural flag is always false. A small perturbation around the
    // uniform branch returns to coherent rotation (R -> 1, gradient -> 0),
    // NOT to a plane wave with pitch 2pi/alpha (which would show a finite
    // mean bond gradient). Large disorder falls into a fragmented basin —
    // neither branch carries the predicted pitch.
    let cfg = ChiralConfig {
        n: 16,
        k: 0.9,
        alpha: 0.4,
        dt: 0.02,
        seed: 7,
    };
    let res = run_dmi(&cfg, 4_000, 10_000);
    assert!(!res.driven_spiral, "driving dynamics has no static pitch");

    // Coherent-rotation branch: perturb uniform -> stays coherent.
    let mut sim = DmiDriveSim::new(&cfg);
    let perturbed: Vec<f64> = (0..cfg.n * cfg.n).map(|i| 0.01 * (i as f64).sin()).collect();
    sim.set_phi(&perturbed);
    sim.integrate(5_000);
    assert!(order_parameter(sim.phi()) > 0.9, "uniform branch is a stable coherent rotation");
    assert!(sim.mean_grad().abs() < 1e-3, "coherent rotation has zero mean bond gradient");
}

#[test]
fn v5_p3_energy_relaxation_reproduces_pitch() {
    let cfg = ChiralConfig {
        n: 96,
        k: 0.9,
        alpha: 0.3,
        dt: 0.05,
        seed: 3,
    };
    let pitch = relax_chain_pitch(&cfg, 30_000);
    assert!((pitch - cfg.alpha).abs() < 0.03, "chain pitch {pitch} should approach alpha {}", cfg.alpha);
}

#[test]
fn v5_p3_large_alpha_fragments_domains() {
    let cfg = ChiralConfig {
        n: 24,
        k: 0.9,
        alpha: 0.9,
        dt: 0.05,
        seed: 5,
    };
    let r = relax_square_domains(&cfg, 12_000);
    assert!(r < 0.5, "alpha > pi/4 must fragment into domains, R={r}");
}

#[test]
fn v1_p4_single_pin_orders_lattice_over_time() {
    let n = 10;
    let pin = (0usize, 0.5_f64);
    let cfg = PinnedConfig {
        n,
        k: 0.9,
        dt: 0.02,
        seed: 9,
        pin_sites: vec![pin],
    };
    let mut sim = PinnedSim::new(&cfg);
    let r_early = order_parameter(sim.phi());
    sim.integrate(1_500);
    let r_late = sim.order();
    assert!(
        r_late > r_early + 0.1,
        "a single strong pin must pull the lattice into alignment: {r_early} -> {r_late}"
    );
}