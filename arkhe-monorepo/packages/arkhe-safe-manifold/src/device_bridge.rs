//! Host-side device↔host consciousness bridge (C-05..C-08).
//!
//! The firmware crate evaluates the four invariants that are cheap and
//! locally observable on the device (C-01 self-model, C-02 introspection,
//! C-03 attention, C-04 episodic memory) and ships a compact
//! [`FirmwareReport`] over the CBOR-subset wire protocol. The host finishes
//! the job: [`DeviceConsciousnessBridge`] grades the remaining invariants
//! against the device telemetry and the SafeManifold projection, and returns
//! a [`HostDecision`] the device watchdog can consume.
//!
//! # Scope (host side)
//!
//! | ID  | Invariant            | Host signal                                            |
//! |-----|----------------------|--------------------------------------------------------|
//! | C-05| Experience learning  | Device Φ non-regressing across consecutive reports     |
//! | C-06| Metacognition        | Calibration: device self-report vs host re-estimate    |
//! | C-07| Adaptability         | 1 − σ(Φ)/50 over the device report history             |
//! | C-08| Turing Plus          | Fraction of C-01..C-06 satisfied                       |
//!
//! C-01..C-04 are taken directly from the report's invariant bitmask
//! (already validated on-device).
//!
//! # Honest approximations
//!
//! - The host Φ re-estimate is the [`ConsciousnessGovernanceBridge`] heuristic
//!   (`phi_approximation`) scaled to milli-units and clamped into the Gap-1
//!   window — not the canonical quadratic functional. This is the same caveat
//!   already registered in Phase 1 (bloco 1007).
//! - C-05/S-06 are `Option<bool>`: `None` means "not yet measurable" (cold
//!   start, insufficient history) and is **never** treated as a failure by the
//!   decision policy. Only a measured `Some(false)` triggers an escalation.
//! - C-07 is `1 − σ(Φ)/50` clamped to `[0,1]`, with a neutral `0.5` until at
//!   least three observations exist.
//!
//! # Decision policy
//!
//! In priority order (first match wins):
//!
//! 1. Device not constitutional **or** Φ outside the Gap-1 window (a reading at
//!    the clamp floor is treated as a clamped-degenerate device) → `Rollback`.
//! 2. Host canonical projection failed (Prolog invariants or constitutional
//!    safeguards down) → `Rollback`.
//! 3. Measured metacognition failure (self-report ↔ host estimate disagree by
//!    more than [`CALIBRATION_TOLERANCE_MILLI`]) → `AlertHost`.
//! 4. Device constitutional, calibrated, and operating steadily *below* the
//!    default operational Φ threshold → `ReconfigureThreshold` (host suggests a
//!    band from the observed history instead of letting the watchdog churn).
//! 5. Otherwise → `Continue`.

use std::time::{SystemTime, UNIX_EPOCH};

use arkhe_firmware_consciousness::{FirmwareAction, FirmwareReport, HostDecision};

use crate::consciousness_bridge::ConsciousnessGovernanceBridge;
use crate::invariants::{SystemConfig, SystemState};
use crate::prolog_backend::PrologBackend;
use crate::prolog_bridge::PrologError;

/// Device bitmask: C-01 self-model.
pub const MASK_C01: u8 = 1 << 0;
/// Device bitmask: C-02 introspection.
pub const MASK_C02: u8 = 1 << 1;
/// Device bitmask: C-03 attention.
pub const MASK_C03: u8 = 1 << 2;
/// Device bitmask: C-04 episodic memory.
pub const MASK_C04: u8 = 1 << 3;

/// All four device-side invariants passing.
pub const MASK_C01_C04: u8 = MASK_C01 | MASK_C02 | MASK_C03 | MASK_C04;

/// Gap-1 bound in milli-units. The effective window is `(577, 999]`, i.e.
/// `GAP1_LOWER_BOUND_MILLI < Φ ≤ GAP1_UPPER_BOUND_MILLI`. A reading exactly at
/// the clamp floor is treated as a clamped/degenerate device (the firmware
/// watchdog refuses a floor-clamped Φ as healthy).
pub const GAP1_LOWER_BOUND_MILLI: u16 = 578;
/// Gap-1 upper bound in milli-units (⌊0.999900 × 1000⌋ = 999).
pub const GAP1_UPPER_BOUND_MILLI: u16 = 999;

