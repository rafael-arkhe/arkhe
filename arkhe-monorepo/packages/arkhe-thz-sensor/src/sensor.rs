//! Dual-band graphene metamaterial THz sensor (analytical surrogate).
//!
//! Implements the article's Lorentzian-on-Drude absorption model, the
//! 495 GHz/RIU analyte sensitivity, Fermi-level tuning and the reduction of a
//! measured spectrum into an 8-dimensional feature vector fed through a real
//! [`arkhe_recurrency::RecurrencyEngine`].

use arkhe_recurrency::{ArousalRegime, RecurrencyEngine, RecurrencyTicket, StimulusId};
use nalgebra::DVector;
use tracing::{debug, warn};

use crate::analyte::Analyte;
use crate::error::ThzSensorError;
use crate::geometry::UnitCellGeometry;
use crate::spectrum::{find_peak, AbsorptionSpectrum};

/// A measured spectrum reduced to a sensor-side anomaly verdict.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ThzMeasurement {
    /// Spectrum of the current analyte.
    pub spectrum: AbsorptionSpectrum,
    /// Spectrum of the baseline reference.
    pub baseline_spectrum: AbsorptionSpectrum,
    /// Band-2 peak shift against baseline, in GHz (the sensing band).
    pub shift_ghz: f64,
    /// Shift normalized to the extreme case (RI 1.40 at 5 µm), `[0, 1]`.
    pub normalized_shift: f64,
    /// Lateral integrity, `1 − 0.8·normalized_shift`.
    pub integrity: f64,
    /// Whether the perturbation cleared the anomaly threshold.
    pub anomaly_detected: bool,
    /// 8-dimensional spectral feature vector fed to the recurrency engine.
    pub features: [f64; crate::FEATURE_DIM],
}

/// A [`RecurrencyTicket`] plus the sensor-side measurement that produced it.
#[derive(Debug, Clone)]
pub struct ThzDetectResult {
    /// Sensor-side anomaly measurement.
    pub measurement: ThzMeasurement,
    /// Ticket emitted by the recurrency engine for this measurement.
    pub ticket: RecurrencyTicket,
}

/// Graphene metamaterial THz sensor.
///
/// Deterministic surrogate of the article's CST + Random-Forest pipeline:
/// the absorption is computed analytically (Lorentzian per band) in
/// nanoseconds, so simulation needs no ML. ML remains the tool for the
/// inverse problem, which this crate does not fake.
pub struct ThzMetamaterialSensor {
    geometry: UnitCellGeometry,
    fermi_level: f64,
    relaxation_time: f64,
    f1_base: f64,
    f2_base: f64,
    a1_base: f64,
    a2_base: f64,
    q1_base: f64,
    q2_base: f64,
    sensitivity_base: f64,
}

impl Default for ThzMetamaterialSensor {
    fn default() -> Self {
        Self::new()
    }
}

impl ThzMetamaterialSensor {
    /// Sensor with the article's default parameters (Fermi level 0.8 eV).
    pub fn new() -> Self {
        Self {
            geometry: UnitCellGeometry::default(),
            fermi_level: 0.8,
            relaxation_time: 0.5,
            f1_base: 2.9215,
            f2_base: 9.0199,
            a1_base: 0.9982,
            a2_base: 0.9988,
            q1_base: 2.6,
            q2_base: 51.9,
            sensitivity_base: 495.0,
        }
    }

    /// Fixed unit-cell geometry of the metamaterial.
    pub fn geometry(&self) -> UnitCellGeometry {
        self.geometry
    }

    /// Current Fermi level of the graphene (eV).
    pub fn fermi_level(&self) -> f64 {
        self.fermi_level
    }

    /// Base sensitivity of the sensing band (GHz/RIU).
    pub fn sensitivity(&self) -> f64 {
        self.sensitivity_base
    }

    /// Tune the graphene Fermi level (dynamic tuning), clamped to `[0.5, 1.0] eV`.
    pub fn set_fermi_level(&mut self, ev: f64) {
        self.fermi_level = ev.clamp(0.5, 1.0);
        debug!("THz sensor: Fermi level set to {:.3} eV", self.fermi_level);
    }

    /// Tune the graphene relaxation time (ps), clamped to `[0.1, 1.0] ps`.
    pub fn set_relaxation_time(&mut self, tau_ps: f64) {
        self.relaxation_time = tau_ps.clamp(0.1, 1.0);
        debug!(
            "THz sensor: relaxation time set to {:.3} ps",
            self.relaxation_time
        );
    }

    /// Map an arousal regime onto a Fermi target — Coma → lowest sensing
    /// sensitivity (0.6 eV), Hypervigilant → full sensing (1.0 eV).
    pub fn set_fermi_from_regime(&mut self, regime: ArousalRegime) {
        let r = regime as u8 as f64;
        let fermi = 0.6 + 0.4 * r / 4.0;
        self.set_fermi_level(fermi);
    }

    /// Resonance frequencies with the Fermi-level tuning applied (THz).
    fn current_resonances(&self) -> (f64, f64) {
        let delta_f = self.fermi_level - 0.8;
        (
            self.f1_base + 0.3 * delta_f,
            self.f2_base + 0.15 * delta_f,
        )
    }

