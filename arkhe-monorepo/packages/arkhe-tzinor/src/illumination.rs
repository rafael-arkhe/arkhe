//! Selo #1 — Iluminação quântica com fase desconhecida (arXiv:2608.13997).
//!
//! A vantagem da iluminação quântica é de ~6 dB (fator ~4) no expoente de erro
//! em relação ao estado coerente de mesma energia — **desde que a fase de retorno
//! seja conhecida**. Em baixa refletividade, com fase desconhecida (parâmetro
//! comum constante), o artigo prova que **todo** receptor i.i.d. e **todo** estado
//! de uma única variável de sinal com idler de qualquer dimensão tem expoente de
//! erro limitado pelo pior caso; luz coerente com **detecção heteródina** satura
//! esse limite para **todo** valor da fase. O emaranhamento, então, não oferece
//! vantagem quântica no pior caso — a ordem principal em refletividade.

use serde::{Deserialize, Serialize};

/// Referência do Selo #1.
pub const ILLUM_REF_2026: &str = "K. Shiraiwa, S. Kukita, 'Limits of independent and identical \
measurements for quantum illumination with an unknown return phase', arXiv:2608.13997 \
[quant-ph], 14 Aug 2026";
/// Vantagem quântica com fase conhecida: ~6 dB.
pub const ENTANGLEMENT_GAIN_DB_KNOWN_PHASE: f64 = 6.0;
/// Vantagem (fator) com fase conhecida: ~4×.
pub const HETERODYNE_WORSECASE: f64 = 1.0;
/// Fator de ganho com fase conhecida.
pub fn known_phase_factor() -> f64 {
    4.0
}

/// Ganho de expoente de erro em dB, por estratégia e conhecimento de fase.
///
/// * fase conhecida + emaranhamento ⇒ ~6 dB;
/// * fase desconhecida (qualquer receptor i.i.d.) ⇒ 0 dB no pior caso (Selo #1).
pub fn error_exponent_gain_db(phase_known: bool, entangled: bool) -> f64 {
    if phase_known && entangled {
        ENTANGLEMENT_GAIN_DB_KNOWN_PHASE
    } else {
        0.0
    }
}

/// Estratégia de detecção do canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetectionStrategy {
    EntangledKnownPhase,
    HeterodyneUnknownPhase,
}

/// A heteródina satura o limite do pior caso? (Selo #1: sim, para todo fase).
pub fn heterodyne_saturates_bound() -> bool {
    true
}

/// Escolhe a estratégia ótima dado o conhecimento de fase (protocolo Tzinor).
pub fn choose_strategy(phase_known: bool) -> DetectionStrategy {
    if phase_known {
        DetectionStrategy::EntangledKnownPhase
    } else {
        DetectionStrategy::HeterodyneUnknownPhase
    }
}

/// Avaliação de iluminação para auditoria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IlluminationAssessment {
    pub phase_known: bool,
    pub strategy: DetectionStrategy,
    pub gain_db: f64,
    pub worst_case_saturated: bool,
}

/// Avalia o canal segundo o Selo #1.
pub fn assess_channel(phase_known: bool) -> IlluminationAssessment {
    let strategy = choose_strategy(phase_known);
    let entangled = matches!(strategy, DetectionStrategy::EntangledKnownPhase);
    IlluminationAssessment {
        phase_known,
        strategy,
        gain_db: error_exponent_gain_db(phase_known, entangled),
        worst_case_saturated: heterodyne_saturates_bound(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_phase_keeps_six_db() {
        assert!((known_phase_factor() - 4.0).abs() < 1e-12);
        assert!((error_exponent_gain_db(true, true) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn unknown_phase_worst_case_no_advantage() {
        assert_eq!(error_exponent_gain_db(false, true), 0.0);
        assert_eq!(error_exponent_gain_db(false, false), 0.0);
        assert!(heterodyne_saturates_bound());
    }

    #[test]
    fn strategy_selection_follows_phase() {
        assert_eq!(choose_strategy(true), DetectionStrategy::EntangledKnownPhase);
        assert_eq!(choose_strategy(false), DetectionStrategy::HeterodyneUnknownPhase);
    }

    #[test]
    fn assessment_consistent() {
        let a = assess_channel(false);
        assert_eq!(a.strategy, DetectionStrategy::HeterodyneUnknownPhase);
        assert_eq!(a.gain_db, 0.0);
        assert!(a.worst_case_saturated);
    }
}