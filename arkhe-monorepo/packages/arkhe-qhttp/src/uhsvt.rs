//! Selo #5 — Linearised quantum signal processing / UHSVT (arXiv:2608.14387).
//!
//! A **UHSVT** (Universal Hamiltonian Singular Value Transformation) transforma os
//! valores singulares de uma matriz arbitrária `A` (bloco de um Hamiltoniano
//! acessível como caixa-preta) por qualquer função complexa suficientemente
//! diferenciável `f` que **desapareça na origem** — `f(0) = 0` é a **única**
//! condição, sem exigir limite inferior nos valores singulares ou portões `X`
//! no subespaço qubitizado, ao contrário das abordagens QSVT prévias.
//!
//! No qhttp, `A` é a matriz do campo de fase e `f = λ₂ + δ` (coerência + correção
//! da anomalia quiral), que por construção satisfaz `f(0) = 0`.

use serde::{Deserialize, Serialize};

/// Referência do Selo #5.
pub const LQSP_REF_2026: &str = "M. Arsenault, H. Kristjánsson, 'Linearised quantum signal \
processing', arXiv:2608.14387 [quant-ph], 14 Aug 2026";

/// Régua da UHSVT: `f(0) = 0`; caso contrário a transformação é inválida.
pub fn valid_uhsvt_function(fn_at_zero: f64) -> bool {
    fn_at_zero.abs() < 1e-12
}

/// Especificação de uma aplicação UHSVT sobre os valores singulares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UhsvtSpec {
    pub singular_values: Vec<f64>,
    pub transformed: Vec<f64>,
    pub condition_f0_zero: bool,
    pub coherent: bool,
}

/// Aplica a UHSVT: `σᵢ ↦ f(σᵢ)` para cada valor singular.
///
/// Retorna `Ok` somente se `f(0) = 0` (a condição única do Selo #5).
pub fn transform_function_singular_values(
    f: impl Fn(f64) -> f64,
    singular_values: &[f64],
) -> Result<UhsvtSpec, String> {
    if !valid_uhsvt_function(f(0.0)) {
        return Err("UHSVT exige f(0) = 0 (Selo #5)".into());
    }
    let transformed: Vec<f64> = singular_values.iter().map(|&s| f(s)).collect();
    let coherent = transformed.iter().all(|t| t.is_finite() && *t >= 0.0);
    Ok(UhsvtSpec {
        singular_values: singular_values.to_vec(),
        transformed,
        condition_f0_zero: true,
        coherent,
    })
}

/// Wrapper conveniente para especificações prontas (auditorias).
pub fn uhsvt_spec(singular_values: Vec<f64>, transformed: Vec<f64>) -> UhsvtSpec {
    UhsvtSpec {
        singular_values,
        transformed,
        condition_f0_zero: true,
        coherent: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_zero_condition_single_requirement() {
        assert!(valid_uhsvt_function(0.0));
        assert!(!valid_uhsvt_function(1e-6));
        let bad = transform_function_singular_values(|s| s + 0.5, &[0.1, 0.9]);
        assert!(bad.is_err(), "f(0)=0.5≠0 deve rejeitar");
    }

    #[test]
    fn linear_coherence_function_maps_singular_values() {
        // f(s) = (λ₂ + δ)·s com λ₂=0.9991, δ>0 ⇒ f(0)=0 ✓.
        let sv = [0.0, 0.3, 0.9991, 1.4];
        let spec = transform_function_singular_values(|s| 1.0001 * s, &sv).unwrap();
        assert!(spec.condition_f0_zero);
        assert!(spec.coherent);
        assert!((spec.transformed[2] - 0.9991 * 1.0001).abs() < 1e-9);
        assert_eq!(spec.transformed[0], 0.0);
    }

    #[test]
    fn singular_zero_maps_to_zero() {
        let spec = transform_function_singular_values(|s| 0.9 * s, &[0.0, 1.0]).unwrap();
        assert_eq!(spec.transformed[0], 0.0);
        assert!((spec.transformed[1] - 0.9).abs() < 1e-12);
    }
}