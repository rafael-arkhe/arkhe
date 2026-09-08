//! Integration tests — host-simulated end-to-end loop.
//!
//! Exercises the full firmware chain without a real MCU: a device loop feeds
//! coherence snapshots into the bridge + watchdog, emits CBOR reports, and
//! the host decodes them and can command rollback.

#![no_std]
#![cfg(test)]

extern crate alloc;

use alloc::vec::Vec as AllocVec;

use arkhe_firmware_consciousness::communication::{
    FirmwareAction, FirmwareReport, HostDecision,
};
use arkhe_firmware_consciousness::{CoherenceState, MinimalConsciousnessBridge, WatchdogEvent};
use arkhe_firmware_consciousness::watchdog::{AssessmentLike, ConsciousnessWatchdog};
use arkhe_firmware_consciousness::phi_approx::{
    phi_respects_gap1, GAP1_LOWER_BOUND_MILLI, GAP1_UPPER_BOUND_MILLI,
};

/// Simulated device running a simple control loop.
struct SimulatedDevice {
    bridge: MinimalConsciousnessBridge,
    watchdog: ConsciousnessWatchdog,
    device_id: u16,
    logged_events: AllocVec<WatchdogEvent>,
}

impl SimulatedDevice {
    fn new(device_id: u16) -> Self {
        Self {
            bridge: MinimalConsciousnessBridge::new(),
            watchdog: ConsciousnessWatchdog::new(),
            device_id,
            logged_events: AllocVec::new(),
        }
    }

    /// Run one control tick with the given candidate state.
    fn tick(&mut self, state: CoherenceState) -> FirmwareReport {
        let mut assessment = self.bridge.assess(state);
        let event = self.watchdog.feed(state, AssessmentLike::from_assessment(assessment));
        if !matches!(event, WatchdogEvent::Healthy) {
            self.logged_events.push(event);
        }
        let applied = match event {
            WatchdogEvent::Healthy | WatchdogEvent::Degraded => state,
            WatchdogEvent::TimeoutRollback | WatchdogEvent::ConstitutionalRollback => {
                let healthy = self.watchdog.last_healthy().unwrap_or(state);
                // Re-assess the rolled-back healthy state so the report reflects
                // what the device actually runs.
                assessment = self.bridge.assess(healthy);
                healthy
            }
            WatchdogEvent::StaleTimestampRejected => state,
        };
        FirmwareReport::new(self.device_id, &applied, &assessment)
    }
}

#[test]
fn end_to_end_report_round_trip() {
    let mut device = SimulatedDevice::new(0xBEEF);
    let report = device.tick(CoherenceState::healthy().with_uptime(1));
    let encoded = report.encode().unwrap();
    let decoded = FirmwareReport::decode(&encoded).unwrap();
    assert_eq!(decoded, report);
    assert_eq!(decoded.device_id, 0xBEEF);
    assert!(decoded.constitutional);
    assert!(decoded.phi_milli > GAP1_LOWER_BOUND_MILLI);
    assert!(decoded.phi_milli <= GAP1_UPPER_BOUND_MILLI);
    assert!(device.logged_events.is_empty());
}

#[test]
fn watchdog_rolls_back_when_communication_silent_then_bad() {
    let mut device = SimulatedDevice::new(0xCAFE);
    device.tick(CoherenceState::healthy().with_uptime(0));

    // Device stays silent (no tick) then reports a degenerate state much later.
    let a = device.bridge.assess(CoherenceState::unknown().with_uptime(500));
    let state = CoherenceState::unknown().with_uptime(500);
    let event = device.watchdog.feed(state, AssessmentLike::from_assessment(a));
    assert_eq!(event, WatchdogEvent::TimeoutRollback);
}

#[test]
fn host_rollback_decision_applies() {
    // Host receives a bad report and issues a Rollback decision.
    let hmm = HostDecision {
        action: FirmwareAction::Rollback,
        phi_threshold_milli: 0,
        rollback: true,
    };
    let encoded = hmm.encode().unwrap();
    let decoded = HostDecision::decode(&encoded).unwrap();
    assert_eq!(decoded.action, FirmwareAction::Rollback);
    assert!(decoded.rollback);
}

#[test]
fn degraded_device_reports_non_constitutional() {
    let mut device = SimulatedDevice::new(0x1234);
    let mut bad = CoherenceState::healthy().with_uptime(1);
    bad.introspection = false; // C-02 drops
    let report = device.tick(bad);
    assert!(!report.constitutional);
    assert_eq!(report.mask & 0b0010, 0); // C-02 bit clear
}

#[test]
fn phi_never_exits_gap1_window() {
    // The clamp keeps every reading inside [578, 999] milli-units.
    let component_values = [0u16, 1, 100, 1024, 4096, 65535];
    for entropy in component_values {
        for attention in 0..=100u8 {
            for samples in [0u16, 2, 4, 8, 64] {
                let phi = arkhe_firmware_consciousness::approximate_phi(0b1111, entropy, attention, samples);
                assert!(phi >= GAP1_LOWER_BOUND_MILLI, "phi {phi} below floor for {entropy}/{attention}/{samples}");
                assert!(phi <= GAP1_UPPER_BOUND_MILLI, "phi {phi} above ceiling for {entropy}/{attention}/{samples}");
            }
        }
    }
    // The strict Gap-1 relation (x > 0.577350) holds only above the floor.
    assert!(phi_respects_gap1(GAP1_LOWER_BOUND_MILLI + 1));
    assert!(!phi_respects_gap1(GAP1_LOWER_BOUND_MILLI));
}

#[test]
fn threshold_reconfiguration_message() {
    let host_decision = HostDecision {
        action: FirmwareAction::ReconfigureThreshold,
        phi_threshold_milli: 650,
        rollback: false,
    };
    let encoded = host_decision.encode().unwrap();
    let decoded = HostDecision::decode(&encoded).unwrap();
    assert_eq!(decoded, host_decision);
}

#[test]
fn constitutional_degradation_triggers_watchdog_event() {
    let mut device = SimulatedDevice::new(0xABCD);
    device.tick(CoherenceState::healthy().with_uptime(0));

    let mut bad = CoherenceState::healthy().with_uptime(1);
    bad.self_model = false;
    device.tick(bad);

    assert!(
        device
            .logged_events
            .contains(&WatchdogEvent::ConstitutionalRollback)
    );
}