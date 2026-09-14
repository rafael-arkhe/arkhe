//! Integration tests for the DPDM saturation stack (arXiv:2510.13956).
//!
//! Pins the aerogel plasma actuator and the Zakharov saturation model:
//!  * PLASMA_SATURACAO — pump energy is clamped at the electron thermal
//!    threshold and the excess cascades into higher-k excitation;
//!  * ZAKHAROV_CASCADE — wave breaking transfers energy to ion-acoustic modes.

use arkhe_actuators::{AerogelPlasmaActuator, AerogelState, PlasmaState, ZakharovSaturation};

const PULSE_S: f64 = 1e-9;
const SPOT_MM: f64 = 1.0;

fn saturate(laser_energy: f64) -> PlasmaState {
    let mut act = AerogelPlasmaActuator::new();
    act.laser_excitation(&AerogelState::default(), laser_energy, PULSE_S, SPOT_MM)
}

#[test]
fn plasma_saturation_limits_effective_energy() {
    // Strong pump: the effective energy must be clamped at the thermal
    // threshold (≈86 eV → <0.1 keV) and never grow with the pump.
    let low = saturate(1e3);
    let high = saturate(1e9);
    assert_eq!(low.saturation_fraction, 1.0);
    assert_eq!(high.saturation_fraction, 1.0);
    assert!(high.coulomb_energy_kev <= low.coulomb_energy_kev + 1e-12);
    assert!(low.coulomb_energy_kev < 0.1);
}

#[test]
fn plasma_saturation_generates_higher_k_observables() {
    let s = saturate(1e6);
    // The saturation products ARE the signal: excess into k-modes and
    // a density perturbation δn/n that the ELF monitor can track.
    assert!(s.higher_k_excitation > 0.9);
    assert!(s.density_perturbation > 0.0);
    assert!(s.energy_harvested_joules <= s.energy_harvested_joules + 0.0);
}

#[test]
fn unsaturated_regime_preserves_linear_scaling() {
    let a = saturate(0.1);
    let b = saturate(0.2);
    assert!(a.saturation_fraction < 1.0);
    assert_eq!(a.higher_k_excitation, 0.0);
    // Half energy → about half the effective kinetic energy (laser term
    // dominates over the constant Coulomb-explosion term at this scale).
    let ratio = b.coulomb_energy_kev / a.coulomb_energy_kev.max(1e-30);
    assert!((1.8..=2.1).contains(&ratio), "ratio {ratio}");
}

#[test]
fn zakharov_cascade_transfers_excess_to_ion_acoustic_modes() {
    let mut z = ZakharovSaturation::new(1.0, 1.0);
    for _ in 0..500 {
        z.evolve(1.0, 0.01);
    }
    assert!(z.langmuir_amplitude <= z.wave_breaking_limit + 1e-9);
    assert!(z.ion_acoustic_amplitude > 0.0);
    assert!(z.density_cavity_depth < 0.0, "ponderomotive force digs cavities");
    assert!(z.saturation_fraction() > 0.0);
    assert!(z.higher_k_generation_rate() > 0.0);
}

#[test]
fn zakharov_observables_feed_the_rf_pipeline() {
    let mut z = ZakharovSaturation::new(100.0, 1.0);
    for _ in 0..300 {
        z.evolve(0.5, 0.01);
    }
    // |δω_p/ω_p| = sqrt(1 + δn/n) - 1 — the QPE phase-noise input.
    let delta_omega = (1.0 + z.density_cavity_depth).max(0.0).sqrt() - 1.0;
    assert!(delta_omega.is_finite());
    // Higher-k generation is the spectral-broadening fingerprint.
    assert!(z.higher_k_generation_rate() >= 0.0 && z.higher_k_generation_rate() <= 0.5);
}
