//! Coerência do fator constitucional Φ_C (Loopseal/Gap).
//!
//! Módulo #7: o fator Φ_C captura quão fortemente os invariantes constitucionais
//! avaliam como coerentemente/estavelmente. O limite rígido da Constituição é
//! `0.577350 < Φ_C ≤ 0.999900` (Gap-1).

use serde::{Deserialize, Serialize};

/// Resultado da análise de coerência de um conjunto de invariantes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhiCoherence {
    /// Avaliações individuais dos invariantes peso-vetor.
    pub invariants: Vec<f64>,
    /// Fator de coerência agregado Φ_C ∈ (0, 1].
    pub phi_c: f64,
    /// True se `0.577350 < Φ_C ≤ 0.999900`.
    pub within_bounds: bool,
}

impl PhiCoherence {
    /// Calcula Φ_C a partir das avaliações individuais (0.0 = falha total, 1.0 = perfeito).
    ///
    /// A coerência é uma função quadrática do quadrado médio das avaliações que:
    /// - cruza exatamente o limiar constitucional 1/√3 ≈ 0.577350 quando todas as
    ///   avaliações estão no ponto giratório Θ = 1/√3 (Gap-1);
    /// - satura em 0.9999 quando todas as avaliações são perfeitas.
    pub fn analyze(invariants: &[f64]) -> Self {
        assert!(!invariants.is_empty(), "pelo menos um invariante é obrigatório");
        let mean_sq = invariants.iter().map(|v| v * v).sum::<f64>() / invariants.len() as f64;
        // Ajuste quadrático sobre Θ² = 1/3 (→ 1/√3) e 1 (→ 0.9999).
        // f(x) = A·x − B·x² com f(1)=0.9999 e f(1/3)=1/√3.
        const T: f64 = 0.577_350_269_189_625_8; // 1/√3
        const A: f64 = 4.5 * (T - 0.9999 / 9.0); // (9T − 0.9999)/2
        const B: f64 = A - 0.9999;
        let phi_c = (A * mean_sq - B * mean_sq * mean_sq).clamp(0.0, 0.9999);
        let within_bounds = phi_c > 0.577350 && phi_c <= 0.999900;
        Self {
            invariants: invariants.to_vec(),
            phi_c,
            within_bounds,
        }
    }

    /// Conveniência: falha constitucional explícita.
    pub fn failed() -> Self {
        Self {
            invariants: Vec::new(),
            phi_c: 0.0,
            within_bounds: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_invariants_yield_in_bounds() {
        let p = PhiCoherence::analyze(&[1.0, 1.0, 1.0]);
        assert!(p.within_bounds, "Φ_C={} deve estar nos limites", p.phi_c);
        assert!(p.phi_c > 0.577350 && p.phi_c <= 0.999900);
    }

    #[test]
    fn failing_invariants_out_of_bounds() {
        let p = PhiCoherence::analyze(&[0.0, 0.0, 0.0]);
        assert!(!p.within_bounds);
    }

    #[test]
    fn threshold_boundary_holds() {
        // Θ = 1/√3 ≈ 0.577350: coerência mínima constitucional.
        let th = 1.0 / 3.0_f64.sqrt();
        let p = PhiCoherence::analyze(&[th, th, th]);
        assert!(p.phi_c > 0.577350, "Φ_C={} deve exceder o limiar Giannasi", p.phi_c);
    }
}