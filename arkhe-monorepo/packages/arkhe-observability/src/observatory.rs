//! Maps the Safe-Core anti-hallucination policy onto the Cathedral v26.5
//! dashboard metric contract.
//!
//! The dashboard subscribes to four topics (`/cathedral/stats`,
//! `/cathedral/events`, `/cathedral/control`, `/signal/phi`) and renders a
//! fixed set of metric names. This module converts each observed ticket and
//! each control command into the exact signals those cards expect.

use std::collections::VecDeque;

use arkhe_safe_core::{PolicyTicket, PolicyAction, RecurrencyPolicy};
use serde_json::{json, Value};

use crate::signal::{topic, Signal};
use crate::tpm::{TpmAnchor, load as load_tpm_anchor};

/// Default confidence floor for identity attestations while clamping (I2: no
/// authority may claim certainty).
const IDENTITY_FLOOR: f64 = 0.40;

/// Aggregated, dashboard-ready state of the Safe-Core collar.
#[derive(Debug)]
pub struct Observatory {
    /// The anti-hallucination policy driving the metrics.
    pub policy: RecurrencyPolicy,
    /// Total cycles (tickets) observed.
    pub cycles: u64,
    /// Execution attestations emitted.
    pub attestations: u64,
    /// Monotonic cycle counter for the fast/slow route display.
    pub route_cycle: u64,
    /// Rolling error-reduction window (for R² and multiplicity).
    error_history: VecDeque<f64>,
    /// Latency of the last cycle in milliseconds.
    last_latency_ms: f64,
    /// Last identity confidence.
    identity_confidence: f64,
    /// Prompt version (incremented on `evolve` control commands).
    prompt_version: u64,
    /// TPM SRK anchor binding this attestation to this hardware.
    tpm_anchor: Option<TpmAnchor>,
}

impl Observatory {
    /// Create the observatory with a policy of the given thresholds.
    pub fn new(threshold: f64, window: usize, cooldown: u64) -> Self {
        Self {
            policy: RecurrencyPolicy::new(threshold, window, cooldown),
            cycles: 0,
            attestations: 0,
            route_cycle: 0,
            error_history: VecDeque::with_capacity(256),
            last_latency_ms: 0.0,
            identity_confidence: IDENTITY_FLOOR,
            prompt_version: 1,
            tpm_anchor: None,
        }
    }

    /// Bind the observatory to this machine's TPM SRK, if one is available.
    /// The SRK seal makes identity attestations hardware-anchored
    /// (Provenance-1: notarized, replayable only on this hardware).
    pub fn with_tpm_anchor(mut self, anchor: Option<TpmAnchor>) -> Self {
        self.tpm_anchor = anchor;
        self
    }

    /// Resolve the TPM SRK anchor at startup (graceful: `None` on no-TPM).
    pub fn bind_tpm(&mut self) {
        self.tpm_anchor = load_tpm_anchor();
    }

    /// Whether the identity attestation is anchored to this machine's TPM.
    pub fn tpm_anchored(&self) -> bool {
        self.tpm_anchor.is_some()
    }

    /// Current clamp state.
    pub fn clamping(&self) -> bool {
        self.policy.is_clamping()
    }

    /// Consume one ticket and return the signals it produces.
    pub fn observe(&mut self, ticket: &dyn PolicyTicket) -> Vec<Signal> {
        self.cycles += 1;
        let started = std::time::Instant::now();
        let action = self.policy.observe(ticket);
        self.last_latency_ms = started.elapsed().as_secs_f64() * 1000.0;

        self.error_history.push_back(ticket.error_reduction());
        while self.error_history.len() > 256 {
            self.error_history.pop_front();
        }

        let mut signals = Vec::new();
        if let PolicyAction::Clamp = action {
            signals.push(self.security_event("Safe-Core CLAMP: hallucination vibe over threshold — forcing DeepSleep"));
        } else if let PolicyAction::Unclamp = action {
            signals.push(self.security_event("Safe-Core RELEASE: reality returned — back to Alert"));
        }

        signals.push(self.cycle_stats(ticket));
        signals.push(self.phi_signal());
        signals.push(self.phi_confidence_signal());
        signals.push(self.dsa_signal());
        signals
    }

