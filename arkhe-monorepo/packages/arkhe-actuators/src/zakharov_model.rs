//! Zakharov non-linear saturation model for laboratory plasmas.
//!
//! Couples Langmuir waves `E` with ion-acoustic density fluctuations `δn/n₀`:
//!
//! ```text
//! i ∂E/∂t + (3/2) ω_p λ_D² ∇²E = (ω_p/2) (δn/n₀) E
//! ∂²/∂t² (δn/n₀) - c_s² ∇² (δn/n₀) = (1/(4π n₀ m_i)) ∇² |E|²
//! ```
//!
//! When the Langmuir amplitude reaches the wave-breaking limit, the excess
//! energy cascades into higher-k ion-acoustic modes (DPDM saturation, see
//! arXiv:2510.13956). The model is deterministic and exposes the three
//! saturation observables consumed by the RF/ELF pipeline.

/// Deterministic Zakharov saturation integrator.
#[derive(Debug, Clone, Copy)]
pub struct ZakharovSaturation {
    /// Electron temperature (eV).
    pub electron_temp_ev: f64,
    /// Ion temperature (eV).
    pub ion_temp_ev: f64,
    /// Ion-to-electron mass ratio.
    pub mass_ratio: f64,
    /// Langmuir wave electric-field amplitude.
    pub langmuir_amplitude: f64,
    /// Ion-acoustic wave amplitude.
    pub ion_acoustic_amplitude: f64,
    /// Density cavity depth δn/n₀.
    pub density_cavity_depth: f64,
    /// Wave-breaking limit for the Langmuir amplitude.
    pub wave_breaking_limit: f64,
    /// Modulational-instability threshold for δn/n₀.
    pub modulational_instability_threshold: f64,
}

impl ZakharovSaturation {
    pub fn new(te_ev: f64, ti_ev: f64) -> Self {
        let mass_ratio = 1836.0 * 2.0; // deuterium
        Self {
            electron_temp_ev: te_ev,
            ion_temp_ev: ti_ev,
            mass_ratio,
            langmuir_amplitude: 0.0,
            ion_acoustic_amplitude: 0.0,
            density_cavity_depth: 0.0,
            wave_breaking_limit: 1.0,
            modulational_instability_threshold: 0.1,
        }
    }

    /// Evolve the coupled system by `dt` under a pump of given amplitude.
    pub fn evolve(&mut self, pump_amplitude: f64, dt: f64) {
        let damping = 0.01; // Landau damping
        let nonlinear_coupling = self.density_cavity_depth * self.langmuir_amplitude;

        // Langmuir equation driven by the pump (DPDM / retrocausal signal).
        let drive =
            pump_amplitude - damping * self.langmuir_amplitude
                - nonlinear_coupling * self.ion_acoustic_amplitude;
        self.langmuir_amplitude += drive * dt;

        // Ion-acoustic equation (density) driven by the ponderomotive force.
        let ponderomotive_force = -0.5 * self.langmuir_amplitude.powi(2);
        let ion_oscillation =
            (self.electron_temp_ev / self.ion_temp_ev.max(1e-12)).sqrt()
                * self.density_cavity_depth;
        self.density_cavity_depth += (ponderomotive_force - ion_oscillation) * dt;

        // Saturation: clamp the Langmuir amplitude at wave breaking and
        // cascade the excess into higher-k ion-acoustic modes.
        let excess = self.langmuir_amplitude - self.wave_breaking_limit;
        if excess > 0.0 {
            self.langmuir_amplitude = self.wave_breaking_limit;
            self.ion_acoustic_amplitude += excess * 0.1;
        }

        // Modulational instability: density cavities deepen once triggered.
        if self.density_cavity_depth > self.modulational_instability_threshold {
            self.density_cavity_depth *= 1.0 + dt * 0.01;
        }
    }

    /// Saturation fraction (0..1) relative to the electron thermal energy.
    pub fn saturation_fraction(&self) -> f64 {
        let energy = self.langmuir_amplitude.powi(2);
        let thermal_limit = (self.electron_temp_ev / 1000.0).max(1e-12);
        (energy / thermal_limit).min(1.0)
    }

    /// Rate at which energy is transferred into higher-k modes (0..0.5).
    pub fn higher_k_generation_rate(&self) -> f64 {
        self.saturation_fraction() * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pump_drives_amplitude_to_wave_breaking() {
        let mut z = ZakharovSaturation::new(100.0, 1.0);
        for _ in 0..400 {
            z.evolve(0.5, 0.01);
        }
        assert!(z.langmuir_amplitude > 0.9);
        assert!(z.langmuir_amplitude <= z.wave_breaking_limit + 1e-12);
        assert!(z.ion_acoustic_amplitude > 0.0, "excess cascades into k-modes");
    }

    #[test]
    fn saturation_fraction_bounded() {
        let mut z = ZakharovSaturation::new(1.0, 1.0);
        for _ in 0..100 {
            z.evolve(1.0, 0.01);
        }
        let frac = z.saturation_fraction();
        assert!((0.0..=1.0).contains(&frac));
        assert!((0.0..=0.5).contains(&z.higher_k_generation_rate()));
    }

    #[test]
    fn generation_rate_is_monotonic_in_saturation() {
        let mut z = ZakharovSaturation::new(1.0, 1.0);
        let mut prev = 0.0;
        for _ in 0..200 {
            let rate = z.higher_k_generation_rate();
            assert!(rate + 1e-12 >= prev, "rate must not decrease");
            prev = rate;
            z.evolve(1.0, 0.01);
        }
    }
}
