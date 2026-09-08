//! Model explainability and interpretability tracking.
//!
//! Provides explainability score tracking for the explainability
//! invariant (I-16) and EU AI Act Article 13 compliance.

use serde::{Deserialize, Serialize};
use crate::invariants::SystemState;

/// Explanation methods supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExplanationMethod {
    Shap,
    Lime,
    AttentionWeights,
    FeatureImportance,
    Counterfactual,
}

impl ExplanationMethod {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Shap,
            Self::Lime,
            Self::AttentionWeights,
            Self::FeatureImportance,
            Self::Counterfactual,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Shap => "shap",
            Self::Lime => "lime",
            Self::AttentionWeights => "attention_weights",
            Self::FeatureImportance => "feature_importance",
            Self::Counterfactual => "counterfactual",
        }
    }
}

/// An explainability report for a single prediction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplainabilityReport {
    pub score: f64,
    pub methods_used: Vec<ExplanationMethod>,
    pub meets_threshold: bool,
    pub threshold: f64,
}

impl ExplainabilityReport {
    pub fn evaluate(state: &SystemState) -> Self {
        let threshold = state.config.min_explainability_threshold;
        let score = state.explainability_score;
        Self {
            score,
            methods_used: vec![ExplanationMethod::FeatureImportance],
            meets_threshold: score >= threshold,
            threshold,
        }
    }

    pub fn with_methods(mut self, methods: Vec<ExplanationMethod>) -> Self {
        self.methods_used = methods;
        self
    }
}

/// Explanation tracker aggregating multiple reports.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExplanationTracker {
    reports: Vec<ExplainabilityReport>,
}

impl ExplanationTracker {
    pub fn new() -> Self {
        Self { reports: Vec::new() }
    }

    pub fn add_report(&mut self, report: ExplainabilityReport) {
        self.reports.push(report);
    }

    pub fn average_score(&self) -> f64 {
        if self.reports.is_empty() {
            return 0.0;
        }
        self.reports.iter().map(|r| r.score).sum::<f64>() / self.reports.len() as f64
    }

    pub fn all_meet_threshold(&self) -> bool {
        self.reports.iter().all(|r| r.meets_threshold)
    }

    pub fn report_count(&self) -> usize {
        self.reports.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::SystemConfig;

    #[test]
    fn explainability_report_default() {
        let config = SystemConfig::default();
        let state = SystemState::safe(config);
        let report = ExplainabilityReport::evaluate(&state);
        assert!(report.meets_threshold);
        assert_eq!(report.score, 1.0);
    }

    #[test]
    fn explanation_method_all() {
        assert_eq!(ExplanationMethod::all().len(), 5);
    }

    #[test]
    fn tracker_average_score() {
        let mut tracker = ExplanationTracker::new();
        tracker.add_report(ExplainabilityReport {
            score: 0.8,
            methods_used: vec![],
            meets_threshold: true,
            threshold: 0.5,
        });
        tracker.add_report(ExplainabilityReport {
            score: 0.6,
            methods_used: vec![],
            meets_threshold: true,
            threshold: 0.5,
        });
        assert!((tracker.average_score() - 0.7).abs() < 1.0e-10);
    }
}