    /// Effective quality factors after relaxation-time broadening.
    fn effective_quality_factors(&self) -> (f64, f64) {
        let dt = 1.0 - self.relaxation_time / 0.5;
        (
            (self.q1_base * (1.0 - 0.1 * dt)).max(1.0),
            (self.q2_base * (1.0 - 0.05 * dt)).max(10.0),
        )
    }

    /// Absorptivity at one frequency for a given analyte, `[0, 1]`.
    pub fn calculate_absorption(
        &self,
        freq_thz: f64,
        analyte: &Analyte,
    ) -> Result<f64, ThzSensorError> {
        if !(0.5..=12.0).contains(&freq_thz) {
            return Err(ThzSensorError::FrequencyOutOfRange(freq_thz));
        }
        let (f1, f2) = self.current_resonances();
        let (q1, q2) = self.effective_quality_factors();

        // Analyte loading: frequency shift + dielectric loss.
        // The high-Q sensing band (2) carries the full 495 GHz/RIU
        // sensitivity; the broad band (1) tracks at 60%.
        let freq_shift_thz =
            (self.sensitivity_base * analyte.delta_ri() * analyte.thickness_factor()) / 1000.0;
        let f1_shifted = f1 + freq_shift_thz * 0.6;
        let f2_shifted = f2 + freq_shift_thz;

        let loss = 1.0 - 0.05 * analyte.delta_ri() * analyte.thickness_factor();
        let a1 = self.a1_base * loss.clamp(0.7, 1.0);
        let a2 = self.a2_base * loss.clamp(0.8, 1.0);

        let lorentzian = |f: f64, f0: f64, a: f64, q: f64| {
            let delta = (f - f0) / f0;
            a / (1.0 + 4.0 * q * q * delta * delta)
        };

        let total = lorentzian(freq_thz, f1_shifted, a1, q1)
            + lorentzian(freq_thz, f2_shifted, a2, q2);
        Ok(total.clamp(0.0, 1.0))
    }

    /// Sweep the full spectrum for an analyte (1.0 → 11.0 THz).
    pub fn calculate_spectrum(&self, analyte: &Analyte) -> AbsorptionSpectrum {
        const STEP: f64 = 0.005;
        let mut freqs = Vec::with_capacity(2000);
        let mut absorption = Vec::with_capacity(2000);
        for i in 0..2000usize {
            let f = 1.0 + i as f64 * STEP;
            freqs.push(f);
            absorption.push(self.calculate_absorption(f, analyte).unwrap_or(0.0));
        }
        let (peak1_freq, peak1_abs) = find_peak(&freqs, &absorption, 2.0, 4.0, STEP);
        let (peak2_freq, peak2_abs) = find_peak(&freqs, &absorption, 8.0, 10.0, STEP);
        AbsorptionSpectrum {
            frequencies: freqs,
            absorption,
            peak1_freq,
            peak1_abs,
            peak2_freq,
            peak2_abs,
        }
    }

    /// Measured sensitivity of a band, in GHz/RIU, over a `delta_ri` step.
    pub fn calculate_sensitivity(&self, band: usize, analyzer_ri: f64, delta_ri: f64) -> f64 {
        let base = Analyte::new(analyzer_ri - delta_ri, 1.0);
        let shifted = Analyte::new(analyzer_ri, 1.0);
        let base_spec = self.calculate_spectrum(&base);
        let shifted_spec = self.calculate_spectrum(&shifted);
        base_spec.sensitivity(&shifted_spec, delta_ri, band)
    }

    /// Measure the current analyte against a baseline reference.
    pub fn measure(&self, baseline: &Analyte, current: &Analyte) -> ThzMeasurement {
        let baseline_spectrum = self.calculate_spectrum(baseline);
        let spectrum = self.calculate_spectrum(current);
        let shift_ghz = baseline_spectrum.peak_shift(&spectrum, 2);

        // Extreme case = RI 1.40 at 5 µm → full-scale shift.
        let max_expected_ghz = self.sensitivity_base * 0.40 * 5.0;
        let normalized_shift = (shift_ghz.abs() / max_expected_ghz).min(1.0);
        let integrity = (1.0 - normalized_shift * 0.8).clamp(0.0, 1.0);
        let anomaly_detected = normalized_shift >= 0.15;

        let (q1, q2) = self.effective_quality_factors();
        let fwhm1 = spectrum.peak1_freq / q1;
        let fwhm2 = spectrum.peak2_freq / q2;
        let features = [
            (spectrum.peak1_freq / 12.0).clamp(0.0, 1.0),
            (spectrum.peak2_freq / 12.0).clamp(0.0, 1.0),
            spectrum.peak1_abs,
            spectrum.peak2_abs,
            (fwhm1 / 2.0).min(1.0),
            (fwhm2 / 1.0).min(1.0),
            normalized_shift,
            (q2 / 60.0).min(1.0),
        ];

        ThzMeasurement {
            spectrum,
            baseline_spectrum,
            shift_ghz,
            normalized_shift,
            integrity,
            anomaly_detected,
            features,
        }
    }