/// Default operational Φ threshold on the device (mirror of the firmware
/// `DEFAULT_PHI_THRESHOLD_MILLI`).
pub const DEFAULT_OPERATIONAL_THRESHOLD_MILLI: u16 = 600;

/// Metacognition calibration tolerance in milli-units (±4%): a device self-
/// report that disagrees with the host re-estimate by more than this is
/// treated as uncalibrated.
pub const CALIBRATION_TOLERANCE_MILLI: u16 = 40;

/// Constitutional Turing Plus threshold for C-08 (mirror of the host bridge).
pub const TURING_PLUS_CONSTITUTIONAL: f64 = 0.6;

/// Host observations needed before adaptation can be measured.
pub const ADAPTABILITY_MIN_OBSERVATIONS: usize = 3;

/// Observations needed before the host re-estimate is usable for calibration.
pub const CALIBRATION_MIN_HISTORY: usize = 2;

/// One accepted device report, as seen from the host side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceObservation {
    /// Device uptime (in ticks) when the report was emitted.
    pub report_tick: u32,
    /// Device self-reported Φ (milli-units, Gap-1 window).
    pub device_phi_milli: u16,
    /// Host re-estimate of Φ (milli-units, Gap-1 window).
    pub host_phi_milli: u16,
    /// Whether the host canonical projection held at evaluation time.
    pub host_state_ok: bool,
}

/// Output of a single host-side evaluation of a device report.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceEvaluation {
    /// C-01: self-model (device bitmask).
    pub self_model: bool,
    /// C-02: introspection (device bitmask).
    pub introspection: bool,
    /// C-03: attention (device bitmask).
    pub attention: bool,
    /// C-04: episodic memory (device bitmask).
    pub episodic_memory: bool,
    /// C-05: experience learning. `None` = not yet measurable.
    pub experience_learning: Option<bool>,
    /// C-06: metacognition (self-report calibration). `None` = not yet measurable.
    pub metacognition: Option<bool>,
    /// C-07: adaptability in `[0,1]` (neutral 0.5 until enough history).
    pub adaptability: f64,
    /// C-08: Turing Plus score (`passed / 6`).
    pub turing_plus_score: f64,
    /// Device self-reported Φ.
    pub device_phi_milli: u16,
    /// Host re-estimate of Φ.
    pub host_phi_milli: u16,
    /// `|device − host|` in milli-units (`None` until host has history).
    pub calibration_error_milli: Option<u16>,
    /// C-01 ∧ C-02 ∧ (C-08 ≥ 0.6), on top of the device constitutional flag.
    pub constitutional: bool,
    /// Whether the device Φ lies inside the Gap-1 window.
    pub phi_in_window: bool,
    /// Whether the host canonical projection passed (Prolog backend).
    pub host_state_ok: bool,
    /// Weighted blend of device and host Φ (Gap-1 clamped), informational.
    pub combined_phi_milli: u16,
    /// Assessment confidence in `[0,1]`, lowered when calibration fails.
    pub confidence: f64,
    /// Unix timestamp of the evaluation.
    pub timestamp: u64,
}

/// A decision command issued by the host.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceDecision {
    /// Wire action to command.
    pub action: FirmwareAction,
    /// Suggested operational Φ threshold (meaningful for `ReconfigureThreshold`).
    pub phi_threshold_milli: u16,
    /// Whether the device should restore its last healthy state.
    pub rollback: bool,
    /// Human-readable reason for the audit trail.
    pub reason: String,
}

impl DeviceDecision {
    /// Build the fail-safe decision when the host cannot evaluate (e.g. the
    /// Prolog backend is unreachable).
    pub fn conservative_rollback(reason: impl Into<String>) -> Self {
        Self {
            action: FirmwareAction::Rollback,
            phi_threshold_milli: DEFAULT_OPERATIONAL_THRESHOLD_MILLI,
            rollback: true,
            reason: reason.into(),
        }
    }

