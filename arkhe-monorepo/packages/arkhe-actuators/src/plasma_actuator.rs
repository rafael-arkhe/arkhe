//! DPDM-saturated plasma actuator — aerogel inertial-electrostatic target.
//!
//! Implements the non-linear saturation model of arXiv:2510.13956 (Hook,
//! Huang & Shalaby, 2025): the energy deposited in the plasma cannot exceed
//! the electron thermal energy, and the excess is diverted into higher-k
//! mode excitation (Langmuir / ion-acoustic cascade). Saturation is not a
//! failure mode — it is the observable signature of resonant conversion.
//!
//! Invariant impact: feeds Gap-1 (Φ_C bounds) via `saturation_fraction` and
//! `coherence_boost`, and Runtime-3 (healthcheck) via deterministic outputs.

use std::f64::consts::PI;

/// Physical constants used by the actuator.
pub mod constants {
    /// Elementary charge (C).
    pub const E_CHARGE: f64 = 1.602176634e-19;
    /// Deuteron mass (kg).
    pub const DEUTERIUM_MASS: f64 = 3.34358377e-27;
    /// Electron mass (kg).
    pub const ELECTRON_MASS: f64 = 9.1093837015e-31;
    /// Coulomb constant (N·m²/C²).
    pub const COULOMB_CONST: f64 = 8.987551792e9;
    /// Boltzmann constant (J/K).
    pub const BOLTZMANN: f64 = 1.380649e-23;
    /// 1 MeV in joules.
    pub const JOULE_PER_MEV: f64 = 1.602176634e-13;
}

/// Aerogel target geometry and composition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AerogelState {
    /// Bulk density of the aerogel (kg/m³).
    pub density: f64,
    /// Target thickness along the beam axis (mm).
    pub thickness_mm: f64,
    /// Mass fraction of deuterium in the target.
    pub deuterium_fraction: f64,
}

impl AerogelState {
    pub fn new(density: f64, thickness_mm: f64, deuterium_fraction: f64) -> Self {
        Self {
            density,
            thickness_mm,
            deuterium_fraction,
        }
    }
}

impl Default for AerogelState {
    /// Reference aerogel: low-density silica, 0.1 mm thick, 10% deuterium.
    fn default() -> Self {
        Self::new(5.0, 0.1, 0.1)
    }
}

/// State of the plasma immediately after a laser pulse, extended with the
/// DPDM saturation observables.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlasmaState {
    /// Effective temperature after saturation (eV).
    pub temperature_ev: f64,
    /// Ion number density (m⁻³).
    pub ion_density: f64,
    /// Effective Coulomb/kinetic energy after saturation (keV).
    pub coulomb_energy_kev: f64,
    /// Neutron yield per pulse (2.45 MeV D–D).
    pub neutron_yield: f64,
    /// Coherence boost (λ₂ proxy), capped at 0.01.
    pub coherence_boost: f64,
    /// Recovered energy (J).
    pub energy_harvested_joules: f64,
    /// Per-pair fusion probability.
    pub fusion_probability: f64,
    /// Saturation fraction 0..1 — how close to the thermal threshold.
    pub saturation_fraction: f64,
    /// Fraction of pump energy diverted to higher-k modes.
    pub higher_k_excitation: f64,
    /// Density perturbation δn/n (inhomogeneity).
    pub density_perturbation: f64,
}

/// Deterministic aerogel plasma actuator (no RNG).
pub struct AerogelPlasmaActuator {
    cache_volume: f64,
    cache_mass: f64,
}

impl Default for AerogelPlasmaActuator {
    fn default() -> Self {
        Self::new()
    }
}

impl AerogelPlasmaActuator {
    pub fn new() -> Self {
        Self {
            cache_volume: 0.0,
            cache_mass: 0.0,
        }
    }

    /// Illuminated cylinder volume (m³) for a circular spot.
    fn compute_volume(spot_diameter_mm: f64, thickness_mm: f64) -> f64 {
        let radius_m = (spot_diameter_mm.max(0.0) / 2.0) * 1e-3;
        let thickness_m = thickness_mm.max(0.0) * 1e-3;
        PI * radius_m * radius_m * thickness_m
    }

    /// Deposited-energy neutral temperature for the electron population (eV).
    fn electron_thermal_kev() -> f64 {
        // 1e6 K ≈ 86.2 eV — typical electron temperature in lab laser plasmas.
        const ELECTRON_TEMP_K: f64 = 1.0e6;
        (ELECTRON_TEMP_K / 11604.0) / 1000.0
    }

