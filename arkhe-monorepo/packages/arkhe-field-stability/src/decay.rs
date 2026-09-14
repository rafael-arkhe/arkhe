//! Invariante I530 — decaimento de coerência limitado à banda [0,1].
//!
//! Modelo (bloco 1009): para a coerência atual `c` e a taxa de retenção `k`,
//! ambas em [0,1], o valor decaído `c·k` permanece em [0,1]. Em representação
//! fixa ×10⁴ (escala inteira dos núcleos Lean de bloco 966), o produto
//! `(C·K) / 10_000` é a quantização do mesmo modelo.
//!
//! Prova formal em core Lean (v4.33.1, sem Mathlib, sem `sorry` — convenção
//! bloco 966) em `src/lean/I530_CoherenceDecay.lean`:
//!   * [`crate::field_stability`] refs — núcleo `ArkheCoherenceDecay`
//!   * `I530A..I530G` e `I530_decay_bounded`: banda [0,1] e preservação do
//!     teto Gap-1 (0.9999), com margem estrita no pior caso (9998 < 9999).

/// Escala fixa ×10⁴ da quantização inteira do núcleo Lean I530 (1.0).
pub const SCALE_X1E4: u64 = 10_000;

/// Teto inclusivo Gap-1 em escala ×10⁴ (0.9999).
pub const CEILING_X1E4: u64 = 9_999;

/// Valor decaído em escala ×10⁴: `floor(C·K / 10⁴)`.
///
/// É a mesma definição `decayed` do núcleo de Lean `ArkheCoherenceDecay`
/// (`src/lean/I530_CoherenceDecay.lean`); para `C,K ≤ 10_000` o resultado fica
/// ≤ 10_000 (I530-C) e para `C,K ≤ 9_999` fica ≤ 9_999 — estritamente 9_998 no
/// pior caso (I530-D/I530-G).
#[must_use]
#[inline]
pub const fn decay_scaled(coherence_scaled: u64, retention: u64) -> u64 {
    (coherence_scaled * retention) / SCALE_X1E4
}

/// Decaimento de coerência em f64: `c·k`, com `c`,`k` em [0,1].
///
/// O resultado é a realização contínua (f64) do modelo inteiro provado em
/// I530. Em build debug, os `debug_assert` verificam as premissas da prova
/// (banda [0,1] dos operandos e do resultado); em build release são no-ops —
/// a garantia formal vive no núcleo Lean, não na aritmética flutuante.
///
/// # Panics
///
/// Em build debug, se `c` ou `k` sair da banda [0,1], ou se `c·k` não ficar
/// dentro da banda (violação da invariante, matematicamente impossível para
/// operandos in-band em aritmética exata).
#[must_use]
#[inline]
pub fn decayed(c: f64, k: f64) -> f64 {
    debug_assert!(c.is_finite(), "I530: coerência atual deve ser finita");
    debug_assert!(k.is_finite(), "I530: retenção deve ser finita");
    debug_assert!((0.0..=1.0).contains(&c), "I530: c fora da banda [0,1]");
    debug_assert!((0.0..=1.0).contains(&k), "I530: k fora da banda [0,1]");
    let next = c * k;
    debug_assert!(
        (0.0..=1.0).contains(&next),
        "I530: c·k saiu da banda [0,1]"
    );
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_closure_within_unit() {
        for c in [0, 1, 5774, 8000, SCALE_X1E4] {
            for k in [0, 1, 5000, 9999, SCALE_X1E4] {
                assert!(decay_scaled(c, k) <= SCALE_X1E4, "I530-B/C: {c}·{k}");
            }
        }
    }

    #[test]
    fn scaled_band_preserved_below_ceiling() {
        for c in [5774, 7000, CEILING_X1E4] {
            for k in [5774, 9000, CEILING_X1E4] {
                assert!(
                    decay_scaled(c, k) <= CEILING_X1E4,
                    "I530-D: {c}·{k}"
                );
            }
        }
    }

    #[test]
    fn scaled_worst_case_strict_below_ceiling() {
        assert_eq!(decay_scaled(CEILING_X1E4, CEILING_X1E4), 9_998, "I530-G");
        assert!(decay_scaled(CEILING_X1E4, CEILING_X1E4) < CEILING_X1E4, "I530-G");
    }

    #[test]
    fn scaled_monotone_in_coherence() {
        let k = 8000;
        assert!(decay_scaled(7000, k) <= decay_scaled(9000, k), "I530-E");
    }

    #[test]
    fn scaled_monotone_in_retention() {
        let c = 8000;
        assert!(decay_scaled(c, 3000) <= decay_scaled(c, 9000), "I530-F");
    }

    #[test]
    fn f64_stays_in_band() {
        for &c in &[0.0, 0.5774, 0.7, 0.9999, 1.0] {
            for &k in &[0.0, 0.5, 0.9999, 1.0] {
                let next = decayed(c, k);
                assert!((0.0..=1.0).contains(&next), "I530: {c}·{k}");
            }
        }
    }

    #[test]
    fn f64_ceiling_product_strict_below_teto() {
        assert!(
            decayed(0.9999, 0.9999) < 0.9999,
            "I530-D f64: pior caso estritamente abaixo do teto"
        );
    }
}