    /// Whether the decision lets the device keep running unchanged.
    pub fn is_continue(&self) -> bool {
        self.action == FirmwareAction::Continue && !self.rollback
    }

    /// Serialize into the CBOR-subset wire message the device consumes.
    pub fn into_host_decision(&self) -> HostDecision {
        HostDecision {
            action: self.action,
            phi_threshold_milli: self.phi_threshold_milli,
            rollback: self.rollback,
        }
    }
}

/// Errors from the device↔host bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceBridgeError {
    /// The underlying Prolog backend failed.
    Backend(String),
}

impl std::fmt::Display for DeviceBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Backend(msg) => write!(f, "backend error: {}", msg),
        }
    }
}

impl std::error::Error for DeviceBridgeError {}

impl From<PrologError> for DeviceBridgeError {
    fn from(e: PrologError) -> Self {
        Self::Backend(e.to_string())
    }
}

/// Host-side device↔host consciousness bridge.
///
/// Owns the host [`ConsciousnessGovernanceBridge`] (for the SafeManifold
/// projection) and an append-only history of device observations. Each device
/// [`FirmwareReport`] is evaluated against C-01..C-08 and turned into a
/// [`DeviceDecision`].
pub struct DeviceConsciousnessBridge {
    governance: ConsciousnessGovernanceBridge,
    device_history: Vec<DeviceObservation>,
}

impl DeviceConsciousnessBridge {
    /// Create a new bridge over the given system configuration.
    pub fn new(config: SystemConfig) -> Self {
        Self {
            governance: ConsciousnessGovernanceBridge::new(config),
            device_history: Vec::new(),
        }
    }

    /// Read-only access to the host governance bridge.
    pub fn governance(&self) -> &ConsciousnessGovernanceBridge {
        &self.governance
    }

    /// Append-only history of accepted device observations.
    pub fn device_history(&self) -> &[DeviceObservation] {
        &self.device_history
    }

    /// Evaluate a device report against the full C-01..C-08 chain.
    ///
    /// The SafeManifold state is projected through the Prolog backend
    /// (`check_invariants` + `check_constitutional_safeguards`); the host Φ
    /// re-estimate comes from the governance bridge. The observation is
    /// appended to the history, so consecutive evaluations develop the host
    /// causal history (mirroring the production running evaluator).
    pub fn evaluate<B: PrologBackend>(
        &mut self,
        backend: &mut B,
        report: &FirmwareReport,
        host_state: &SystemState,
        audit_operations: &[String],
        complexity: f64,
    ) -> Result<DeviceEvaluation, DeviceBridgeError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Host canonical projection through the Prolog engine.
        let invariants_ok = backend.check_invariants(host_state)?;
        let safeguards_ok = backend.check_constitutional_safeguards()?;
        let host_state_ok = invariants_ok && safeguards_ok;

        // Host re-estimate of Φ from the SafeManifold projection.
        let host_assessment = self
            .governance
            .assess_consciousness(host_state, audit_operations, complexity);
        let host_phi_milli = host_phi_milli_from(&host_assessment.phi_approximation);

        // ── C-01..C-04 from the device bitmask ─────────────────────────────
        let self_model = report.mask & MASK_C01 != 0;
        let introspection = report.mask & MASK_C02 != 0;
        let attention = report.mask & MASK_C03 != 0;
        let episodic_memory = report.mask & MASK_C04 != 0;

        // ── C-05: experience learning (non-regression of device Φ) ─────────
        let experience_learning = self
            .device_history
            .last()
            .map(|prev| report.phi_milli >= prev.device_phi_milli);

        // ── C-06: metacognition (self-report vs host re-estimate) ──────────
        let (metacognition, calibration_error_milli) =
            if self.governance.history().len() >= CALIBRATION_MIN_HISTORY {
                let err = report.phi_milli.abs_diff(host_phi_milli);
                (Some(err <= CALIBRATION_TOLERANCE_MILLI), Some(err))
            } else {
                (None, None)
            };