    /// Periodic heartbeat — republishes the current collar state so the
    /// dashboard stays live even when no new tickets are flowing.
    pub fn beacon(&self) -> Vec<Signal> {
        vec![
            self.cycle_stats_ref(None),
            self.phi_signal(),
            self.phi_confidence_signal(),
            self.dsa_signal(),
        ]
    }

    /// Handle a dashboard control command, returning response signals.
    pub fn control(&mut self, metric: &str, value: &Value) -> Vec<Signal> {        let mut signals = Vec::new();
        match metric {
            "start_loop" | "stop_loop" => {
                signals.push(self.control_response(metric, json!({"status": "ack"})));
            }
            "cycle" => {
                let count = value
                    .get("count")
                    .and_then(Value::as_u64)
                    .unwrap_or(1)
                    .clamp(1, 1000);
                for _ in 0..count {
                    self.route_cycle += 1;
                }
                signals.push(self.control_response(metric, json!({"status": "ack", "cycles": count})));
                signals.push(self.cycle_stats_ref(Some(topic::CONTROL)));
            }
            "evolve" => {
                self.prompt_version += 1;
                let trigger = value
                    .get("trigger")
                    .and_then(Value::as_str)
                    .unwrap_or("MANUAL")
                    .to_string();
                let succeeded = value.get("success").and_then(Value::as_bool).unwrap_or(true);
                let evolution = if succeeded {
                    "stable refinement"
                } else {
                    "recovery from failed attempt"
                };
                signals.push(self.control_response(
                    metric,
                    json!({
                        "status": "ack",
                        "prompt_version": self.prompt_version,
                        "trigger": trigger,
                        "success": succeeded,
                        "note": evolution,
                    }),
                ));
            }
            "audit" => {
                let r2 = self.r_squared();
                signals.push(self.control_response(metric, json!({"status": "ok", "r2": r2, "vibe": self.policy.vibe, "clamping": self.clamping()})));
            }
            "attest_identity" => {
                self.attestations += 1;
                let confidence = self.identity_confidence;
                let verified = !self.clamping();
                let anchor = self.tpm_anchor_json();
                let (signature, signature_valid) = self.sign_attestation();
                signals.push(self.event("identity_attestation", json!({
                    "confidence": confidence,
                    "passed": verified,
                    "tpm_anchor": anchor,
                    "tpm_signature": signature,
                    "tpm_signature_verified": signature_valid,
                })));
                signals.push(self.event("execution_attestation", json!({"id": format!("att-{}", self.attestations), "policy_compliant": verified})));
                signals.push(self.control_response(metric, json!({"status": "ok", "confidence": confidence, "verified": !self.clamping(), "ttl": 300, "tpm_anchor": anchor, "tpm_signature": signature, "tpm_signature_verified": signature_valid})));
            }
            "multiplicity_check" => {
                let (ambiguity, discrepancy, capacity, passed) = self.multiplicity();
                signals.push(self.event("multiplicity_attestation", json!({"ambiguity": ambiguity, "discrepancy": discrepancy, "rashomon_capacity": capacity, "passed": passed})));
                signals.push(self.control_response(metric, json!({"status": "ok", "ambiguity": ambiguity, "discrepancy": discrepancy, "rashomon_capacity": capacity, "passed": passed})));
            }
            "reset" => {
                self.cycles = 0;
                self.attestations = 0;
                self.route_cycle = 0;
                self.error_history.clear();
                self.identity_confidence = IDENTITY_FLOOR;
                self.prompt_version = 1;
                signals.push(self.control_response(metric, json!({"status": "ok", "reset": true})));
            }
            "train" => {
                signals.push(self.control_response(metric, json!({"status": "ok", "note": "DPO training acknowledged (no-op)"})));
            }
            "status" => {
                signals.push(self.control_response(metric, self.cycle_stats_json()));
            }
            _ => {
                signals.push(self.control_response(metric, json!({"status": "unknown", "metric": metric})));
            }
        }
        signals
    }