    /// Measure an analyte and feed the spectral features through a real
    /// [`RecurrencyEngine`], emitting a [`RecurrencyTicket`].
    ///
    /// The engine's `PerceptionLoop` must have `n == 8` (the sensor's
    /// [`crate::FEATURE_DIM`]); otherwise a
    /// [`ThzSensorError::DimensionMismatch`] is returned.
    pub fn detect_anomaly(
        &self,
        engine: &mut RecurrencyEngine,
        baseline: &Analyte,
        current: &Analyte,
        stimulus_id: &str,
    ) -> Result<ThzDetectResult, ThzSensorError> {
        if engine.perception.n != crate::FEATURE_DIM {
            return Err(ThzSensorError::DimensionMismatch {
                engine: engine.perception.n,
                sensor: crate::FEATURE_DIM,
            });
        }
        let measurement = self.measure(baseline, current);
        let frame = DVector::from_iterator(crate::FEATURE_DIM, measurement.features.iter().copied());
        let ticket = engine.process_stimulus(StimulusId(stimulus_id.to_string()), &frame);

        if measurement.anomaly_detected {
            warn!(
                "THz sensor: anomaly — band-2 shift {:.2} GHz, integrity {:.3}",
                measurement.shift_ghz, measurement.integrity
            );
        } else {
            debug!(
                "THz sensor: nominal — band-2 shift {:.2} GHz",
                measurement.shift_ghz
            );
        }
        Ok(ThzDetectResult {
            measurement,
            ticket,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn baseline_peaks_match_the_article() {
        let sensor = ThzMetamaterialSensor::new();
        let air = Analyte::air();
        let spec = sensor.calculate_spectrum(&air);
        assert_relative_eq!(spec.peak1_freq, 2.9215, epsilon = 0.01);
        assert_relative_eq!(spec.peak1_abs, 0.9982, epsilon = 0.01);
        assert_relative_eq!(spec.peak2_freq, 9.0199, epsilon = 0.01);
        assert_relative_eq!(spec.peak2_abs, 0.9988, epsilon = 0.01);
    }

    #[test]
    fn analyte_shift_matches_the_495_ghz_per_riu_sensitivity() {
        let sensor = ThzMetamaterialSensor::new();
        let baseline = Analyte::new(1.32, 1.0);
        let shifted = Analyte::new(1.40, 1.0);
        let base_spec = sensor.calculate_spectrum(&baseline);
        let cur_spec = sensor.calculate_spectrum(&shifted);
        // ΔRI = 0.08 at 1 µm → ≈ 39.6 GHz.
        let shift = base_spec.peak_shift(&cur_spec, 2);
        assert!(
            (35.0..45.0).contains(&shift),
            "band-2 shift must be ~39.6 GHz, got {shift:.2} GHz"
        );
    }

    #[test]
    fn fermi_tuning_shifts_the_resonances_upward() {
        let mut sensor = ThzMetamaterialSensor::new();
        let air = Analyte::air();
        let f2_at_08 = sensor.calculate_spectrum(&air).peak2_freq;
        sensor.set_fermi_level(1.0);
        let f2_at_10 = sensor.calculate_spectrum(&air).peak2_freq;
        assert!(f2_at_10 > f2_at_08, "{f2_at_10} must exceed {f2_at_08}");
    }

    #[test]
    fn measured_sensitivity_is_near_495_ghz_per_riu() {
        let sensor = ThzMetamaterialSensor::new();
        let sens = sensor.calculate_sensitivity(2, 1.32, 0.08);
        assert!(
            (450.0..550.0).contains(&sens),
            "band-2 sensitivity must be ~495 GHz/RIU, got {sens:.2}"
        );
    }

    #[test]
    fn fermi_from_regime_maps_monotonically() {
        let mut sensor = ThzMetamaterialSensor::new();
        let regimes = [
            ArousalRegime::Coma,
            ArousalRegime::DeepSleep,
            ArousalRegime::Drowsy,
            ArousalRegime::Alert,
            ArousalRegime::Hypervigilant,
        ];
        let mut previous = 0.0;
        for r in regimes {
            sensor.set_fermi_from_regime(r);
            let f = sensor.fermi_level();
            assert!(f > previous, "{r:?} Fermi {f} must rise");
            previous = f;
        }
        assert!((sensor.fermi_level() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn measure_flags_strong_analyte_as_anomaly() {
        let sensor = ThzMetamaterialSensor::new();
        let air = Analyte::air();
        let clean = sensor.measure(&air, &air);
        assert!(!clean.anomaly_detected);
        assert_eq!(clean.integrity, 1.0);
        assert_eq!(clean.shift_ghz, 0.0);

        let strong = Analyte::new(1.40, 5.0);
        let hit = sensor.measure(&air, &strong);
        assert!(hit.anomaly_detected);
        assert!(hit.integrity < 1.0);
        assert!(hit.shift_ghz > 900.0);
        assert!(hit.features.iter().all(|f| (0.0..=1.0).contains(f)));
    }
}
