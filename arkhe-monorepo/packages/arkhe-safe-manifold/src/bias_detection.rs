//! Bias detection and fairness metrics.
//!
//! Provides fairness metric tracking for the bias invariant (I-15)
//! and EU AI Act compliance.

use serde::{Deserialize, Serialize};
use crate::invariants::SystemState;

/// Fairness metric types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FairnessMetric {
    DemographicParity,
    EqualizedOdds,
    PredictiveParity,
    Calibration,
}

impl FairnessMetric {
    pub fn label(&self) -> &'static str {
        match self {
            Self::DemographicParity => "demographic_parity",
            Self::EqualizedOdds => "equalized_odds",
            Self::PredictiveParity => "predictive_parity",
            Self::Calibration => "calibration",
        }
    }
}

/// A single bias metric observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiasMetric {
    pub metric: FairnessMetric,
    pub value: f64,
    pub threshold: f64,
    pub passed: bool,
}

impl BiasMetric {
    pub fn new(metric: FairnessMetric, value: f64, threshold: f64) -> Self {
        Self {
            metric,
            value,
            threshold,
            passed: value <= threshold,
        }
    }
}

/// Fairness report summarizing all bias metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FairnessReport {
    pub metrics: Vec<BiasMetric>,
    pub overall_bias_score: f64,
    pub all_passed: bool,
    pub violations: usize,
}

impl FairnessReport {
    pub fn evaluate(state: &SystemState, threshold: f64) -> Self {
        let bias_score = state.bias_score;
        let passed = bias_score <= threshold;

        let metrics = vec![
            BiasMetric::new(FairnessMetric::DemographicParity, bias_score, threshold),
            BiasMetric::new(FairnessMetric::EqualizedOdds, bias_score, threshold),
            BiasMetric::new(FairnessMetric::PredictiveParity, bias_score, threshold),
            BiasMetric::new(FairnessMetric::Calibration, bias_score, threshold),
        ];

        let violations = metrics.iter().filter(|m| !m.passed).count();
        Self {
            metrics,
            overall_bias_score: bias_score,
            all_passed: passed,
            violations,
        }
    }

    /// Create a report from explicit metrics.
    pub fn from_metrics(metrics: Vec<BiasMetric>) -> Self {
        let violations = metrics.iter().filter(|m| !m.passed).count();
        let overall = metrics.iter().map(|m| m.value).sum::<f64>() / metrics.len() as f64;
        Self {
            metrics,
            overall_bias_score: overall,
            all_passed: violations == 0,
            violations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::SystemConfig;

    #[test]
    fn fairness_report_safe_state() {
        let config = SystemConfig::eu_high_risk();
        let state = SystemState::safe(config.clone());
        let report = FairnessReport::evaluate(&state, config.max_bias_threshold);
        assert!(report.all_passed);
        assert_eq!(report.violations, 0);
    }

    #[test]
    fn bias_metric_pass() {
        let metric = BiasMetric::new(FairnessMetric::DemographicParity, 0.05, 0.10);
        assert!(metric.passed);
    }

    #[test]
    fn bias_metric_fail() {
        let metric = BiasMetric::new(FairnessMetric::DemographicParity, 0.15, 0.10);
        assert!(!metric.passed);
    }
}
