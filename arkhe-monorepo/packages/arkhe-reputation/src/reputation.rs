//! Reputação de nós TOON operando o transporte (módulo #4).
//!
//! Modelo: reputação exponencialmente decaída no tempo somada a indicadores de
//! qualidade de serviço (φ_C reportado, uptime, confiabilidade de entrega),
//! sempre dentro do orçamento de entropia (Gap-2) e minimizando dados (Ethics-2).

use serde::{Deserialize, Serialize};

/// Meia-vida de relevância de um evento de reputação (dias).
pub const TOON_DECAY_DAYS: f64 = 30.0;

/// Resultado agregado de reputação de um nó.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationScore {
    pub id: String,
    /// Uptime reportado [0,1].
    pub uptime: f64,
    /// Confiabilidade de entrega [0,1].
    pub reliability: f64,
    /// Fator de coerência reportado Φ_C ∈ (0,1].
    pub phi_c: f64,
    /// Pontos de base acumulados (integridade).
    pub integrity_points: f64,
    /// Idade (dias) desde o primeiro selo.
    pub age_days: f64,
    /// Reputação final estendida [0,∞).
    pub score: f64,
    /// Escala Logística [0,100].
    pub normalized: f64,
}

impl ReputationScore {
    /// Avalia a reputação com decaimento exponencial sobre a idade.
    pub fn evaluate(
        id: &str,
        uptime: f64,
        reliability: f64,
        phi_c: f64,
        integrity_points: f64,
        age_days: f64,
    ) -> Self {
        let decay = (-age_days / TOON_DECAY_DAYS).exp();
        let base = 0.6 * uptime + 0.3 * reliability + 0.1 * phi_c.clamp(0.0, 1.0);
        let score = (base * (1.0 + integrity_points) * decay).max(0.0);
        // Escala logística normalizada para [0,100].
        let normalized = 100.0 / (1.0 + (-4.0 * (score - 0.5)).exp());
        Self {
            id: id.to_string(),
            uptime,
            reliability,
            phi_c,
            integrity_points,
            age_days,
            score,
            normalized,
        }
    }

    /// Limiar de reputação para melhoria de selo (Loopseal-3).
    pub fn is_trusted(&self) -> bool {
        self.normalized >= 70.0
    }
}

/// Avalia a reputação combinada dos nós de um subset; útil para auditoria.
pub fn evaluate_node_reputation(scores: &[ReputationScore]) -> f64 {
    if scores.is_empty() {
        return 0.0;
    }
    scores.iter().map(|s| s.score).sum::<f64>() / scores.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responsive_honest_node_is_trusted() {
        let s = ReputationScore::evaluate("n1", 0.99, 0.98, 0.971, 2.0, 10.0);
        assert!(s.is_trusted(), "nó honesto deve ser confiável ({})", s.normalized);
        assert!((0.0..=100.0).contains(&s.normalized));
    }

    #[test]
    fn dishonest_silent_node_is_blacklisted() {
        let s = ReputationScore::evaluate("n2", 0.10, 0.05, 0.3, -2.0, 90.0);
        assert!(!s.is_trusted());
    }

    #[test]
    fn fresh_scores_dominate_overage() {
        let fresh = ReputationScore::evaluate("a", 0.9, 0.9, 0.9, 0.0, 0.0);
        let old = ReputationScore::evaluate("b", 0.9, 0.9, 0.9, 0.0, 400.0);
        assert!(fresh.score > old.score, "reputação deve decair com o tempo");
    }

    #[test]
    fn average_is_sane() {
        let a = ReputationScore::evaluate("x", 0.5, 0.5, 0.5, 0.0, 0.0);
        let b = ReputationScore::evaluate("y", 0.5, 0.5, 0.5, 0.0, 0.0);
        let avg = evaluate_node_reputation(&[a, b]);
        assert!(avg > 0.0 && avg <= 1.0);
    }
}