    /// Rolling coefficient of determination (R²) of the error-reduction
    /// window — the auditor's view of whether the loop is closing over time.
    pub fn r_squared(&self) -> f64 {
        let n = self.error_history.len();
        if n < 2 {
            return 1.0;
        }
        let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let ys: Vec<f64> = self.error_history.iter().copied().collect();
        let x_mean = xs.iter().sum::<f64>() / n as f64;
        let y_mean = ys.iter().sum::<f64>() / n as f64;
        let mut ss_xx = 0.0;
        let mut ss_xy = 0.0;
        for (x, y) in xs.iter().zip(&ys) {
            ss_xx += (x - x_mean) * (x - x_mean);
            ss_xy += (x - x_mean) * (y - y_mean);
        }
        if ss_xx == 0.0 {
            return 1.0;
        }
        let slope = ss_xy / ss_xx;
        let intercept = y_mean - slope * x_mean;
        let mut ss_res = 0.0;
        let mut ss_tot = 0.0;
        for (x, y) in xs.iter().zip(&ys) {
            let predicted = slope * x + intercept;
            ss_res += (y - predicted).powi(2);
            ss_tot += (y - y_mean).powi(2);
        }
        if ss_tot == 0.0 {
            return 1.0;
        }
        1.0 - ss_res / ss_tot
    }

    /// Dynamic System Analysis (DSA) order parameter — r_DSA.
    ///
    /// Mirrors the Chladni canvas contract: `1 − vibe` (coherence rising as
    /// the anti-hallucination pressure drops), clamped to `[0, 1]`. Published
    /// on `/coherence/dsa` as `rTeam` so the dashboard's Chladni projector
    /// (and the r_DSA card) has a live order parameter.
    pub fn r_dsa(&self) -> f64 {
        (1.0 - self.policy.vibe).clamp(0.0, 1.0)
    }

