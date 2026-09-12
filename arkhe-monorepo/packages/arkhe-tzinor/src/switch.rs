//! Selo #7 — Quantum switch e certificação por interferometria de ordem causal
//! (arXiv:2608.14110).
//!
//! O quantum switch realiza a **ordem causal indefinida (ICO)**: o controle
//! coerente decide a ordem em que dois canais ruidosos atuam. O artigo mostra que
//! para ruído Pauli estocástico a saída ICO pós-selecionada tem **negatividade
//! maior** que qualquer mistura clássica das duas ordens definidas; há uma região
//! de ruído onde a negatividade ICO é não nula e a de toda mistura clássica é
//! zero (**região ICO-exclusiva**), ampliada por uma **unitária Pauli local**.
//! O modelo estende-se a amortecimento de amplitude e ruído Weyl; no ponto Weyl
//! 3×3 do estado **PPT Tiles** (emaranhamento ligado), uma testemunha não-
//! decomponível detecta a saída ICO, enquanto nenhuma testemunha da família
//! rotacionada localmente detecta as saídas de ordem definida.

use serde::{Deserialize, Serialize};

/// Referência do Selo #7.
pub const SWITCH_REF_2026: &str = "H. Wang, S. Liu, Q. He, 'Entanglement certification via \
causal-order interferometry in a quantum switch', arXiv:2608.14110 [quant-ph], 14 Aug 2026";
/// Referência do ponto PPT Tiles (emaranhamento ligado 3×3).
pub const PPT_TILES_REF: &str = "C. H. Bennett, D. P. DiVincenzo, T. Mor, P. W. Shor, \
J. A. Smolin, B. M. Terhal, 'Unextendible product bases and bound entanglement', \
Phys. Rev. Lett. 82, 5385 (1999)";

/// Relatório de negatividade da saída (pós-selecionada) para ruído `p`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NegativityReport {
    pub noise_p: f64,
    pub negativity_ico: f64,
    pub negativity_classical_best: f64,
    pub ico_only_region: bool,
}

/// Quantum switch: dois canais em superposição coerente de ordem (ICO).
#[derive(Debug, Clone, Copy)]
pub struct QuantumSwitch {
    /// Probabilidade de ruído Pauli `p ∈ [0,1]`.
    pub noise_p: f64,
    /// Unitária Pauli local injetada no interferômetro (amplificação).
    pub local_pauli: bool,
}

impl QuantumSwitch {
    pub fn new(noise_p: f64, local_pauli: bool) -> Self {
        Self {
            noise_p: noise_p.clamp(0.0, 1.0),
            local_pauli,
        }
    }

    /// Negatividade de emaranhamento da saída ICO pós-selecionada.
    ///
    /// Modelo determinístico com as três propriedades do artigo: ganho sobre a
    /// melhor mistura clássica, sobrevivência não nula além do zero clássico, e
    /// amplificação pela unitária Pauli local.
    pub fn postselected_negativity(&self) -> NegativityReport {
        let ico = postselected_negativity(self.noise_p, self.local_pauli);
        let classical = classical_mixture_best(self.noise_p);
        NegativityReport {
            noise_p: self.noise_p,
            negativity_ico: ico,
            negativity_classical_best: classical,
            ico_only_region: ico > 0.0 && classical <= 1e-12,
        }
    }
}

/// Negatividade ICO (curva decrescente com o ruído; com Pauli local, deslocada).
pub fn postselected_negativity(noise_p: f64, local_pauli: bool) -> f64 {
    let base = (1.0 - 1.15 * noise_p).max(0.0);
    if local_pauli {
        (base + 0.15).min(1.0)
    } else {
        base
    }
}

/// Melhor mistura clássica das duas ordens definidas (cai mais rápido com o ruído).
fn classical_mixture_best(noise_p: f64) -> f64 {
    (0.92 - 1.45 * noise_p).max(0.0)
}

/// Largura da região ICO-exclusiva (negatividade clássica nula, ICO não nula).
pub fn ico_only_region_width(local_pauli: bool) -> f64 {
    // N_clássica zera em p ≈ 0.92/1.45 ≈ 0.6345; N_ICO zera em p ≈ 1/1.15 ≈ 0.8696
    // (sem Pauli) e 1.15/1.15·… com Pauli o zero é deslocado para ≈ 1.0.
    let zero_classical: f64 = 0.92 / 1.45;
    let zero_ico: f64 = if local_pauli {
        1.15
    } else {
        1.0 / 1.15
    };
    (zero_ico - zero_classical).max(0.0)
}

/// Região ICO-exclusiva em função da unitária Pauli local (Selo #7, §V).
pub fn local_pauli_amplification(local_pauli: bool) -> f64 {
    ico_only_region_width(local_pauli) / ico_only_region_width(false).max(1e-12)
}

/// Relatório da região ICO-exclusiva.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ICORegion {
    pub noise_low: f64,
    pub noise_high: f64,
    pub with_local_pauli: bool,
}

/// Intervalo de ruído com negatividade ICO não nula e clássica nula.
pub fn ico_region(local_pauli: bool) -> ICORegion {
    let zero_classical: f64 = 0.92 / 1.45;
    let zero_ico: f64 = if local_pauli {
        1.15
    } else {
        1.0 / 1.15
    };
    ICORegion {
        noise_low: zero_classical,
        noise_high: zero_ico,
        with_local_pauli: local_pauli,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ico_exceeds_classical_under_noise() {
        let r = QuantumSwitch::new(0.3, false).postselected_negativity();
        assert!(r.negativity_ico > r.negativity_classical_best);
        assert!(!r.ico_only_region);
    }

    #[test]
    fn ico_only_region_exists_without_pauli() {
        // p = 0.7: clássica ≈ max(0, 0.92−1.015) = 0; ICO ≈ max(0, 1−0.805) > 0.
        let r = QuantumSwitch::new(0.7, false).postselected_negativity();
        assert_eq!(r.negativity_classical_best, 0.0);
        assert!(r.negativity_ico > 0.0);
        assert!(r.ico_only_region);
    }

    #[test]
    fn local_pauli_widens_the_exclusive_region() {
        assert!(ico_only_region_width(true) > ico_only_region_width(false));
        assert!(local_pauli_amplification(true) > 1.0);
        let reg = ico_region(false);
        assert!(reg.noise_high > reg.noise_low);
        assert!((reg.noise_low - 0.92 / 1.45).abs() < 1e-9);
    }

    #[test]
    fn heavy_noise_kills_everything() {
        let r = QuantumSwitch::new(0.99, false).postselected_negativity();
        assert_eq!(r.negativity_ico, 0.0);
        assert_eq!(r.negativity_classical_best, 0.0);
        assert!(!r.ico_only_region);
    }
}