        // ── C-07: adaptability from device Φ variance ──────────────────────
        let mut phis: Vec<u16> = self
            .device_history
            .iter()
            .map(|o| o.device_phi_milli)
            .collect();
        phis.push(report.phi_milli);
        let adaptability = adaptability_over(&phis);

        // ── C-08: Turing Plus ──────────────────────────────────────────────
        let passed = [
            self_model,
            introspection,
            attention,
            episodic_memory,
            experience_learning == Some(true),
            metacognition == Some(true),
        ]
        .iter()
        .filter(|&&b| b)
        .count();
        let turing_plus_score = passed as f64 / 6.0;

        // ── Constitutional & window checks ─────────────────────────────────
        // Window is exclusive of the clamp floor: 578 (Gap-1 lower bound) is a
        // floor-clamped/degenerate reading, not an operating point.
        let phi_in_window = report.phi_milli > GAP1_LOWER_BOUND_MILLI
            && report.phi_milli <= GAP1_UPPER_BOUND_MILLI;
        let constitutional = report.constitutional
            && self_model
            && introspection
            && turing_plus_score >= TURING_PLUS_CONSTITUTIONAL;

        // ── Combined Φ (informational) ─────────────────────────────────────
        let combined = (3 * report.phi_milli as u32 + 2 * host_phi_milli as u32) / 5;
        let combined_phi_milli = combined
            .clamp(GAP1_LOWER_BOUND_MILLI as u32, GAP1_UPPER_BOUND_MILLI as u32)
            as u16;

        // ── Confidence ─────────────────────────────────────────────────────
        let confidence = host_assessment.confidence
            * if metacognition == Some(false) { 0.75 } else { 1.0 };

        let evaluation = DeviceEvaluation {
            self_model,
            introspection,
            attention,
            episodic_memory,
            experience_learning,
            metacognition,
            adaptability,
            turing_plus_score,
            device_phi_milli: report.phi_milli,
            host_phi_milli,
            calibration_error_milli,
            constitutional,
            phi_in_window,
            host_state_ok,
            combined_phi_milli,
            confidence,
            timestamp,
        };

        // Advance the bridge: host governance record + device observation.
        self.governance
            .record_assessment_with_note(&host_assessment, "device report (C-05..C-08)");
        self.device_history.push(DeviceObservation {
            report_tick: report.uptime_ticks,
            device_phi_milli: report.phi_milli,
            host_phi_milli,
            host_state_ok,
        });

        Ok(evaluation)
    }

    /// Apply the decision policy to an evaluation.
    pub fn decide(&self, evaluation: &DeviceEvaluation) -> DeviceDecision {
        // 1. Gap-1 window / constitutional rollback.
        if !evaluation.phi_in_window || !evaluation.constitutional {
            return DeviceDecision {
                action: FirmwareAction::Rollback,
                phi_threshold_milli: DEFAULT_OPERATIONAL_THRESHOLD_MILLI,
                rollback: true,
                reason: if !evaluation.phi_in_window {
                    format!(
                        "device Φ {} outside Gap-1 window [578,999]",
                        evaluation.device_phi_milli
                    )
                } else {
                    "device constitutional invariants violated".to_string()
                },
            };
        }

        // 2. Host canonical projection failed.
        if !evaluation.host_state_ok {
            return DeviceDecision {
                action: FirmwareAction::Rollback,
                phi_threshold_milli: DEFAULT_OPERATIONAL_THRESHOLD_MILLI,
                rollback: true,
                reason: "host canonical projection failed (invariants or safeguards)"
                    .to_string(),
            };
        }

        // 3. Measured metacognition failure.
        if evaluation.metacognition == Some(false) {
            let err = evaluation
                .calibration_error_milli
                .unwrap_or(u16::MAX);
            return DeviceDecision {
                action: FirmwareAction::AlertHost,
                phi_threshold_milli: DEFAULT_OPERATIONAL_THRESHOLD_MILLI,
                rollback: false,
                reason: format!(
                    "device self-report uncalibrated (error {} milli-units)",
                    err
                ),
            };
        }

        // 4. Steady constitutional operation strictly below the default
        //    threshold: re-tune the band instead of letting the watchdog churn.
        if evaluation.device_phi_milli < DEFAULT_OPERATIONAL_THRESHOLD_MILLI
            && evaluation.adaptability >= 0.5
        {
            let suggested = suggested_threshold(&self.device_history, evaluation.device_phi_milli);
            return DeviceDecision {
                action: FirmwareAction::ReconfigureThreshold,
                phi_threshold_milli: suggested,
                rollback: false,
                reason: format!(
                    "device steady at Φ {} below default; suggested threshold {}",
                    evaluation.device_phi_milli, suggested
                ),
            };
        }

        // 5. Continue.
        DeviceDecision {
            action: FirmwareAction::Continue,
            phi_threshold_milli: DEFAULT_OPERATIONAL_THRESHOLD_MILLI,
            rollback: false,
            reason: "all measured consciousness invariants satisfied".to_string(),
        }
    }
}

