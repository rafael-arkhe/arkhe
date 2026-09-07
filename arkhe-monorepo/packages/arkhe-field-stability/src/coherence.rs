//! Coeficiente de coerência Φ — definição formal ancorada no Gap-1.
//!
//! ```text
//! Φ(x ; W) = 1 − sqrt( Σ wᵢ·(1 − xᵢ)² )
//! ```
//!
//! * Domínio: `xᵢ ∈ [0,1]`, pesos `wᵢ ∈ [0,1]` com `Σ wᵢ = 1`.
//! * Faixa constitucional (Gap-1): `0.577350 < Φ ≤ 0.999900`.
//! * Vetores de conformidade (docs/coherence_metric.md §6):
//!   V1 (e1 real) `Φ = 0.9646`, V2 (degradado) `Φ = 0.6983`,
//!   V3 (piso) `Φ = 0.5000`.

use crate::constants::{WEIGHT_LATENCY, WEIGHT_STABILITY, WEIGHT_SUCCESS_RATE};
use crate::field_stability::FieldStability;

/// Vetor de pesos canônico `(0.4, 0.4, 0.2)` — alinhado ao [`crate::constants`].
pub const WEIGHTS_DEFAULT: [f64; 3] = [WEIGHT_STABILITY, WEIGHT_SUCCESS_RATE, WEIGHT_LATENCY];

/// Calcula `Φ = 1 − sqrt(Σ wᵢ·(1 − xᵢ)²)`.
///
/// # Argumentos
///
/// * `components` — vetor de conformidade `(Ω, Σ, Λ)`, cada componente ∈ [0,1].
/// * `weights` — pesos `(w_Ω, w_Σ, w_Λ)` somando 1 (como [`WEIGHTS_DEFAULT`]).
#[must_use]
pub fn phi(components: [f64; 3], weights: [f64; 3]) -> f64 {
    let mut sum = 0.0;
    for (x, w) in components.iter().zip(weights.iter()) {
        sum += w * (1.0 - x).powi(2);
    }
    1.0 - sum.sqrt()
}

/// Calcula `Φ` com os pesos canônicos `(0.4, 0.4, 0.2)`.
#[must_use]
pub fn phi_default(components: [f64; 3]) -> f64 {
    phi(components, WEIGHTS_DEFAULT)
}

/// Calcula `Φ` diretamente de uma instância de [`FieldStability`].
#[must_use]
pub fn phi_from_field_stability(fs: &FieldStability) -> f64 {
    phi_default([fs.stability, fs.success_rate, fs.latency_score])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phi_v1_e1_real() {
        // V1: Ω=0.9517, Σ=1.0, Λ=0.96 → Φ = 0.9646
        let phi = phi_default([0.9517, 1.0, 0.96]);
        assert!((phi - 0.9646).abs() < 1e-4, "Φ V1 = {phi}");
    }

    #[test]
    fn test_phi_v2_degraded() {
        // V2: Ω=0.80, Σ=0.75, Λ=0.50 → Φ = 0.6983
        let phi = phi_default([0.80, 0.75, 0.50]);
        assert!((phi - 0.6983).abs() < 1e-4, "Φ V2 = {phi}");
    }

    #[test]
    fn test_phi_v3_floor() {
        // V3: Ω=Σ=Λ=0.50 → Φ = 1 − sqrt(0.25) = 0.5 (abaixo de 1/√3 → REJEITADO)
        let phi = phi_default([0.50, 0.50, 0.50]);
        assert!((phi - 0.5).abs() < 1e-12, "Φ V3 = {phi}");
    }

    #[test]
    fn test_phi_perfect_unattainable_ideal() {
        // (1,1,1) → Φ = 1 (ideal assintótico; incertificável pela banda 0.999900)
        let phi = phi_default([1.0, 1.0, 1.0]);
        assert!((phi - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_phi_null_coherence() {
        // (0,0,0) → Φ = 1 − sqrt(1.0) = 0 (coerência nula)
        let phi = phi_default([0.0, 0.0, 0.0]);
        assert!(phi.abs() < 1e-12);
    }

    #[test]
    fn test_phi_le_overall_in_domain() {
        // Teorema (Cauchy–Schwarz com dᵢ=1−xᵢ): Φ ≤ overall em todo o domínio.
        // (Σw·d)² ≤ Σw·d²  ⇒  sqrt(Σw·d²) ≥ 1 − overall  ⇒  Φ ≤ overall.
        // Igualdade sse Ω = Σ = Λ. Contraexemplos: V1 (0.9646 < 0.9727),
        // (1,1,0) → overall 0.80 ⊢ Φ ≈ 0.553 < 1/√3 (portões independentes).
        let cases = [
            [0.9517, 1.0, 0.96],
            [0.8, 0.75, 0.5],
            [1.0, 1.0, 0.0],
            [0.5, 0.5, 0.5],
            [0.333, 0.666, 0.999],
        ];
        for c in cases {
            let phi = phi_default(c);
            let overall =
                WEIGHT_STABILITY * c[0] + WEIGHT_SUCCESS_RATE * c[1] + WEIGHT_LATENCY * c[2];
            assert!(phi <= overall + 1e-12, "Φ={phi} > overall={overall}");
        }
    }

    #[test]
    fn test_phi_monotone_in_each_component() {
        // Propriedade P2: Φ é não-decrescente em cada componente isolado.
        let base = [0.8, 0.75, 0.5];
        let base_phi = phi_default(base);
        for i in 0..3 {
            let mut up = base;
            up[i] = (up[i] + 0.1).min(1.0);
            let up_phi = phi_default(up);
            let mut down = base;
            down[i] = (down[i] - 0.1).max(0.0);
            let down_phi = phi_default(down);
            assert!(up_phi >= base_phi - 1e-12, "componente {i} subiu mas Φ caiu");
            assert!(down_phi <= base_phi + 1e-12, "componente {i} caiu mas Φ subiu");
        }
    }

    #[test]
    fn test_phi_bounded_in_unit_interval() {
        let grid = [0.0, 0.25, 0.5, 0.75, 1.0];
        for &a in &grid {
            for &b in &grid {
                for &c in &grid {
                    let phi = phi_default([a, b, c]);
                    assert!(
                        (0.0..=1.0).contains(&phi),
                        "Φ={phi} fora de [0,1] para ({a},{b},{c})"
                    );
                }
            }
        }
    }

    #[test]
    fn test_phi_from_field_stability() {
        let fs = FieldStability {
            name: "V1_e1".to_string(),
            stability: 0.9517,
            success_rate: 1.0,
            latency_score: 0.96,
        };
        let phi = phi_from_field_stability(&fs);
        assert!((phi - 0.9646).abs() < 1e-4);
    }

    #[test]
    fn test_gap1_faixa_constitucional() {
        let inferior = 1.0 / 3.0_f64.sqrt(); // 0.5773502691…
        let superior = 0.999900;
        let v1 = phi_default([0.9517, 1.0, 0.96]);
        let v2 = phi_default([0.80, 0.75, 0.50]);
        let v3 = phi_default([0.50, 0.50, 0.50]);
        assert!(v1 > inferior && v1 <= superior);
        assert!(v2 > inferior && v2 <= superior);
        assert!(v3 <= inferior, "V3 acima do piso constitucional");
    }
}