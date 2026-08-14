//! Anomaly-aware recurrency engine (v0.4.3).
//!
//! Wraps the deterministic [`arkhe_recurrency::RecurrencyEngine`] and the
//! shared [`crate::daemon::TemporalStateDaemon`] into a temporal engine that
//! computes the `Φ` proxy and the `S` measure per ticket, and gates ticket
//! issuance behind the arousal regime, the network anomaly detector and the
//! lateral RF geometry integrity.
//!
//! In `Coma` every `tick_correct` call fails with
//! [`RecurrencyError::RegimeForbidden`] — the substrate stops consuming
//! stimulus until the safety interlock lifts it.

use std::sync::Arc;

use arkhe_recurrency::{
    ArousalRegime, GlobalWorkspace, PerceptionLoop, Percept, RecurrencyEngine, RecurrencyTicket,
    StateDaemon, StimulusId,
};
use nalgebra::DVector;

use crate::daemon::{TemporalIntegrity, TemporalStateDaemon};
use crate::error::RecurrencyError;
use crate::network::smartad::NetworkAnomalyDetector;
use crate::spatial::csi::CsiLateralField;

/// Saturated `Φ` proxy: `(ε·δ·scale) / (1 + ε·δ·scale)` from the closed-loop
/// error reduction `ε` and the causal-closure depth `δ`.
#[derive(Clone, Copy, Debug)]
pub struct PhiCalculator {
    /// Saturation scale.
    pub scale: f64,
}

impl PhiCalculator {
    /// Calculator with the given saturation scale.
    pub fn new(scale: f64) -> Self {
        Self { scale }
    }

    /// `Φ` from the error reduction and closure depth.
    pub fn phi(&self, error_reduction: f64, closure_depth: f64) -> f64 {
        let x = error_reduction * closure_depth * self.scale;
        if x.is_finite() {
            x / (1.0 + x)
        } else {
            1.0
        }
    }

    /// `Φ` of a completed percept.
    pub fn phi_of_percept(&self, percept: &Percept, closure_depth: f64) -> f64 {
        self.phi(percept.error_reduction, closure_depth)
    }
}

impl Default for PhiCalculator {
    fn default() -> Self {
        Self::new(1.0)
    }
}

/// `S` measure: how far the causal-closure depth sits above the access gate,
/// discounted if access was denied.
#[derive(Clone, Copy, Debug)]
pub struct SMeasureCalculator {
    /// Access-gate depth above which content enters the global workspace.
    pub access_gate: f64,
}

impl SMeasureCalculator {
    /// Calculator with the given access gate.
    pub fn new(access_gate: f64) -> Self {
        Self { access_gate }
    }

    /// `S` measure of a ticket, in `[0, 1]`.
    pub fn s_measure(&self, ticket: &RecurrencyTicket) -> f64 {
        let denom = (1.0 - self.access_gate).max(1e-9);
        let depth = ticket.local_depth.max(0.0);
        if depth < self.access_gate {
            0.0
        } else {
            let base = (depth - self.access_gate) / denom;
            if ticket.access_granted {
                base.min(1.0)
            } else {
                (0.5 * base).min(1.0)
            }
        }
    }
}

impl Default for SMeasureCalculator {
    fn default() -> Self {
        Self::new(0.3)
    }
}

/// A ticket extended with the temporal integrity snapshot and the `Φ`/`S`
/// measures of the cycle.
#[derive(Clone, Debug, serde::Serialize)]
pub struct TemporalTicket {
    /// The core recurrency ticket.
    pub core: RecurrencyTicket,
    /// Temporal integrity at issuance.
    pub temporal: TemporalIntegrity,
    /// `Φ` proxy of the cycle.
    pub phi: f64,
    /// `S` measure of the cycle.
    pub s_measure: f64,
}

/// Temporal recurrency engine: core engine + shared temporal daemon.
#[derive(Clone, Debug)]
pub struct TemporalRecurrencyEngine {
    /// The core deterministic pipeline.
    pub core: RecurrencyEngine,
    /// Shared temporal daemon (also owned by the safety kernel).
    pub daemon: Arc<TemporalStateDaemon>,
    /// `Φ` proxy calculator.
    pub phi: PhiCalculator,
    /// `S` measure calculator.
    pub s_measure: SMeasureCalculator,
}

impl TemporalRecurrencyEngine {
    /// Build the temporal engine over the core components.
    pub fn new(
        perception: PerceptionLoop,
        daemon: Arc<TemporalStateDaemon>,
        workspace: GlobalWorkspace,
    ) -> Self {
        Self {
            core: RecurrencyEngine::new(perception, StateDaemon::new(daemon.regime()), workspace),
            daemon,
            phi: PhiCalculator::default(),
            s_measure: SMeasureCalculator::default(),
        }
    }

    /// Current arousal regime of the shared daemon.
    pub fn regime(&self) -> ArousalRegime {
        self.daemon.regime()
    }

    /// Current temporal integrity snapshot.
    pub fn current_integrity(&self) -> TemporalIntegrity {
        self.daemon.current_integrity()
    }

