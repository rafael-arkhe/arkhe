//! Estrutura Espectral da Coerência — Invariantes I157–I160.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Φ_irr = (3/2) log₂ ln N, saturando em 1.0  (I157).
pub fn irreducible_coherence(n: usize) -> f64 {
    if n <= 2 {
        return 0.0;
    }
    let n_f64 = n as f64;
    (1.5 * (n_f64.ln()).log2()).min(1.0)
}

/// Limiar de Gödel normalizado: Φ_crit = 1/log₂ N  (I158).
pub fn godel_threshold(n: usize) -> f64 {
    if n <= 2 {
        return 0.0;
    }
    let n_f64 = n as f64;
    (1.0 / n_f64.log2()).min(1.0)
}

/// Verifica se o espectro já cruzou o limiar de Gödel  (I158).
pub fn is_godel_threshold_reached(spectrum: &[f64]) -> bool {
    let n = spectrum.len();
    if n <= 2 {
        return false;
    }
    compute_phi_irr(spectrum) >= godel_threshold(n)
}

/// Anti‑flatness 𝒜_α do espectro  (I159).
pub fn compute_anti_flatness(phi: &[f64], alpha: f64, max_samples: usize) -> f64 {
    let n = phi.len();
    if n < 2 {
        return 0.0;
    }

    let sum: f64 = phi.iter().sum();
    if sum <= 0.0 {
        return 0.0;
    }

    let p: Vec<f64> = phi.iter().map(|&x| x / sum).collect();
    let sample_size = n.min(max_samples);
    if sample_size < 2 {
        return 0.0;
    }

    let p_pow: Vec<f64> = p
        .iter()
        .take(sample_size)
        .map(|&pi| if pi > 1e-12 { pi.powf(alpha - 1.0) } else { 0.0 })
        .collect();

    let mut total = 0.0;
    for i in 0..sample_size {
        let pi = p[i];
        let p_pow_i = p_pow[i];
        for j in (i + 1)..sample_size {
            let diff = p_pow_i - p_pow[j];
            total += pi * p[j] * diff * diff;
        }
    }

    if sample_size < n {
        total * (n as f64 / sample_size as f64)
    } else {
        total
    }
}

/// Organiza o espectro em cascas diádicas: níveis ⌊log₂ i⌋  (I160).
pub fn organize_in_shells(spectrum: &[f64]) -> BTreeMap<usize, Vec<f64>> {
    let mut shells: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
    for (idx, &val) in spectrum.iter().enumerate() {
        shells
            .entry(handover_level(idx + 1))
            .or_insert_with(Vec::new)
            .push(val);
    }
    shells
}

/// Nível de casca diádica do i‑ésimo handover (1‑based)  (I160).
pub fn handover_level(idx: usize) -> usize {
    if idx == 0 {
        0
    } else {
        (idx as f64).log2().floor() as usize
    }
}

/// Casca diádica j: { i | 2^j ≤ i < 2^(j+1) }  (I160).
pub fn diadic_shell(j: usize, max: usize) -> Vec<usize> {
    let lo = 1usize << j;
    let hi = 1usize << (j + 1);
    (lo..hi.min(max + 1)).collect()
}

/// Um handover espectral pronto para ser encadeado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectralHandover {
    /// Φ_irr — coerência irreduzível (I157).
    pub phi_irr: f64,
    /// Φ_crit — limiar de Gödel (I158).
    pub phi_crit: f64,
    /// Anti‑flatness (I159).
    pub anti_flatness: f64,
    /// Distribuição por cascas diádicas (I160).
    pub shells: BTreeMap<usize, Vec<f64>>,
    /// Source label (ex.: "TCameraS3").
    pub source: String,
}

/// Extrai um handover espectral completo a partir de um espectro (I157–I160).
pub fn extract_spectral_handover(spectrum: &[f64], source: impl Into<String>) -> SpectralHandover {
    SpectralHandover {
        phi_irr: compute_phi_irr(spectrum),
        phi_crit: godel_threshold(spectrum.len()),
        anti_flatness: compute_anti_flatness(spectrum, 1.5, 100),
        shells: organize_in_shells(spectrum),
        source: source.into(),
    }
}

fn compute_phi_irr(spectrum: &[f64]) -> f64 {
    let n = spectrum.len();
    if n <= 2 {
        return 0.0;
    }
    let sum: f64 = spectrum.iter().sum();
    if sum <= 0.0 {
        return 0.0;
    }
    // Normaliza antes de computar a Rényi-2.
    let p: Vec<f64> = spectrum.iter().map(|&x| x / sum).collect();
    let sum_sq: f64 = p.iter().map(|&x| x * x).sum();
    let renyi_2 = -sum_sq.log2();
    renyi_2.min(irreducible_coherence(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_irreducible_scaling() {
        // N=10 satura em 1.0 (1.5·log₂(ln10) ≈ 1.81).
        assert_eq!(irreducible_coherence(10), 1.0);
        assert_eq!(irreducible_coherence(2), 0.0);
        assert!(irreducible_coherence(1_000_000) <= 1.0);
        // N=4: 1.5·log₂(ln4) ≈ 0.707 — não satura.
        let expected = 1.5 * (4.0f64.ln()).log2();
        assert!((irreducible_coherence(4) - expected).abs() < 1e-12);
    }

    #[test]
    fn test_godel_threshold() {
        let crit = godel_threshold(100);
        assert!((crit - 1.0 / (100.0f64.log2())).abs() < 1e-12);
        assert_eq!(godel_threshold(1), 0.0);
    }

    #[test]
    fn test_godel_threshold_reached() {
        let spectrum = vec![0.9, 0.9, 0.9, 0.9];
        assert!(is_godel_threshold_reached(&spectrum));
    }

    #[test]
    fn test_diadic_shells() {
        let spec = vec![0.1, 0.2, 0.3, 0.4];
        let shells = organize_in_shells(&spec);
        assert!(shells.contains_key(&0));
        assert!(shells.contains_key(&1));
        assert_eq!(handover_level(1), 0);
        assert_eq!(handover_level(2), 1);
        assert_eq!(handover_level(4), 2);
    }

    #[test]
    fn test_anti_flatness_nonneg() {
        let spec = vec![0.1, 0.2, 0.3, 0.4];
        assert!(compute_anti_flatness(&spec, 1.5, 100) >= 0.0);
    }

    #[test]
    fn test_extract_spectral_handover() {
        let spec = vec![0.2, 0.3, 0.5];
        let h = extract_spectral_handover(&spec, "TCameraS3");
        assert_eq!(h.source, "TCameraS3");
        assert!(h.phi_irr > 0.0);
        assert!(h.phi_crit > 0.0);
        assert!(!h.shells.is_empty());
    }
}