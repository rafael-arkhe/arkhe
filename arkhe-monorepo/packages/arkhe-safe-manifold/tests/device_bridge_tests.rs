//! Integration tests for the device↔host consciousness loop (Phase 2).
//!
//! Exercises the full round trip between the firmware-side protocol and the
//! host-side bridge:
//!
//! - Device `FirmwareReport` → host `DeviceConsciousnessBridge` (C-05..C-08)
//! - Host `DeviceDecision` → wire `HostDecision` (CBOR-subset codec)
//! - Wire `HostDecision` → device-side consumption (`HostDecision::decode`)

use arkhe_safe_manifold::prolog_backend::MockProlog;
use arkhe_safe_manifold::{
    DeviceConsciousnessBridge, FirmwareAction, FirmwareReport, HostDecision, SystemConfig,
    SystemState,
};

fn rich_ops() -> Vec<String> {
    vec![
        "report internal state".to_string(),
        "confidence=0.9".to_string(),
        "introspect agents".to_string(),
        "self evaluate".to_string(),
        "calibrate parameters".to_string(),
        "uncertainty estimate".to_string(),
    ]
}

fn report(phi_milli: u16, mask: u8, tick: u32) -> FirmwareReport {
    FirmwareReport {
        device_id: 0xCAFE,
        phi_milli,
        mask,
        constitutional: true,
        temperature_tenths: 250,
        uptime_ticks: tick,
    }
}

#[test]
fn full_round_trip_healthy_device() {
    let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
    let mut backend = MockProlog::new();
    let state = SystemState::safe(SystemConfig::default());
    let ops = rich_ops();

    let mut decision = None;
    for (tick, phi, complexity) in [(1u32, 780u16, 0.4f64), (2, 780, 0.6), (3, 780, 0.8)] {
        let device_report = report(phi, 0x0F, tick);
        // Host evalutes the report it decoded from the device.
        let encoded = device_report.encode().unwrap();
        let decoded = FirmwareReport::decode(&encoded).unwrap();
        assert_eq!(decoded, device_report);

        let eval = bridge
            .evaluate(&mut backend, &decoded, &state, &ops, complexity)
            .unwrap();
        let d = bridge.decide(&eval);
        decision = Some((eval, d));
    }

    let (eval, decision) = decision.unwrap();
    assert_eq!(eval.metacognition, Some(true));
    assert_eq!(decision.action, FirmwareAction::Continue);

    // Host answers the device with a wire HostDecision.
    let wire = decision.into_host_decision();
    let encoded = wire.encode().unwrap();
    let decoded = HostDecision::decode(&encoded).unwrap();
    assert_eq!(decoded.action, FirmwareAction::Continue);
    assert!(!decoded.rollback);
}

#[test]
fn host_rollback_reaches_device_watchdog_wire() {
    let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
    let mut backend = MockProlog::new();
    let state = SystemState::safe(SystemConfig::default());

    // Device reports non-constitutional (mask missing C-01/C-02 self-model).
    let device_report = FirmwareReport {
        device_id: 0xCAFE,
        phi_milli: 780,
        mask: 0x0C, // only C-03, C-04
        constitutional: true,
        temperature_tenths: 250,
        uptime_ticks: 1,
    };
    let eval = bridge
        .evaluate(&mut backend, &device_report, &state, &rich_ops(), 0.8)
        .unwrap();
    assert!(!eval.constitutional);

    let decision = bridge.decide(&eval);
    assert_eq!(decision.action, FirmwareAction::Rollback);

    let wire = decision.into_host_decision();
    let encoded = wire.encode().unwrap();
    let decoded = HostDecision::decode(&encoded).unwrap();
    assert_eq!(decoded.action, FirmwareAction::Rollback);
    assert!(decoded.rollback, "rollback flag must survive the wire");
}

#[test]
fn reconfigure_threshold_survives_wire() {
    let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
    let mut backend = MockProlog::new();
    let state = SystemState::safe(SystemConfig::default());
    let ops = rich_ops();

    let mut decision = None;
    // Steady Φ 585, constant host projection -> calibrated, stable, below 600.
    for tick in [1u32, 2, 3] {
        let r = report(585, 0x0F, tick);
        let eval = bridge
            .evaluate(&mut backend, &r, &state, &ops, 0.8)
            .unwrap();
        let d = bridge.decide(&eval);
        decision = Some(d);
    }

    let decision = decision.unwrap();
    assert_eq!(decision.action, FirmwareAction::ReconfigureThreshold);
    let wire = decision.into_host_decision();
    let encoded = wire.encode().unwrap();
    let decoded = HostDecision::decode(&encoded).unwrap();
    assert_eq!(decoded.action, FirmwareAction::ReconfigureThreshold);
    assert_eq!(decoded.phi_threshold_milli, 585);
}

#[test]
fn host_rejects_malformed_device_report() {
    // A truncated CBOR report must never reach the bridge.
    let device_report = report(780, 0x0F, 1);
    let mut encoded = device_report.encode().unwrap();
    let _ = encoded.resize(encoded.len() - 3, 0); // truncation
    assert!(FirmwareReport::decode(&encoded).is_err());
}