//! Network-layer anomaly detection (Huawei SmartAD-style flow scanner).

/// A network anomaly detector: maps a feature vector to an anomaly
/// probability in `[0, 1]` and decides whether it crossed the alarm
/// threshold.
pub trait NetworkAnomalyDetector {
    /// Anomaly probability of the given flow-feature vector.
    fn detect(&self, features: &[f64]) -> f64;

    /// Alarm threshold. A detection at or above it is a full anomaly.
    fn threshold(&self) -> f64;

    /// Whether the flow is anomalous under this detector.
    fn is_anomaly(&self, features: &[f64]) -> bool {
        self.detect(features) >= self.threshold()
    }
}

/// One fired (or candidate) network anomaly event.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SmartADEvent {
    /// Identifier of the flow that triggered the event.
    pub flow_id: String,
    /// Anomaly probability assigned by the detector.
    pub anomaly_probability: f64,
    /// Feature-space delta that drove the probability.
    pub feature_delta: f64,
    /// Detection timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Huawei SmartAD-style detector.
///
/// Compares the live flow-feature vector against a learned baseline. The
/// anomaly probability is `dist / (1 + dist)` with `dist` the Euclidean
/// distance from baseline — near zero for nominal traffic, rising to `>= 1/2`
/// as soon as the flow deviates and saturating toward `1.0` for intrusions.
#[derive(Clone, Debug)]
pub struct HuaweiSmartADDetector {
    threshold: f64,
    baseline: Vec<f64>,
}

impl HuaweiSmartADDetector {
    /// Detector with an all-zeros baseline and the given alarm threshold.
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            baseline: Vec::new(),
        }
    }

    /// Detector over an explicit learned baseline.
    pub fn with_baseline(baseline: Vec<f64>, threshold: f64) -> Self {
        Self { threshold, baseline }
    }

    /// The learned baseline.
    pub fn baseline(&self) -> &[f64] {
        &self.baseline
    }
}

impl Default for HuaweiSmartADDetector {
    fn default() -> Self {
        Self::new(0.95)
    }
}

fn euclidean(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

impl NetworkAnomalyDetector for HuaweiSmartADDetector {
    fn detect(&self, features: &[f64]) -> f64 {
        if self.baseline.is_empty() || features.is_empty() {
            return 0.0;
        }
        let dist = euclidean(features, &self.baseline);
        (dist / (1.0 + dist)).min(1.0)
    }

    fn threshold(&self) -> f64 {
        self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nominal_traffic_is_below_threshold() {
        let d = HuaweiSmartADDetector::with_baseline(vec![1.0; 8], 0.95);
        let p = d.detect(&[1.05; 8]);
        assert!(p < 0.95, "nominal flow scored {p}");
        assert!(!d.is_anomaly(&[1.05; 8]));
    }

    #[test]
    fn intrusion_crosses_threshold() {
        let d = HuaweiSmartADDetector::with_baseline(vec![1.0; 8], 0.95);
        let p = d.detect(&[9.0; 8]);
        assert!(p >= 0.95, "intrusion scored {p}");
        assert!(d.is_anomaly(&[9.0; 8]));
    }

    #[test]
    fn partial_detection_stays_below_threshold() {
        let d = HuaweiSmartADDetector::with_baseline(vec![1.0; 8], 0.95);
        let p = d.detect(&[1.25; 8]);
        assert!((0.3..0.5).contains(&p), "partial scored {p}");
        assert!(!d.is_anomaly(&[1.25; 8]));
    }

    #[test]
    fn empty_baseline_is_benign() {
        let d = HuaweiSmartADDetector::new(0.95);
        assert_eq!(d.detect(&[9.0; 8]), 0.0);
    }
}
