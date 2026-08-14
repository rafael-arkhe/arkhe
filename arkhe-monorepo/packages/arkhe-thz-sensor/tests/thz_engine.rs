//! Integration: THz sensor → real `arkhe_recurrency::RecurrencyEngine`.
//!
//! The sensor reduces a measured spectrum to an 8-dimensional feature vector
//! and feeds it through the engine's two-pass loop, so every measurement
//! advances the state daemon's tick and yields a genuine
//! [`arkhe_recurrency::RecurrencyTicket`].

use arkhe_recurrency::{ArousalRegime, GlobalWorkspace, PerceptionLoop, RecurrencyEngine, StateDaemon};
use arkhe_thz_sensor::{Analyte, ThzMetamaterialSensor, ThzSensorError};

fn engine(n: usize) -> RecurrencyEngine {
    RecurrencyEngine::new(
        PerceptionLoop {
            n,
            ..Default::default()
        },
        StateDaemon::new(ArousalRegime::Alert),
        GlobalWorkspace::new(0.3),
    )
}

#[test]
fn nominal_measurement_emits_a_ticket_with_a_closed_loop() {
    let sensor = ThzMetamaterialSensor::new();
    let mut eng = engine(8);
    let air = Analyte::air();

    let result = sensor
        .detect_anomaly(&mut eng, &air, &air, "thz_clean")
        .expect("clean measurement must not fail");
    assert!(!result.measurement.anomaly_detected);
    assert_eq!(result.measurement.shift_ghz, 0.0);
    assert!(result.ticket.local_loop_closed, "default loop must close");
    assert!(result.ticket.access_granted, "Alert must broadcast a clean frame");
    assert!(result.ticket.local_depth > 0.0);
}

#[test]
fn strong_analyte_is_flagged_but_still_ticketed() {
    let sensor = ThzMetamaterialSensor::new();
    let mut eng = engine(8);
    let air = Analyte::air();
    let strong = Analyte::new(1.40, 5.0);

    let result = sensor
        .detect_anomaly(&mut eng, &air, &strong, "thz_exposure")
        .expect("measurement must not fail");
    assert!(result.measurement.anomaly_detected);
    assert!(result.measurement.integrity < 1.0);
    assert!(result.ticket.tick >= 1);
}

#[test]
fn every_measurement_advances_the_state_daemon_tick() {
    let sensor = ThzMetamaterialSensor::new();
    let mut eng = engine(8);
    let air = Analyte::air();

    let t1 = sensor
        .detect_anomaly(&mut eng, &air, &air, "thz_t1")
        .expect("first")
        .ticket;
    let t2 = sensor
        .detect_anomaly(&mut eng, &air, &air, "thz_t2")
        .expect("second")
        .ticket;
    let t3 = sensor
        .detect_anomaly(&mut eng, &air, &air, "thz_t3")
        .expect("third")
        .ticket;
    assert!(t1.tick < t2.tick);
    assert!(t2.tick < t3.tick);
}

#[test]
fn wrong_substrate_dimension_is_rejected() {
    let sensor = ThzMetamaterialSensor::new();
    let mut eng = engine(4); // must be 8
    let air = Analyte::air();
    match sensor.detect_anomaly(&mut eng, &air, &air, "thz_bad_dim") {
        Err(ThzSensorError::DimensionMismatch { engine: 4, sensor: 8 }) => {}
        other => panic!("expected DimensionMismatch(4, 8), got {other:?}"),
    }
}

#[test]
fn fermi_tracks_the_state_daemon_regime() {
    let sensor = ThzMetamaterialSensor::new();
    let mut eng = engine(8);

    // Force the substrate into Coma: the sensor still measures, but maps its
    // Fermi target to the lowest sensing sensitivity.
    eng.daemon.set_regime(ArousalRegime::Coma);
    let mut tuned = sensor;
    tuned.set_fermi_from_regime(eng.daemon.regime());
    assert!((tuned.fermi_level() - 0.6).abs() < 1e-9);
}
