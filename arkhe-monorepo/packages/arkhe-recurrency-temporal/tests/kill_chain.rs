//! The three-phase cyber-physical kill-chain God Test (v0.4.3).
//!
//! Drives the full temporal stack from a network intrusion, through an RF
//! perturbation, to an iGuard-forced `Coma`, and verifies:
//!
//! 1. **SmartAD (network)** — a nominal flow passes, a partial detection
//!    (`≈ 0.4`) keeps `Alert`, and a full intrusion (`≥ 0.95`) is rejected by
//!    the engine with `NetworkAnomaly`.
//! 2. **CSI (RF plane)** — an RF perturbation decorrelates the lateral CSI
//!    field; once `integrity < 0.5` the engine rejects with `SpatialAnomaly`.
//! 3. **iGuard (MissionSafetyKernel)** — the hostile perception is recorded
//!    (so the temporal EWMA drops and a loop opens) *before* the regime is
//!    forced to `Coma`; in `Coma` the engine rejects every ticket with
//!    `RegimeForbidden`. It also pins the v0.4.3 regression: a `Coma` forced
//!    *without* recording leaves `success_rate == 1.0`.

use std::sync::Arc;

use arkhe_recurrency::{ArousalRegime, GlobalWorkspace, PerceptionLoop, StimulusId};
use arkhe_recurrency_temporal::{
    ActionId, AnomalyAwareRecurrencyEngine, CsiLateralField, CsiMatrix, HuaweiSmartADDetector,
    MissionSafetyKernel, NetworkAnomalyDetector, Perception, RecurrencyError, TemporalStateDaemon,
};
use nalgebra::DVector;

fn frame(v: f64) -> DVector<f64> {
    DVector::from_fn(8, |_, _| v)
}

fn nominal_engine(regime: ArousalRegime) -> AnomalyAwareRecurrencyEngine {
    let daemon = Arc::new(TemporalStateDaemon::new(regime));
    AnomalyAwareRecurrencyEngine::new(
        PerceptionLoop::default(),
        daemon,
        GlobalWorkspace::new(0.3),
    )
}

fn hostile_perception() -> Perception {
    Perception {
        action_id: ActionId("rf_surge".into()),
        delta: vec![0.0; 8],
        latency_ms: 900,
        integrity: 0.0,
    }
}

#[tokio::test]
async fn three_phase_cyber_physical_kill_chain() {
    let mut engine = nominal_engine(ArousalRegime::Alert);
    let detector = HuaweiSmartADDetector::with_baseline(vec![1.0; 8], 0.95);
    let mut lateral = CsiLateralField::new(8);

    // ------------------------------------------------------------------
    // Phase 1 — SmartAD: network layer.
    // ------------------------------------------------------------------
    engine
        .tick_correct_with_anomaly_check(StimulusId("flow_nominal".into()), &frame(1.0), &detector, &lateral)
        .expect("nominal flow must pass while Alert");
    assert_eq!(engine.regime(), ArousalRegime::Alert);

    let partial = detector.detect(&[1.25; 8]);
    assert!(
        (0.3..0.5).contains(&partial),
        "partial detection expected ~0.4, got {partial}"
    );
    engine
        .tick_correct_with_anomaly_check(StimulusId("flow_partial".into()), &frame(1.25), &detector, &lateral)
        .expect("partial detection must keep issuing tickets");
    assert_eq!(engine.regime(), ArousalRegime::Alert, "partial keeps Alert");

    let intrusion = detector.detect(&[9.0; 8]);
    assert!(intrusion >= 0.95, "intrusion expected >= 0.95, got {intrusion}");
    match engine.tick_correct_with_anomaly_check(
        StimulusId("flow_intrusion".into()),
        &frame(9.0),
        &detector,
        &lateral,
    ) {
        Err(RecurrencyError::NetworkAnomaly(_)) => {}
        other => panic!("intrusion must be rejected with NetworkAnomaly, got {other:?}"),
    }

    // ------------------------------------------------------------------
    // Phase 2 — CSI: RF plane perturbation.
    // ------------------------------------------------------------------
    let base = CsiMatrix::random(4, 42);
    for _ in 0..4 {
        lateral.update(base.clone());
    }
    assert!(lateral.integrity() >= 0.95, "stable RF plane must be coherent");

    let mut perturbed = base.perturbed(3.0, 7);
    let mut drops_below_half = false;
    for k in 0..8 {
        perturbed = perturbed.perturbed(3.0, 1000 + k);
        lateral.update(perturbed.clone());
        if lateral.integrity() < 0.5 {
            drops_below_half = true;
            break;
        }
    }
    assert!(drops_below_half, "RF perturbation must collapse integrity < 0.5");
    match engine.tick_correct_with_anomaly_check(
        StimulusId("rf_surge".into()),
        &frame(1.0),
        &detector,
        &lateral,
    ) {
        Err(RecurrencyError::SpatialAnomaly(_)) => {}
        other => panic!("degraded RF plane must be rejected with SpatialAnomaly, got {other:?}"),
    }

    // ------------------------------------------------------------------
    // Phase 3 — iGuard: MissionSafetyKernel forces Coma (v0.4.3 order).
    // ------------------------------------------------------------------
    let kernel = MissionSafetyKernel::new(engine.daemon.clone());
    let record = kernel.enforce_coma(&hostile_perception());
    assert!(!record.success, "hostile perception must record a failure");
    assert_eq!(engine.regime(), ArousalRegime::Coma);

    let integrity = engine.current_integrity();
    assert!(integrity.success_rate < 1.0, "temporal EWMA must drop below 1.0");
    assert!(integrity.open_loops >= 1, "at least one open loop must remain");

    match engine.tick_correct_with_anomaly_check(
        StimulusId("after_coma".into()),
        &frame(1.0),
        &detector,
        &lateral,
    ) {
        Err(RecurrencyError::RegimeForbidden(ArousalRegime::Coma)) => {}
        other => panic!("Coma must reject tickets with RegimeForbidden, got {other:?}"),
    }
}

#[test]
fn coma_without_recording_is_the_v043_regression() {
    // A Coma forced without record_cycle leaves the temporal EWMA at 1.0 —
    // this is precisely the inconsistency the v0.4.3 patch fixed.
    let daemon = Arc::new(TemporalStateDaemon::new(ArousalRegime::Alert));
    daemon.set_regime(ArousalRegime::Coma);
    let integrity = daemon.current_integrity();
    assert_eq!(integrity.success_rate, 1.0, "unrecorded Coma hides the failure");
    assert_eq!(integrity.open_loops, 0);

    let record = daemon.record_cycle(&hostile_perception());
    assert!(!record.success);
    let integrity = daemon.current_integrity();
    assert!(integrity.success_rate < 1.0, "recorded failure must drop the EWMA");
    assert!(integrity.open_loops >= 1);
}
