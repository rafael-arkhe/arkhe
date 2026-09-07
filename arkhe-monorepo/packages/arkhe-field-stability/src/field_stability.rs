//! Cálculo da estabilidade de campo com métricas de latência.
//!
//! A estabilidade de campo mede a consistência de um sistema com base em
//! três dimensões:
//!
//! * **Estabilidade**: 1 − desvio padrão normalizado da coerência Φ no tempo.
//! * **Taxa de sucesso**: proporção de valores de coerência ≥ 0.7.
//! * **Latência**: pontuação `1 − (latência/tolerância)²`, com saturação em 0.

use serde::{Deserialize, Serialize};

/// Agrega métricas de estabilidade de campo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldStability {
    /// Identificador do campo (nome com timestamp UTC de criação).
    pub name: String,
    /// Φ_estabilidade ∈ [0,1].
    pub stability: f64,
    /// Φ_sucesso ∈ [0,1].
    pub success_rate: f64,
    /// Pontuação de latência ∈ [0,1] (0 = acima do limite; NÃO é ms bruto).
    pub latency_score: f64,
}

impl FieldStability {
    /// Cria uma instância a partir de um histórico de coerência.
    ///
    /// # Argumentos
    ///
    /// * `history` — série temporal de valores de coerência Φ ∈ [0,1].
    /// * `latency_mean` — latência média observada (ms).
    /// * `latency_tolerance` — tolerância de latência (ms).
    pub fn new(history: Vec<f64>, latency_mean: f64, latency_tolerance: f64) -> Self {
        let stability = Self::compute_stability(&history);
        let success_rate = Self::compute_success_rate(&history);
        let latency = Self::compute_latency_score(latency_mean, latency_tolerance);

        Self {
            name: format!("campo_{}", chrono::Utc::now().timestamp()),
            stability,
            success_rate,
            latency_score: latency,
        }
    }

    /// Estabilidade = 1 − desvio padrão normalizado (std máximo assumido 0.5).
    pub fn compute_stability(history: &[f64]) -> f64 {
        if history.len() < 2 {
            return 1.0;
        }
        let mean = history.iter().sum::<f64>() / history.len() as f64;
        let variance = history
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / history.len() as f64;
        let std = variance.sqrt();
        let normalized_std = (std / 0.5).min(1.0);
        1.0 - normalized_std
    }

    /// Taxa de sucesso = proporção de valores ≥ 0.7.
    pub fn compute_success_rate(history: &[f64]) -> f64 {
        if history.is_empty() {
            return 0.0;
        }
        let success_count = history.iter().filter(|&&x| x >= 0.7).count();
        success_count as f64 / history.len() as f64
    }

    /// Pontuação de latência = `1 − (latência/tolerância)²`, satura em 0.
    pub fn compute_latency_score(latency_mean: f64, tolerance: f64) -> f64 {
        if tolerance <= 0.0 {
            return 0.0;
        }
        let normalized = (latency_mean / tolerance).min(1.0);
        1.0 - normalized * normalized
    }
}

impl Default for FieldStability {
    fn default() -> Self {
        Self {
            name: "campo_default".to_string(),
            stability: 0.85,
            success_rate: 0.90,
            latency_score: 0.75,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stability_high() {
        let history = vec![0.95, 0.93, 0.94, 0.96, 0.92];
        let fs = FieldStability::new(history, 20.0, 50.0);
        assert!(fs.stability > 0.8);
    }

    #[test]
    fn test_stability_low() {
        let history = vec![0.9, 0.5, 0.8, 0.3, 0.7];
        let fs = FieldStability::new(history, 20.0, 50.0);
        assert!(fs.stability < 0.7);
    }

    #[test]
    fn test_success_rate() {
        let history = vec![0.8, 0.6, 0.9, 0.7, 0.5];
        let fs = FieldStability::new(history, 20.0, 50.0);
        assert!((fs.success_rate - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_latency_score() {
        let score = FieldStability::compute_latency_score(25.0, 50.0);
        assert!((score - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_latency_score_saturates_at_zero() {
        let score = FieldStability::compute_latency_score(500.0, 50.0);
        assert_eq!(score, 0.0);
    }
}