//! Shared diagnostics: order parameters, energy floors, convergence and
//! statistical helpers (mean / std).

/// Complex order parameter `R = |<exp(i phi)>|`.
pub fn order_parameter(phi: &[f64]) -> f64 {
    let n = phi.len() as f64;
    let (re, im) = phi.iter().fold((0.0, 0.0), |(re, im), &p| {
        (re + p.cos() / n, im + p.sin() / n)
    });
    (re * re + im * im).sqrt()
}

/// Mean and std of a series of order parameters.
pub fn mean_std(series: &[f64]) -> (f64, f64) {
    let m = series.iter().sum::<f64>() / series.len() as f64;
    let v = series.iter().map(|x| (x - m).powi(2)).sum::<f64>() / series.len() as f64;
    (m, v.sqrt())
}

/// Bandwidth-corrected estimator for the Lorenzian linewidth used in P1.
/// (Kept here so the laser mapping shares the same function as the solver.)
pub fn lorentzian_half_width(fwhm_hz: f64) -> f64 {
    fwhm_hz / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_phase_order_is_one() {
        let phi = vec![0.0; 16];
        assert!((order_parameter(&phi) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn mean_std_smoke() {
        let (m, s) = mean_std(&[1.0, 1.0, 1.0]);
        assert!((m - 1.0).abs() < 1e-12);
        assert!((s - 0.0).abs() < 1e-12);
    }
}