//! Relatório de qualidade consolidado.
//!
//! Agrega as métricas de estabilidade, sucesso e latência em um único
//! relatório com score global ponderado.

use serde::{Deserialize, Serialize};

use crate::field_stability::FieldStability;

/// Relatório de qualidade do sistema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    /// Média ponderada (pesos de [`crate::constants`]).
    pub overall: f64,
    /// Estabilidade ∈ [0,1].
    pub stability: f64,
    /// Taxa de sucesso ∈ [0,1].
    pub success_rate: f64,
    /// Score de latência ∈ [0,1].
    pub latency_score: f64,
    /// Momento da medição (RFC 3339).
    pub timestamp: String,
}

impl QualityReport {
    /// Gera um relatório a partir de uma instância de [`FieldStability`].
    ///
    /// # Argumentos
    ///
    /// * `fs` — métricas de estabilidade de campo (scores em [0,1]).
    /// * `w_stability` — peso da estabilidade (recomendado 0.4).
    /// * `w_success` — peso da taxa de sucesso (recomendado 0.4).
    /// * `w_latency` — peso da latência (recomendado 0.2).
    ///
    /// Os pesos devem somar 1 para que `overall` permaneça em [0,1].
    #[must_use]
    pub fn from_field_stability(
        fs: &FieldStability,
        w_stability: f64,
        w_success: f64,
        w_latency: f64,
    ) -> Self {
        let overall = w_stability * fs.stability
            + w_success * fs.success_rate
            + w_latency * fs.latency_score;

        Self {
            overall,
            stability: fs.stability,
            success_rate: fs.success_rate,
            latency_score: fs.latency_score,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Verifica se a qualidade atende a um limiar mínimo.
    #[must_use]
    pub fn is_acceptable(&self, threshold: f64) -> bool {
        self.overall >= threshold
    }
}

impl Default for QualityReport {
    fn default() -> Self {
        Self {
            overall: 0.85,
            stability: 0.85,
            success_rate: 0.90,
            latency_score: 0.75,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_stability::FieldStability;

    #[test]
    fn test_quality_report() {
        let fs = FieldStability::default();
        let report = QualityReport::from_field_stability(&fs, 0.4, 0.4, 0.2);
        assert!(report.overall > 0.8);
        assert!(report.is_acceptable(0.75));
    }

    #[test]
    fn test_weights() {
        let fs = FieldStability::default();
        let report1 = QualityReport::from_field_stability(&fs, 1.0, 0.0, 0.0);
        let report2 = QualityReport::from_field_stability(&fs, 0.0, 1.0, 0.0);
        assert_eq!(report1.overall, fs.stability);
        assert_eq!(report2.overall, fs.success_rate);
    }

    #[test]
    fn test_overall_stays_in_range_with_normalized_weights() {
        // Pesos 0.4/0.4/0.2 somam 1 → overall ∈ [0,1] e igual ao valor esperado.
        let fs = FieldStability {
            stability: 1.0,
            success_rate: 1.0,
            ..Default::default()
        };
        let report = QualityReport::from_field_stability(&fs, 0.4, 0.4, 0.2);
        let expected = 0.4 * 1.0 + 0.4 * 1.0 + 0.2 * fs.latency_score;
        assert!((report.overall - expected).abs() < 1e-9);
        assert!(report.overall <= 1.0);
    }
}