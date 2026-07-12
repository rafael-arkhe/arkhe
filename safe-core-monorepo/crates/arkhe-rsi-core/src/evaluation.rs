use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Resultado da avaliação de um artefato candidato.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub score: f64,
    pub metrics: HashMap<String, f64>,
}

impl EvaluationResult {
    pub fn new(score: f64) -> Self {
        Self {
            score,
            metrics: HashMap::new(),
        }
    }

    pub fn with_metric(mut self, name: impl Into<String>, value: f64) -> Self {
        self.metrics.insert(name.into(), value);
        self
    }
}