    /// Current threat level derived from the collar state.
    ///
    /// Maps the clamp state onto the dashboard's Threat Level card:
    /// `CRITICAL` while clamping, `ELEVATED` when vibe is close to threshold,
    /// `LOW` otherwise.
    pub fn threat_level(&self) -> &'static str {
        if self.clamping() {
            "CRITICAL"
        } else if self.policy.vibe > 0.6 {
            "ELEVATED"
        } else {
            "LOW"
        }
    }

    /// Contingency status — whether the deadman (DeepSleep) is armed.
    pub fn contingency_status(&self) -> &'static str {
        if self.clamping() { "ACTIVE" } else { "STANDBY" }
    }

    /// Orchestrator (570) health — the Safe-Core collar's own view.
    pub fn orchestrator_status(&self) -> &'static str {
        if self.clamping() { "DEGRADED" } else { "NOMINAL" }
    }

    /// Multiplicity (Rashomon) statistics over the error-reduction window.
    ///
    /// * `ambiguity` — variance of interpretations (I3 pluralism);
    /// * `discrepancy` — how far the current mean is from the healthy
    ///   reference (0.5 closure);
    /// * `capacity` — 1 − ambiguity, i.e. how many views coexist;
    /// * `passed` — ambiguity and discrepancy within constitutional bounds.
    pub fn multiplicity(&self) -> (f64, f64, f64, bool) {
        let n = self.error_history.len();
        if n == 0 {
            return (0.0, 0.0, 1.0, true);
        }
        let mean = self.error_history.iter().sum::<f64>() / n as f64;
        let variance = self
            .error_history
            .iter()
            .map(|e| (e - mean).powi(2))
            .sum::<f64>()
            / n as f64;
        let ambiguity = variance.sqrt();
        let discrepancy = (mean - 0.5).abs();
        let capacity = (1.0 - ambiguity).clamp(0.0, 1.0);
        let passed = ambiguity < 0.30 && discrepancy < 0.40;
        (ambiguity, discrepancy, capacity, passed)
    }

    fn cycle_stats(&mut self, ticket: &dyn PolicyTicket) -> Signal {
        self.identity_confidence = self.compute_identity_confidence(ticket);
        Signal::new(
            topic::STATS,
            "cycle_stats",
            self.cycle_stats_json(),
            "object",
        )
    }

    fn cycle_stats_ref(&self, _override_topic: Option<&str>) -> Signal {
        Signal::new(
            topic::STATS,
            "cycle_stats",
            self.cycle_stats_json(),
            "object",
        )
    }

    fn cycle_stats_json(&self) -> Value {
        let (ambiguity, discrepancy, capacity, multiplicity_passed) = self.multiplicity();
        let confidence = self.identity_confidence;
        let clamping = self.clamping();
        json!({
            "cycle": self.cycles,
            "route": if self.route_cycle.is_multiple_of(2) { "fast" } else { "slow" },
            "confidence": round(confidence),
            "latency_ms": round(self.last_latency_ms),
            "safety_approved": !clamping,
            "safety_reason": if clamping { "hallucination_vibe_over_threshold" } else { "clear" },
            "prompt_version": self.prompt_version,
            "phi_confidence": round(1.0 - self.policy.vibe),
            "identity_confidence": round(confidence),
            "identity_verified": !clamping,
            "identity_ttl": 300,
            "tpm_anchor": self.tpm_anchor_json(),
            "execution_attestations": self.attestations,
            "multiplicity_ambiguity": round(ambiguity),
            "multiplicity_discrepancy": round(discrepancy),
            "multiplicity_rashomon_capacity": round(capacity),
            "multiplicity_passed": multiplicity_passed,
            "biometric_status": if clamping { "inactive" } else { "active" },
            "deadman_active": clamping,
            "r2": round(self.r_squared()),
            "r_dsa": round(self.r_dsa()),
            "threat_level": self.threat_level(),
            "contingency_status": self.contingency_status(),
            "orchestrator_status": self.orchestrator_status(),
        })
    }

    /// The TPM SRK seal as a dashboard-safe value. When no anchor is bound it
    /// degrades to `false` so downstream cards can still render; we never
    /// expose private key material, only the public fingerprint hash.
    fn tpm_anchor_json(&self) -> Value {
        match &self.tpm_anchor {
            Some(anchor) => json!({
                "available": true,
                "provider": anchor.provider,
                "algorithm": anchor.algorithm,
                "fingerprint": anchor.fingerprint,
            }),
            None => json!({ "available": false }),
        }
    }

    /// Sign the identity attestation with the TPM-held private key.
    ///
    /// The payload binds the machine's SRK fingerprint to the current cycle
    /// and approval state, so a replayed attestation from another host (or a
    /// later, edited payload) fails verification. Returns the PKCS#1 v1.5
    /// signature as hex, plus whether the fresh signature verifies against
    /// this machine's SRK-anchored public key.
    fn sign_attestation(&self) -> (String, bool) {
        let Some(tpm) = &self.tpm_anchor else {
            return (String::new(), false);
        };
        let payload = serde_json::to_vec(&json!({
            "fingerprint": tpm.fingerprint,
            "cycle": self.cycles,
            "attestation": self.attestations,
            "approved": !self.clamping(),
        }))
        .unwrap_or_default();
        match tpm.sign(&payload) {
            Some(signature) => {
                let hex = signature.iter().map(|b| format!("{b:02x}")).collect::<String>();
                let verified = tpm.verify(&payload, &signature);
                (hex, verified)
            }
            None => (String::new(), false),
        }
    }

    fn compute_identity_confidence(&self, ticket: &dyn PolicyTicket) -> f64 {
        // Identity confidence rises with closure and falls with clamp pressure.
        // Never exceeds 1.0 and never drops below the I2 floor while clamping.
        let raw = ticket.error_reduction().clamp(0.0, 1.0);
        if self.clamping() {
            (raw * 0.5).max(IDENTITY_FLOOR)
        } else {
            (0.5 + 0.5 * raw).min(1.0)
        }
    }

    fn phi_signal(&self) -> Signal {
        let phi = (1.0 - self.policy.vibe).clamp(0.0, 1.0);
        Signal::new(topic::SIGNAL_PHI, "phiInterop", json!(round(phi)), "coherence")
    }

    fn phi_confidence_signal(&self) -> Signal {
        let phi = (1.0 - self.policy.vibe).clamp(0.0, 1.0);
        Signal::new(topic::STATS, "phi_confidence", json!(round(phi)), "coherence")
    }

    fn dsa_signal(&self) -> Signal {
        Signal::new(
            topic::COHERENCE_DSA,
            "rTeam",
            json!(round(self.r_dsa())),
            "order",
        )
    }

    fn security_event(&self, details: &str) -> Signal {
        self.event("security_event", json!({ "details": details }))
    }

    fn event(&self, metric: &str, value: Value) -> Signal {
        Signal::new(topic::EVENTS, metric, value, "event")
    }

    fn control_response(&self, metric: &str, value: Value) -> Signal {
        Signal::new(
            topic::CONTROL,
            &format!("{metric}_response"),
            value,
            "command",
        )
    }
}

