//! Entropia Construtiva — I167.
//!
//! Ṽ = S̃ + 𝒜 − Φ_crit − custo
//! onde S̃ = −Σ p_i log₂ p_i + log₂ N (entropia espectral), 𝒜 = anti‑flatness
//! (I159), Φ_crit = limiar de Gödel (I158).

use crate::core::spectral::{compute_anti_flatness, godel_threshold};

/// Entropia construtiva do espectro (I167).
pub fn constructive_entropy(spectrum: &[f64], n_total: usize, n_handovers: usize, alpha: f64) -> f64 {
    let s_tilde = spectral_entropy(spectrum, n_total);
    let a = compute_anti_flatness(spectrum, alpha, 100);
    let phi_crit = godel_threshold(n_total);
    let cost = log2_ceil(n_handovers) as f64;
    s_tilde + a - phi_crit - cost
}

/// S̃ = −Σ p_i log₂ p_i + log₂ N (entropia espectral do espectro).
pub fn spectral_entropy(spectrum: &[f64], n_total: usize) -> f64 {
    let sum: f64 = spectrum.iter().sum();
    if sum <= 0.0 {
        return 0.0;
    }
    let p: Vec<f64> = spectrum.iter().map(|&x| x / sum).collect();
    let shannon: f64 = p
        .iter()
        .map(|&pi| if pi > 0.0 { -pi * pi.log2() } else { 0.0 })
        .sum();
    let log_n = if n_total > 0 { (n_total as f64).log2() } else { 0.0 };
    shannon + log_n
}

/// ⌈log₂ n⌉, com n=0 → 0.
pub fn log2_ceil(n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    (n as f64).log2().ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_shannon() {
        let e = spectral_entropy(&[0.5, 0.5], 4);
        assert!((e - (1.0 + 2.0)).abs() < 1e-9);
    }

    #[test]
    fn test_entropy_nonneg() {
        let spec = vec![0.25, 0.25, 0.25, 0.25];
        assert!(spectral_entropy(&spec, 16) >= 0.0);
    }

    #[test]
    fn test_constructive_entropy_finite() {
        let spec = vec![0.2, 0.3, 0.5];
        let e = constructive_entropy(&spec, 8, 12, 1.5);
        assert!(e.is_finite());
    }

    #[test]
    fn test_log2_ceil() {
        assert_eq!(log2_ceil(0), 0);
        assert_eq!(log2_ceil(1), 0);
        assert_eq!(log2_ceil(2), 1);
        assert_eq!(log2_ceil(4), 2);
    }
}