    /// Run one laser pulse on the target and return the saturated plasma state.
    pub fn laser_excitation(
        &mut self,
        aerogel: &AerogelState,
        laser_energy: f64,
        pulse_duration: f64,
        spot_diameter_mm: f64,
    ) -> PlasmaState {
        let volume_m3 = Self::compute_volume(spot_diameter_mm, aerogel.thickness_mm);
        self.cache_volume = volume_m3;
        let total_mass_kg = aerogel.density.max(0.0) * volume_m3;
        let deuterium_mass_kg = total_mass_kg * aerogel.deuterium_fraction.clamp(0.0, 1.0);
        let n_ions = (deuterium_mass_kg / constants::DEUTERIUM_MASS).max(1.0);
        let ion_density = n_ions / volume_m3.max(1e-30);
        self.cache_mass = deuterium_mass_kg;

        // Initial kinetic energy from Coulomb explosion.
        let ion_energy_j = (laser_energy.max(0.0) * 0.10) / n_ions;
        let r_avg = (1.0 / ion_density.max(1e10)).powf(1.0 / 3.0);
        let coulomb_energy_j = constants::COULOMB_CONST
            * (1.0 * constants::E_CHARGE).powi(2)
            / r_avg;
        let kinetic_energy_j = ion_energy_j + coulomb_energy_j * 0.1;
        let kinetic_energy_kev = kinetic_energy_j / constants::JOULE_PER_MEV * 1000.0;

        // --- DPDM non-linear saturation ---
        let thermal_energy_kev = Self::electron_thermal_kev();
        let saturation_fraction = (kinetic_energy_kev / thermal_energy_kev.max(1e-12)).min(1.0);
        let effective_kev = if saturation_fraction < 1.0 {
            kinetic_energy_kev
        } else {
            thermal_energy_kev
        };

        // Excess energy is diverted into higher-k mode excitation.
        let excess_energy = (kinetic_energy_kev - thermal_energy_kev).max(0.0);
        let higher_k_excitation = if kinetic_energy_kev > 0.0 {
            (excess_energy / kinetic_energy_kev).min(1.0)
        } else {
            0.0
        };

        // Density inhomogeneity δn/n scales with saturation (≤10%).
        let density_perturbation = saturation_fraction * 0.1;

        // Neutron yield, degraded by saturation and the inhomogeneous density.
        let (neutron_yield, fusion_prob) = self.fusion_yield(
            effective_kev,
            ion_density * (1.0 - density_perturbation),
            pulse_duration,
        );

        // Coherence boost: limited by saturation.
        let base_coherence = if neutron_yield > 0.0 {
            0.001 * (neutron_yield + 1.0).ln() / 10.0_f64.ln()
        } else {
            0.0
        };
        let coherence_boost = (base_coherence * (1.0 - saturation_fraction * 0.8)).min(0.01);

        // Harvested energy, reduced by saturation.
        let total_energy_j = neutron_yield * 2.45 * constants::JOULE_PER_MEV;
        let harvested = total_energy_j * 0.1 * (1.0 - saturation_fraction * 0.5);

        PlasmaState {
            temperature_ev: effective_kev * 1000.0,
            ion_density,
            coulomb_energy_kev: effective_kev,
            neutron_yield,
            coherence_boost: coherence_boost.max(0.0),
            energy_harvested_joules: harvested.max(0.0),
            fusion_probability: fusion_prob,
            saturation_fraction,
            higher_k_excitation,
            density_perturbation,
        }
    }

    /// D–D fusion yield estimate (2.45 MeV neutrons) from a saturated plasma.
    fn fusion_yield(&self, kinetic_kev: f64, density: f64, duration: f64) -> (f64, f64) {
        let _ = duration; // steady-state per-pulse estimate
        let energy_scaling = (kinetic_kev / 100.0).clamp(0.0, 1.0);
        let sigma_eff = 1e-30 * energy_scaling;
        let path_length = 1e-6; // mean chord in the dense core (µm-scale)
        let reaction_prob = (sigma_eff * density * path_length).min(1.0);
        let n_pairs = (density * self.cache_volume) / 2.0;
        let efficiency = 0.01 * (1.0 - 0.9 * (kinetic_kev / 100.0).min(1.0));
        let yield_raw = n_pairs * reaction_prob * efficiency;
        (yield_raw.clamp(0.0, 1e15), reaction_prob)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_is_cylindrical() {
        let v = AerogelPlasmaActuator::compute_volume(2.0, 1.0);
        let radius_m: f64 = 1e-3;
        let expected = PI * radius_m * radius_m * 1e-3;
        assert!((v - expected).abs() < 1e-18);
    }

    #[test]
    fn high_power_saturates_at_thermal_limit() {
        let mut act = AerogelPlasmaActuator::new();
        let state = act.laser_excitation(&AerogelState::default(), 1e6, 1e-9, 1.0);
        assert_eq!(state.saturation_fraction, 1.0);
        assert!(state.higher_k_excitation > 0.9);
        assert!(state.density_perturbation > 0.0);
        // Effective energy clamped to the electron thermal energy (86 eV).
        assert!(state.coulomb_energy_kev < 0.1);
        assert!(state.temperature_ev < 100.0);
    }

    #[test]
    fn low_power_stays_unsaturated() {
        let mut act = AerogelPlasmaActuator::new();
        let state = act.laser_excitation(&AerogelState::default(), 1e-9, 1e-9, 1.0);
        assert!(state.saturation_fraction < 1.0);
        assert_eq!(state.higher_k_excitation, 0.0);
    }

    #[test]
    fn outputs_are_finite_and_bounded() {
        let mut act = AerogelPlasmaActuator::new();
        for &energy in &[0.0, 1e-6, 1.0, 1e3, 1e9] {
            let s = act.laser_excitation(&AerogelState::default(), energy, 1e-9, 1.0);
            assert!(s.saturation_fraction.is_finite());
            assert!((0.0..=1.0).contains(&s.saturation_fraction));
            assert!((0.0..=1.0).contains(&s.higher_k_excitation));
            assert!((0.0..=1.0).contains(&s.density_perturbation));
            assert!(s.ion_density > 0.0 && s.ion_density.is_finite());
            assert!(s.neutron_yield.is_finite() && s.neutron_yield >= 0.0);
            assert!((0.0..=0.01).contains(&s.coherence_boost));
        }
    }
}
