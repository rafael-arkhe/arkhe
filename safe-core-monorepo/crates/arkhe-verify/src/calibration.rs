use arkhe_core::TypedValue;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Decisão calibrada: output com probabilidade e incerteza.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibratedDecision {
    /// Probabilidade da decisão (0.0 a 1.0).
    pub probability: f64,
    /// Incerteza associada (0.0 a 1.0).
    pub uncertainty: f64,
    /// Valor type-safe da decisão.
    pub value: TypedValue,
    /// Metadados de calibração (opcional).
    pub calibration_metadata: Option<CalibrationMetadata>,
}

/// Metadados de calibração para auditoria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationMetadata {
    /// Método de calibração (ex: "RLCD", "temperature_scaling").
    pub method: String,
    /// Data de calibração.
    pub calibrated_at: u64,
    /// Dataset de calibração (hash).
    pub dataset_hash: Option<[u8; 32]>,
    /// Vendor-tested ou independente.
    pub evaluation_type: EvaluationType,
}

/// Tipo de avaliação da calibração.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvaluationType {
    /// Avaliado pelo fornecedor (vendor-tested).
    VendorTested,
    /// Avaliado independentemente.
    Independent,
    /// Não avaliado.
    NotEvaluated,
}

/// Erros da verificação de calibração.
#[derive(Debug, Error)]
pub enum CalibrationError {
    /// A decisão não satisfaz os limiares de calibração.
    #[error("decisão não calibrada: prob={probability}, incerteza={uncertainty}")]
    NotCalibrated {
        /// Probabilidade observada, abaixo do mínimo exigido.
        probability: f64,
        /// Incerteza observada, acima do máximo tolerado.
        uncertainty: f64,
    },
    /// A avaliação não é independente.
    #[error("avaliação não independente: {0:?}")]
    NotIndependentlyEvaluated(EvaluationType),
    /// Faltam os metadados de calibração.
    #[error("metadados de calibração em falta")]
    MissingCalibrationMetadata,
}

impl CalibratedDecision {
    /// Verifica se a decisão é calibrada.
    pub fn is_calibrated(&self, min_prob: f64, max_uncertainty: f64) -> bool {
        self.probability >= min_prob && self.uncertainty <= max_uncertainty
    }

    /// Fail-closed: se não calibrada, rejeita a decisão.
    pub fn verify(&self, min_prob: f64, max_uncertainty: f64) -> Result<(), CalibrationError> {
        if !self.is_calibrated(min_prob, max_uncertainty) {
            return Err(CalibrationError::NotCalibrated {
                probability: self.probability,
                uncertainty: self.uncertainty,
            });
        }
        let metadata = self.calibration_metadata.as_ref()
            .ok_or(CalibrationError::MissingCalibrationMetadata)?;
        if metadata.evaluation_type == EvaluationType::VendorTested {
            tracing::warn!(
                "Calibração vendor-tested: {:?}. Métricas não verificadas independentemente.",
                metadata.method
            );
        }
        Ok(())
    }

    /// Verifica se a decisão é calibrada E independentemente avaliada.
    pub fn verify_independent(&self, min_prob: f64, max_uncertainty: f64) -> Result<(), CalibrationError> {
        self.verify(min_prob, max_uncertainty)?;
        let metadata = self.calibration_metadata.as_ref()
            .ok_or(CalibrationError::MissingCalibrationMetadata)?;
        if metadata.evaluation_type != EvaluationType::Independent {
            return Err(CalibrationError::NotIndependentlyEvaluated(metadata.evaluation_type));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibrated_decision_accepts_valid() {
        let decision = CalibratedDecision {
            probability: 0.95,
            uncertainty: 0.05,
            value: TypedValue::Bool(true),
            calibration_metadata: Some(CalibrationMetadata {
                method: "RLCD".into(),
                calibrated_at: 0,
                dataset_hash: None,
                evaluation_type: EvaluationType::VendorTested,
            }),
        };
        assert!(decision.verify(0.9, 0.1).is_ok());
    }

    #[test]
    fn calibrated_decision_rejects_uncalibrated() {
        let decision = CalibratedDecision {
            probability: 0.5,
            uncertainty: 0.5,
            value: TypedValue::Bool(true),
            calibration_metadata: None,
        };
        assert!(decision.verify(0.9, 0.1).is_err());
    }

    #[test]
    fn verify_independent_rejects_vendor_tested() {
        let decision = CalibratedDecision {
            probability: 0.95,
            uncertainty: 0.05,
            value: TypedValue::Bool(true),
            calibration_metadata: Some(CalibrationMetadata {
                method: "RLCD".into(),
                calibrated_at: 0,
                dataset_hash: None,
                evaluation_type: EvaluationType::VendorTested,
            }),
        };
        assert!(decision.verify_independent(0.9, 0.1).is_err());
    }
}
