//! Selo #2 — Robust Quantum Extremal Numbers (arXiv:2608.13907).
//!
//! Estados absolutamente maximamente emaranhados exigem que toda redução com no
//! máximo metade das partes seja maximamente misturada — condição rígida e, para
//! qubits, frequentemente impossível. A extensão robusta maximiza o número de
//! marginals de corpo-k **exatamente** maximamente misturados dentro do defeito
//! `ε`. O platô exato `Q_ex,ε^D(8,4)=56` vale para `0 ≤ ε < 1/5`; para 9 qubits,
//! `Q_ex,ε^D(9,4) ≤ 120` para `0 ≤ ε < 1/17`. A estabilidade local moderna segue
//! da desigualdade `Σ_{i∈T} D_{T∖{i}} ≥ 1` com `|T| = 2m+1` em `4m` qubits.

use serde::{Deserialize, Serialize};

/// Referência do Selo #2 (proveniência primária).
pub const EXTREMAL_8_QU4_REF: &str =
    "W. Zhang, Z. Han, X. Zhang, 'Robust Quantum Extremal Numbers', arXiv:2608.13907 [quant-ph], 14 Aug 2026";
/// Referência do Selo #2 para o limite de 9 qubits.
pub const EXTREMAL_9_QU4_REF: &str =
    "W. Zhang, Z. Han, X. Zhang, 'Robust Quantum Extremal Numbers', arXiv:2608.13907 [quant-ph] §5 (9 qubits)";

/// Platô exato do número extremal quântico robusto para 8 qubits, corpo 4.
pub const QEX_8_4_PLATEAU: usize = 56;
/// Limite superior para 9 qubits, corpo 4.
pub const QEX_9_4_UPPER_BOUND: usize = 120;
/// Limiar de estabilidade para 8 qubits: `ε < 1/5` (20%).
pub const STABILITY_EPS_8_QU4: f64 = 1.0 / 5.0;
/// Limiar de estabilidade para 9 qubits: `ε < 1/17` (≈ 5.9%).
pub const STABILITY_EPS_9_QU4: f64 = 1.0 / 17.0;

/// Defeito de maximal-mixing: `D_A = 2^|A|·Tr(ρ_A²) − 1`.
///
/// `purity = Tr(ρ_A²)` está em `[2^-|A|, 1]`; o defeito resultante varia em
/// `[0, 2^|A| − 1]`, com `D_A = 0` precisamente quando `ρ_A = I_A / 2^|A|`.
pub fn defect_maximal_mixing(purity: f64, subsystem_qubits: usize) -> f64 {
    let dim = 2u64.pow(subsystem_qubits as u32) as f64;
    (dim * purity - 1.0).max(0.0)
}

/// Desigualdade de estabilidade local: `Σ_{i∈T} D_{T∖{i}} ≥ 1`, `|T| = 2m+1`.
///
/// `defects` deve conter os defeitos dos subsistemas `T ∖ {i}` (tamanho `2m`),
/// na ordem de `i ∈ T`. Se `sum ≥ 1`, nenhum hipergrafo de `ε`-bons `2m`-subsets
/// com `ε < 1/(2m+1)` é `K_{2m+1}^{(2m)}`-livre na forma violada — platô garantido.
pub fn local_stability_holds(defects: &[f64], m: usize) -> bool {
    let n_req = 2 * m + 1;
    defects.len() == n_req && defects.iter().sum::<f64>() >= 1.0
}

/// Tipo de vínculo extremal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindKind {
    PlateauExact,
    UpperBound,
}

/// Vínculo extremal quantitativo com o platô/limite e o respectivo limiar `ε`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ExtremalBinding {
    pub n_qubits: usize,
    pub body: usize,
    pub kind: BindKind,
    pub best_q: usize,
    pub epsilon: f64,
    pub threshold: f64,
    pub stable: bool,
}

/// `Q_ex,ε^D(8,4) = 56` para `0 ≤ ε < 1/5`.
pub fn extremal_8_4(epsilon: f64) -> ExtremalBinding {
    ExtremalBinding {
        n_qubits: 8,
        body: 4,
        kind: BindKind::PlateauExact,
        best_q: QEX_8_4_PLATEAU,
        epsilon,
        threshold: STABILITY_EPS_8_QU4,
        stable: epsilon < STABILITY_EPS_8_QU4,
    }
}

/// `Q_ex,ε^D(9,4) ≤ 120` para `0 ≤ ε < 1/17`.
pub fn extremal_9_4(epsilon: f64) -> ExtremalBinding {
    ExtremalBinding {
        n_qubits: 9,
        body: 4,
        kind: BindKind::UpperBound,
        best_q: QEX_9_4_UPPER_BOUND,
        epsilon,
        threshold: STABILITY_EPS_9_QU4,
        stable: epsilon < STABILITY_EPS_9_QU4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plateau_exact_for_eight_qubits() {
        let b = extremal_8_4(0.1);
        assert_eq!(b.best_q, 56);
        assert_eq!(b.kind, BindKind::PlateauExact);
        assert!(b.stable);
        let b2 = extremal_8_4(0.25);
        assert!(!b2.stable, "ε ≥ 1/5 quebra o platô");
    }

    #[test]
    fn nine_qubit_upper_bound() {
        let b = extremal_9_4(0.05);
        assert_eq!(b.best_q, 120);
        assert_eq!(b.kind, BindKind::UpperBound);
        assert!(b.stable);
        assert!((b.threshold - (1.0 / 17.0)).abs() < 1e-12);
        let b2 = extremal_9_4(0.06);
        assert!(!b2.stable, "ε ≥ 1/17 viola o limiar de 9 qubits");
    }

    #[test]
    fn max_mixing_defect_is_zero_for_maximally_mixed() {
        // ρ_A = I/16 ⇒ Tr(ρ²) = 1/16; D = 16·(1/16) − 1 = 0.
        let d = defect_maximal_mixing(1.0 / 16.0, 4);
        assert!(d.abs() < 1e-12);
    }

    #[test]
    fn defect_saturates_for_pure_subsystem() {
        // Subsistema puro: Tr(ρ²) = 1 ⇒ D = 2^k − 1 (2⁴−1 = 15).
        let d = defect_maximal_mixing(1.0, 4);
        assert!((d - 15.0).abs() < 1e-12);
    }

    #[test]
    fn local_stability_inequality() {
        // m = 2 ⇒ |T| = 5; defeitos dos 5 subsistemas 4-corpos somam ≥ 1.
        let defects = [0.22, 0.22, 0.22, 0.22, 0.22];
        assert!(local_stability_holds(&defects, 2));
        let weak = [0.1, 0.1, 0.1, 0.1, 0.1];
        assert!(!local_stability_holds(&weak, 2));
        assert!(!local_stability_holds(&defects, 3), "|T| = 2m+1 exigido");
    }
}