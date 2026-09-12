//! Constantes da primeira medição JUNO publicada na Nature (módulo #20).
//!
//! Fonte: **The JUNO Collaboration, "Measurement of reactor neutrino oscillation
//! with the first JUNO data", Nature 654, 343–348 (2026)**,
//! DOI 10.1038/s41586-026-10538-z (cover, 10 de junho de 2026).
//!
//! Utilizando 59,1 dias de dados desde a conclusão do detector (agosto de 2025),
//! JUNO determinou simultaneamente:
//! - sin²θ₁₂ = 0.3092 ± 0.0087
//! - Δm²₂₁ = (7.50 ± 0.12) × 10⁻⁵ eV²
//!
//! (cenário de ordenamento de massa normal), melhorando a precisão por um fator de
//! 1.6 relativamente à combinação de todas as medidas anteriores.
//!
//! Detector: cintilador líquido de 20 kton, 52,5 km de múltiplos núcleos de
//! reatores — desenhado para resolver o padrão de interferência de neutrinos de
//! reator com precisão sub-porcentual.

use serde::{Deserialize, Serialize};

/// sin²θ₁₂ medido (cenário normal).
pub const JUNO_SIN2_THETA12: f64 = 0.3092;
pub const JUNO_SIN2_THETA12_ERR: f64 = 0.0087;
/// Δm²₂₁ medido em eV².
pub const JUNO_DELTA_M21_SQ_EV2: f64 = 7.50e-5;
pub const JUNO_DELTA_M21_ERR: f64 = 0.12e-5;
/// Linha de base do experimento (km).
pub const JUNO_BASELINE_KM: f64 = 52.5;
/// Massa alvo (kton).
pub const JUNO_DETECTOR_KT: f64 = 20.0;
/// Exposição (dias).
pub const JUNO_DAYS: f64 = 59.1;
/// Fator de melhora de precisão sobre medidas anteriores combinadas.
pub const JUNO_PRECISION_IMPROVEMENT: f64 = 1.6;
/// Referência completa (Provenance-1).
pub const JUNO_NATURE_REF: &str =
    "The JUNO Collaboration, 'Measurement of reactor neutrino oscillation with the first JUNO data', Nature 654:343-348 (2026), DOI 10.1038/s41586-026-10538-z";

/// Medida JUNO de dois parâmetros de oscilação.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct JunoMeasurement {
    pub sin2_theta12: f64,
    pub sin2_theta12_err: f64,
    pub delta_m21_sq_ev2: f64,
    pub delta_m21_err_ev2: f64,
    pub baseline_km: f64,
    pub detector_kt: f64,
    pub days: f64,
    pub normal_mass_ordering: bool,
}

impl JunoMeasurement {
    /// Os dois parâmetros publicados na Nature 654 (2026).
    pub fn nature_first_data() -> Self {
        Self {
            sin2_theta12: JUNO_SIN2_THETA12,
            sin2_theta12_err: JUNO_SIN2_THETA12_ERR,
            delta_m21_sq_ev2: JUNO_DELTA_M21_SQ_EV2,
            delta_m21_err_ev2: JUNO_DELTA_M21_ERR,
            baseline_km: JUNO_BASELINE_KM,
            detector_kt: JUNO_DETECTOR_KT,
            days: JUNO_DAYS,
            normal_mass_ordering: true,
        }
    }

    /// sin²2θ₁₂ e sua propagação de erro a 1σ.
    pub fn sin2_2theta12(&self) -> (f64, f64) {
        let s = self.sin2_theta12;
        let v = 4.0 * s * (1.0 - s);
        let dv = 4.0 * (1.0 - 2.0 * s).abs() * self.sin2_theta12_err;
        (v, dv)
    }

    /// O par (sin²θ₁₂, Δm²₂₁) conhece os limites 1σ publicados.
    pub fn consistent_with_before(&self, sin2_theta12: f64, dm21: f64) -> bool {
        (sin2_theta12 - self.sin2_theta12).abs() <= 3.0 * self.sin2_theta12_err
            && (dm21 - self.delta_m21_sq_ev2).abs() <= 3.0 * self.delta_m21_err_ev2
    }
}

/// Retorna a medida canônica.
pub fn juno_spectrum_measurement() -> JunoMeasurement {
    JunoMeasurement::nature_first_data()
}

/// A referência JUNO (Nature 654, 2026).
pub fn juno_reference() -> &'static str {
    JUNO_NATURE_REF
}

/// sin²θ₁₂ diretamente do valor publicado.
pub fn juno_sin2_theta12() -> f64 {
    JUNO_SIN2_THETA12
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn juno_nature_constants() {
        let j = juno_spectrum_measurement();
        assert_eq!(j.sin2_theta12, JUNO_SIN2_THETA12);
        assert_eq!(j.delta_m21_sq_ev2, JUNO_DELTA_M21_SQ_EV2);
        assert_eq!(j.baseline_km, 52.5);
        assert!(j.normal_mass_ordering);
    }

    #[test]
    fn amplitude_matches_sin2_2theta() {
        let j = juno_spectrum_measurement();
        let (amp, err) = j.sin2_2theta12();
        // sin²2θ₁₂ ≈ 0.8544 ± 0.013 (±1σ).
        assert!((amp - 0.8544).abs() < 0.001);
        assert!(err > 0.0 && err < 0.02);
    }

    #[test]
    fn measurement_self_consistent() {
        let j = juno_spectrum_measurement();
        assert!(j.consistent_with_before(JUNO_SIN2_THETA12, JUNO_DELTA_M21_SQ_EV2));
        assert!(!j.consistent_with_before(0.5, JUNO_DELTA_M21_SQ_EV2));
    }

    #[test]
    fn reference_provenance() {
        let r = juno_reference();
        assert!(r.contains("Nature 654"));
        assert!(r.contains("10.1038/s41586-026-10538-z"));
    }
}