    /// Issue a ticket for one stimulus frame, gated on the arousal regime.
    ///
    /// Fails with [`RecurrencyError::RegimeForbidden`] in `Coma`.
    pub fn tick_correct(
        &mut self,
        id: StimulusId,
        frame: &DVector<f64>,
    ) -> Result<TemporalTicket, RecurrencyError> {
        if self.regime() == ArousalRegime::Coma {
            return Err(RecurrencyError::RegimeForbidden(ArousalRegime::Coma));
        }
        let core = self.core.process_stimulus(id, frame);
        let phi = self.phi.phi(core.error_reduction, core.local_depth);
        let s = self.s_measure.s_measure(&core);
        Ok(TemporalTicket {
            core,
            temporal: self.daemon.current_integrity(),
            phi,
            s_measure: s,
        })
    }

    /// Issue a ticket with the full v0.4.3 anomaly gate: `Coma` regime, the
    /// network anomaly detector and the lateral RF geometry integrity must all
    /// be clear.
    ///
    /// The stimulus frame doubles as the flow-feature vector for the network
    /// detector, which is consistent with a functional simulation where the
    /// perceptual stream is the traffic stream.
    pub fn tick_correct_with_anomaly_check(
        &mut self,
        id: StimulusId,
        frame: &DVector<f64>,
        detector: &dyn NetworkAnomalyDetector,
        lateral: &CsiLateralField,
    ) -> Result<TemporalTicket, RecurrencyError> {
        if self.regime() == ArousalRegime::Coma {
            return Err(RecurrencyError::RegimeForbidden(ArousalRegime::Coma));
        }
        let features: Vec<f64> = frame.iter().copied().collect();
        if detector.is_anomaly(&features) {
            return Err(RecurrencyError::NetworkAnomaly(format!(
                "prob {:.3} >= threshold {:.3}",
                detector.detect(&features),
                detector.threshold()
            )));
        }
        if lateral.integrity() < 0.5 {
            return Err(RecurrencyError::SpatialAnomaly(lateral.integrity()));
        }
        self.tick_correct(id, frame)
    }
}

/// v0.4.3 name for the anomaly-aware engine.
pub type AnomalyAwareRecurrencyEngine = TemporalRecurrencyEngine;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::TemporalStateDaemon;
    use crate::network::smartad::HuaweiSmartADDetector;
    use crate::spatial::csi::CsiMatrix;
    use std::sync::Arc;

    fn frame(v: f64) -> DVector<f64> {
        DVector::from_fn(8, |_, _| v)
    }

    fn engine(regime: ArousalRegime) -> TemporalRecurrencyEngine {
        let daemon = Arc::new(TemporalStateDaemon::new(regime));
        TemporalRecurrencyEngine::new(
            PerceptionLoop::default(),
            daemon,
            GlobalWorkspace::new(0.3),
        )
    }

    #[test]
    fn alert_engine_issues_tickets() {
        let mut eng = engine(ArousalRegime::Alert);
        let t = eng
            .tick_correct(StimulusId("s1".into()), &frame(0.5))
            .expect("Alert must issue tickets");
        assert_eq!(t.core.tick, 1);
        assert!(t.phi >= 0.0);
        assert!(t.s_measure >= 0.0);
    }

    #[test]
    fn coma_engine_rejects_tickets() {
        let mut eng = engine(ArousalRegime::Coma);
        match eng.tick_correct(StimulusId("s1".into()), &frame(0.5)) {
            Err(RecurrencyError::RegimeForbidden(ArousalRegime::Coma)) => {}
            other => panic!("expected RegimeForbidden(Coma), got {other:?}"),
        }
    }

    #[test]
    fn network_anomaly_gates_tickets() {
        let mut eng = engine(ArousalRegime::Alert);
        let detector = HuaweiSmartADDetector::with_baseline(vec![1.0; 8], 0.95);
        let lateral = CsiLateralField::new(8);
        // Nominal frame.
        eng.tick_correct_with_anomaly_check(
            StimulusId("s1".into()),
            &frame(1.0),
            &detector,
            &lateral,
        )
        .expect("nominal must pass");
        // Intrusion frame.
        match eng.tick_correct_with_anomaly_check(
            StimulusId("s2".into()),
            &frame(9.0),
            &detector,
            &lateral,
        ) {
            Err(RecurrencyError::NetworkAnomaly(_)) => {}
            other => panic!("expected NetworkAnomaly, got {other:?}"),
        }
    }

    #[test]
    fn degraded_lateral_geometry_gates_tickets() {
        let mut eng = engine(ArousalRegime::Alert);
        let detector = HuaweiSmartADDetector::with_baseline(vec![1.0; 8], 0.95);
        let mut lateral = CsiLateralField::new(8);
        let base = CsiMatrix::random(4, 42);
        for _ in 0..4 {
            lateral.update(base.clone());
        }
        let mut p = base.perturbed(3.0, 7);
        for k in 0..6 {
            p = p.perturbed(3.0, 1000 + k);
            lateral.update(p.clone());
        }
        assert!(lateral.integrity() < 0.5);
        match eng.tick_correct_with_anomaly_check(
            StimulusId("s3".into()),
            &frame(1.0),
            &detector,
            &lateral,
        ) {
            Err(RecurrencyError::SpatialAnomaly(_)) => {}
            other => panic!("expected SpatialAnomaly, got {other:?}"),
        }
    }
}
