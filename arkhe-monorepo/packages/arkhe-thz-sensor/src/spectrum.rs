//! Absorption spectrum of the dual-band sensor.

/// Absorption spectrum computed by the analytical model.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AbsorptionSpectrum {
    /// Sampled frequencies (THz).
    pub frequencies: Vec<f64>,
    /// Absorptivity at each sample, `[0, 1]`.
    pub absorption: Vec<f64>,
    /// Interpolated peak of band 1.
    pub peak1_freq: f64,
    /// Interpolated peak absorption of band 1.
    pub peak1_abs: f64,
    /// Interpolated peak of band 2.
    pub peak2_freq: f64,
    /// Interpolated peak absorption of band 2.
    pub peak2_abs: f64,
}

impl AbsorptionSpectrum {
    /// Peak-frequency shift of the given band relative to `other`, in GHz.
    ///
    /// `band` is 1-indexed: `1` → band 1, `2` → band 2.
    pub fn peak_shift(&self, other: &Self, band: usize) -> f64 {
        let (a, b) = match band {
            1 => (self.peak1_freq, other.peak1_freq),
            2 => (self.peak2_freq, other.peak2_freq),
            _ => return 0.0,
        };
        (b - a) * 1000.0
    }

    /// Sensitivity of the given band, `GHz / ΔRI`.
    ///
    /// `delta_ri` is the refractive-index step between the two spectra.
    pub fn sensitivity(&self, other: &Self, delta_ri: f64, band: usize) -> f64 {
        let shift = self.peak_shift(other, band);
        if delta_ri.abs() < 1e-9 {
            0.0
        } else {
            shift / delta_ri
        }
    }
}

/// Locate a peak inside `[lo, hi]` and refine it with a three-point parabola.
///
/// The sweep is uniform with step `h`, so the classic vertex formula for an
/// equally spaced abscissa is used, giving sub-resolution peak locations.
pub(crate) fn find_peak(freqs: &[f64], abs: &[f64], lo: f64, hi: f64, h: f64) -> (f64, f64) {
    let mut best_i = 0usize;
    let mut best = f64::NEG_INFINITY;
    for (i, f) in freqs.iter().enumerate() {
        if *f >= lo && *f <= hi && abs[i] > best {
            best = abs[i];
            best_i = i;
        }
    }
    if best_i == 0 || best_i + 1 >= abs.len() {
        return (freqs[best_i], abs[best_i]);
    }
    let (y0, y1, y2) = (abs[best_i - 1], abs[best_i], abs[best_i + 1]);
    let denom = y0 - 2.0 * y1 + y2;
    if denom.abs() < 1e-12 {
        return (freqs[best_i], y1);
    }
    let x1 = freqs[best_i];
    let x_peak = x1 + h * (y0 - y2) / (2.0 * denom);
    let y_peak = y1 - (y0 - y2).powi(2) / (8.0 * denom);
    (x_peak, y_peak.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parabola_refines_a_lorentzian_peak() {
        // Synthetic Lorentzian centred at 9.0199 THz, Q = 51.9.
        let h = 0.005;
        let f0 = 9.0199;
        let q = 51.9;
        let lorentz = |f: f64| 0.9988 / (1.0 + 4.0 * q * q * ((f - f0) / f0).powi(2));
        let freqs: Vec<f64> = (0..2000).map(|i| 1.0 + i as f64 * h).collect();
        let abs: Vec<f64> = freqs.iter().map(|f| lorentz(*f)).collect();
        let (f, a) = find_peak(&freqs, &abs, 8.0, 10.0, h);
        assert!((f - f0).abs() < 1e-3, "peak at {f}, expected ~{f0}");
        assert!((a - 0.9988).abs() < 1e-3, "peak abs {a}");
    }

    #[test]
    fn peak_shift_is_in_ghz() {
        let mut a = AbsorptionSpectrum {
            frequencies: vec![0.0],
            absorption: vec![0.0],
            peak1_freq: 2.9215,
            peak1_abs: 0.9982,
            peak2_freq: 9.0199,
            peak2_abs: 0.9988,
        };
        let mut b = a.clone();
        b.peak2_freq = 9.0599;
        assert!((a.peak_shift(&b, 2) - 40.0).abs() < 1e-9);
        a.peak1_freq = 2.9215;
        b.peak1_freq = 2.9215;
        assert_eq!(a.peak_shift(&b, 1), 0.0);
    }
}