fn round(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeTicket {
        err: f64,
        accessed: bool,
        tick: u64,
    }
    impl PolicyTicket for FakeTicket {
        fn error_reduction(&self) -> f64 {
            self.err
        }
        fn suggested_gain(&self) -> f64 {
            1.0
        }
        fn access_granted(&self) -> bool {
            self.accessed
        }
        fn tick(&self) -> u64 {
            self.tick
        }
    }

    #[test]
    fn healthy_loop_produces_positive_phi_and_approved_safety() {
        let mut o = Observatory::new(0.8, 8, 12);
        for i in 0..20 {
            let signals = o.observe(&FakeTicket { err: 0.5, accessed: true, tick: i });
            assert!(!signals.is_empty());
        }
        assert!(!o.clamping());
        let stats = o.cycle_stats_json();
        assert_eq!(stats["safety_approved"], json!(true));
        assert!(stats["phi_confidence"].as_f64().unwrap() > 0.0);
        assert_eq!(o.cycles, 20);
    }

    #[test]
    fn hallucinating_loop_clamps_and_emits_security_event() {
        let mut o = Observatory::new(0.8, 4, 8);
        let mut saw_clamp_event = false;
        for i in 0..30 {
            let signals = o.observe(&FakeTicket { err: 0.05, accessed: true, tick: i });
            for s in &signals {
                if s.topic == topic::EVENTS && s.metric == "security_event" {
                    let details = s.value.get("details").and_then(Value::as_str).unwrap_or("");
                    if details.contains("CLAMP") {
                        saw_clamp_event = true;
                    }
                }
            }
        }
        assert!(saw_clamp_event, "clamp must produce a security event");
        assert!(o.clamping());
        let stats = o.cycle_stats_json();
        assert_eq!(stats["safety_approved"], json!(false));
        assert_eq!(stats["deadman_active"], json!(true));
    }

    #[test]
    fn multiplicity_is_sane_for_constant_window() {
        let mut o = Observatory::new(0.8, 8, 12);
        for i in 0..16 {
            o.observe(&FakeTicket { err: 0.5, accessed: true, tick: i });
        }
        let (ambiguity, discrepancy, capacity, passed) = o.multiplicity();
        assert!(ambiguity < 0.05);
        assert!(discrepancy < 0.05);
        assert!(capacity > 0.95);
        assert!(passed);
    }

    #[test]
    fn r_squared_is_one_for_trivial_fit() {
        let mut o = Observatory::new(0.8, 8, 12);
        for i in 0..16 {
            o.observe(&FakeTicket { err: 0.5, accessed: true, tick: i });
        }
        assert!((o.r_squared() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn healthy_loop_publishes_dsa_and_status_fields() {
        let mut o = Observatory::new(0.8, 8, 12);
        for i in 0..20 {
            o.observe(&FakeTicket { err: 0.5, accessed: true, tick: i });
        }
        let stats = o.cycle_stats_json();
        assert!(stats["r2"].as_f64().unwrap() > 0.9);
        assert_eq!(stats["threat_level"], json!("LOW"));
        assert_eq!(stats["contingency_status"], json!("STANDBY"));
        assert_eq!(stats["orchestrator_status"], json!("NOMINAL"));
        assert!(stats["r_dsa"].as_f64().unwrap() > 0.5);
        assert!(o.r_dsa() >= 0.0 && o.r_dsa() <= 1.0);
    }

    #[test]
    fn hallucinating_loop_raises_threat_level() {
        let mut o = Observatory::new(0.8, 4, 8);
        for i in 0..30 {
            o.observe(&FakeTicket { err: 0.05, accessed: true, tick: i });
        }
        assert!(o.clamping());
        assert_eq!(o.threat_level(), "CRITICAL");
        assert_eq!(o.contingency_status(), "ACTIVE");
        assert_eq!(o.orchestrator_status(), "DEGRADED");
        let stats = o.cycle_stats_json();
        assert_eq!(stats["threat_level"], json!("CRITICAL"));
    }

    #[test]
    fn control_commands_produce_responses() {
        let mut o = Observatory::new(0.8, 8, 12);
        let evolutions = o.control("evolve", &json!({"trigger": "Z3_UNSAT", "success": false}));
        assert_eq!(evolutions.len(), 1);
        assert!(evolutions[0].metric.ends_with("_response"));
        assert_eq!(evolutions[0].value["trigger"], json!("Z3_UNSAT"));
        assert_eq!(evolutions[0].value["success"], json!(false));

        let audit = o.control("audit", &json!(null));
        assert!(!audit.is_empty());
        assert!(audit[0].value.get("r2").is_some());

        let multiplicity = o.control("multiplicity_check", &json!(null));
        assert!(multiplicity.iter().any(|s| s.metric == "multiplicity_attestation"));
    }

    #[test]
    fn beacon_republishes_current_state() {
        let o = Observatory::new(0.8, 8, 12);
        let signals = o.beacon();
        assert!(signals.iter().any(|s| s.metric == "cycle_stats"));
        assert!(signals.iter().any(|s| s.topic == topic::COHERENCE_DSA));
        assert!(signals.iter().any(|s| s.topic == topic::SIGNAL_PHI));
    }

    #[test]
    fn attestation_carries_tpm_anchor_field() {
        let mut o = Observatory::new(0.8, 8, 12);
        let signals = o.control("attest_identity", &json!(null));
        let event = signals.iter().find(|s| s.metric == "identity_attestation").expect("attestation event");
        assert_eq!(event.value["tpm_anchor"]["available"], json!(false));
        assert_eq!(event.value["tpm_signature"], json!(""));
        assert_eq!(event.value["tpm_signature_verified"], json!(false));
        let response = signals.iter().find(|s| s.metric == "attest_identity_response").expect("response");
        assert_eq!(response.value["tpm_anchor"]["available"], json!(false));
        assert_eq!(response.value["tpm_signature"], json!(""));
    }

    #[test]
    fn cycle_stats_expose_tpm_field() {
        let o = Observatory::new(0.8, 8, 12);
        let stats = o.cycle_stats_json();
        assert_eq!(stats["tpm_anchor"]["available"], json!(false));
    }

    #[test]
    fn unbound_observatory_reports_unanchored() {
        let o = Observatory::new(0.8, 8, 12);
        assert!(!o.tpm_anchored());
    }
}