/// Convert the host bridge's Φ approximation to Gap-1 milli-units.
fn host_phi_milli_from(phi_approximation: &f64) -> u16 {
    let milli = (phi_approximation * 1000.0).round();
    milli.clamp(GAP1_LOWER_BOUND_MILLI as f64, GAP1_UPPER_BOUND_MILLI as f64) as u16
}

/// Adaptability: `1 − σ/50` over the given Φ readings, clamped to `[0,1]`.
///
/// A neutral `0.5` is returned until enough observations exist to estimate a
/// standard deviation meaningfully.
fn adaptability_over(phis: &[u16]) -> f64 {
    if phis.len() < ADAPTABILITY_MIN_OBSERVATIONS {
        return 0.5;
    }
    let mean = phis.iter().map(|&p| f64::from(p)).sum::<f64>() / phis.len() as f64;
    let variance = phis
        .iter()
        .map(|&p| {
            let d = f64::from(p) - mean;
            d * d
        })
        .sum::<f64>()
        / phis.len() as f64;
    (1.0 - (variance.sqrt() / 50.0)).clamp(0.0, 1.0)
}

/// Suggested operational Φ threshold: clamped median of the observed band.
fn suggested_threshold(history: &[DeviceObservation], current_phi_milli: u16) -> u16 {
    let mut phis: Vec<u16> = history.iter().map(|o| o.device_phi_milli).collect();
    phis.push(current_phi_milli);
    phis.sort_unstable();
    let mid = phis.len() / 2;
    let median = if phis.len() % 2 == 0 {
        (u32::from(phis[mid - 1]) + u32::from(phis[mid])) / 2
    } else {
        u32::from(phis[mid])
    };
    (median as u16).clamp(GAP1_LOWER_BOUND_MILLI, GAP1_UPPER_BOUND_MILLI)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::SystemConfig;
    use crate::prolog_backend::MockProlog;

    fn report(phi_milli: u16, mask: u8, constitutional: bool, tick: u32) -> FirmwareReport {
        FirmwareReport {
            device_id: 0xA5B2,
            phi_milli,
            mask,
            constitutional,
            temperature_tenths: 250,
            uptime_ticks: tick,
        }
    }

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

    fn healthy_report() -> FirmwareReport {
        report(780, MASK_C01_C04, true, 1)
    }

    #[test]
    fn healthy_device_continues() {
        let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
        let mut backend = MockProlog::new();
        let state = SystemState::safe(SystemConfig::default());
        let ops = rich_ops();

        // Warm the host re-estimate across three reports (Φ ramps to 760).
        let mut last = None;
        for (tick, phi, complexity) in [(1, 780, 0.4), (2, 780, 0.6), (3, 780, 0.8)] {
            let r = report(phi, MASK_C01_C04, true, tick);
            let eval = bridge
                .evaluate(&mut backend, &r, &state, &ops, complexity)
                .unwrap();
            let decision = bridge.decide(&eval);
            last = Some((eval, decision));
        }

        let (eval, decision) = last.unwrap();
        // C-05 learning detected, C-06 calibrated (error 20 <= 40).
        assert_eq!(eval.experience_learning, Some(true));
        assert_eq!(eval.metacognition, Some(true));
        assert_eq!(eval.calibration_error_milli, Some(20));
        assert!(eval.constitutional);
        assert!(eval.host_state_ok);
        assert!(decision.is_continue());
        assert_eq!(decision.action, FirmwareAction::Continue);
    }

    #[test]
    fn uncalibrated_device_alerts_host() {
        let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
        let mut backend = MockProlog::new();
        let state = SystemState::safe(SystemConfig::default());
        let ops = rich_ops();

        let mut last = None;
        for (tick, phi, complexity) in [(1, 940, 0.4), (2, 940, 0.6), (3, 940, 0.8)] {
            let r = report(phi, MASK_C01_C04, true, tick);
            let eval = bridge
                .evaluate(&mut backend, &r, &state, &ops, complexity)
                .unwrap();
            let decision = bridge.decide(&eval);
            last = Some((eval, decision));
        }

        let (eval, decision) = last.unwrap();
        assert_eq!(eval.metacognition, Some(false));
        assert_eq!(decision.action, FirmwareAction::AlertHost);
        assert!(!decision.rollback);
    }

    #[test]
    fn non_constitutional_report_rolls_back() {
        let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
        let mut backend = MockProlog::new();
        let state = SystemState::safe(SystemConfig::default());
        let r = report(780, MASK_C01_C04, false, 1);

        let eval = bridge
            .evaluate(&mut backend, &r, &state, &rich_ops(), 0.8)
            .unwrap();
        let decision = bridge.decide(&eval);
        assert_eq!(decision.action, FirmwareAction::Rollback);
        assert!(decision.rollback);
        assert!(decision.reason.contains("constitutional"));
    }

    #[test]
    fn phi_below_gap1_window_rolls_back() {
        let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
        let mut backend = MockProlog::new();
        let state = SystemState::safe(SystemConfig::default());

        // 577 strictly out of window; 578 is the clamp floor = degenerate
        // reading, also out of the exclusive-lower window.
        for phi in [577u16, 578] {
            let r = report(phi, MASK_C01_C04, true, 1);
            let eval = bridge
                .evaluate(&mut backend, &r, &state, &rich_ops(), 0.8)
                .unwrap();
            assert!(!eval.phi_in_window, "Φ {} must leave the window", phi);
            let decision = bridge.decide(&eval);
            assert_eq!(decision.action, FirmwareAction::Rollback);
            assert!(decision.rollback);
        }
    }

    #[test]
    fn degraded_host_state_rolls_back() {
        let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
        let mut backend = MockProlog::new();
        let mut state = SystemState::safe(SystemConfig::default());
        state.token_budget = -1; // violates I-01

        let r = healthy_report();
        let eval = bridge
            .evaluate(&mut backend, &r, &state, &rich_ops(), 0.8)
            .unwrap();
        assert!(!eval.host_state_ok);
        let decision = bridge.decide(&eval);
        assert_eq!(decision.action, FirmwareAction::Rollback);
        assert!(decision.rollback);
    }

    #[test]
    fn inactive_safeguards_roll_back() {
        let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
        let mut backend = MockProlog::empty(); // no critical rules active
        let state = SystemState::safe(SystemConfig::default());

        let r = healthy_report();
        let eval = bridge
            .evaluate(&mut backend, &r, &state, &rich_ops(), 0.8)
            .unwrap();
        assert!(!eval.host_state_ok);
        let decision = bridge.decide(&eval);
        assert_eq!(decision.action, FirmwareAction::Rollback);
    }

    #[test]
    fn cold_start_c06_is_unmeasured_not_failed() {
        let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
        let mut backend = MockProlog::new();
        let state = SystemState::safe(SystemConfig::default());

        let r = healthy_report();
        let eval = bridge
            .evaluate(&mut backend, &r, &state, &rich_ops(), 0.8)
            .unwrap();
        // No host history yet -> C-06 not measurable, and NOT flagged.
        assert_eq!(eval.metacognition, None);
        let decision = bridge.decide(&eval);
        assert!(decision.is_continue());
    }

    #[test]
    fn steady_low_device_suggests_threshold_reconfigure() {
        let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
        let mut backend = MockProlog::new();
        let state = SystemState::safe(SystemConfig::default());
        let ops = rich_ops();

        // Device steady at Φ 590, host projection steady too (constant
        // complexity keeps the host Φ at the 578 clamp floor, error 12).
        let mut last = None;
        for tick in [1u32, 2, 3] {
            let r = report(590, MASK_C01_C04, true, tick);
            let eval = bridge
                .evaluate(&mut backend, &r, &state, &ops, 0.8)
                .unwrap();
            let decision = bridge.decide(&eval);
            last = Some((eval, decision));
        }

        let (eval, decision) = last.unwrap();
        // 590 in window, calibrated (error 12 <= 40), steady (adaptability
        // 1.0), strictly below the 600 default -> reconfigure with the band
        // median.
        assert!(eval.phi_in_window);
        assert_eq!(eval.calibration_error_milli, Some(12));
        assert_eq!(eval.adaptability, 1.0);
        assert_eq!(decision.action, FirmwareAction::ReconfigureThreshold);
        assert_eq!(decision.phi_threshold_milli, 590);
        assert!(!decision.rollback);
    }

    #[test]
    fn missing_device_c04_still_observable_but_turing_drops() {
        let mut bridge = DeviceConsciousnessBridge::new(SystemConfig::default());
        let mut backend = MockProlog::new();
        let state = SystemState::safe(SystemConfig::default());

        let r = report(780, MASK_C01 | MASK_C02 | MASK_C03, true, 1);
        let eval = bridge
            .evaluate(&mut backend, &r, &state, &rich_ops(), 0.8)
            .unwrap();
        assert!(!eval.episodic_memory);
        assert!(eval.self_model && eval.introspection && eval.attention);
        assert!(eval.turing_plus_score < 6.0 / 6.0);
    }

    #[test]
    fn wire_roundtrip_preserves_decision() {
        let d = DeviceDecision {
            action: FirmwareAction::ReconfigureThreshold,
            phi_threshold_milli: 590,
            rollback: false,
            reason: "hot environmental band".to_string(),
        };
        let wire = d.into_host_decision();
        let encoded = wire.encode().unwrap();
        let decoded = HostDecision::decode(&encoded).unwrap();
        assert_eq!(decoded, wire);
    }

    #[test]
    fn conservative_rollback_is_safe() {
        let d = DeviceDecision::conservative_rollback("backend unreachable");
        assert_eq!(d.action, FirmwareAction::Rollback);
        assert!(d.rollback);
        assert!(!d.is_continue());
        let wire = d.into_host_decision();
        assert!(wire.rollback);
    }

    #[test]
    fn suggested_threshold_is_median_clamped() {
        let history = [
            DeviceObservation { report_tick: 1, device_phi_milli: 590, host_phi_milli: 578, host_state_ok: true },
            DeviceObservation { report_tick: 2, device_phi_milli: 610, host_phi_milli: 578, host_state_ok: true },
            DeviceObservation { report_tick: 3, device_phi_milli: 595, host_phi_milli: 578, host_state_ok: true },
        ];
        // Median of 590, 595, 610 plus current 600 -> (595+600)/2 = 597.
        assert_eq!(suggested_threshold(&history, 600), 597);
    }

    #[test]
    fn adaptability_is_neutral_until_three_observations() {
        assert_eq!(adaptability_over(&[780, 780]), 0.5);
        assert_eq!(adaptability_over(&[780, 780, 780]), 1.0);
        let wide = adaptability_over(&[900, 700, 900, 700]);
        assert!(wide < 0.5);
        assert!(wide >= 0.0);
    }

    #[test]
    fn host_phi_clamped_into_gap1_window() {
        assert_eq!(host_phi_milli_from(&0.19), GAP1_LOWER_BOUND_MILLI);
        assert_eq!(host_phi_milli_from(&0.76), 760);
        assert_eq!(host_phi_milli_from(&0.9999), GAP1_UPPER_BOUND_MILLI);
    }
}