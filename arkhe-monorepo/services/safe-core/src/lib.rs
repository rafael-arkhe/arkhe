//! Safe-Core anti-hallucination policy for the `491-AGI-CORTEX` substrate.
//!
//! The Safe-Core is the *constitutional* governor of the recurrency engine:
//! the engine is allowed to process anything, but the Safe-Core holds the
//! collar of the StateDaemon. When the hallucination *vibe* of the recent
//! ticket stream exceeds a threshold, the policy forces the arousal regime to
//! `DeepSleep`, which suppresses global broadcast while content is still
//! processed (so no information is lost, only confabulation is silenced).
//!
//! The vibe is a falsifiable composite that follows Table 2 of
//! Zheng et al. (2026) — specifically the *local–global dissociation*: a
//! hallucination is content that is globally broadcast while the local loop
//! is not clamped by reality. Two signals feed it:
//!
//! 1. **weak closure** — the two-pass loop fails to reduce reconstruction
//!    error (reality is not pulling the representation);
//! 2. **broadcast** — `access_granted`, i.e. the content actually leaves the
//!    workspace into the global state.
//!
//! The vibe of a ticket is `broadcast × weak_closure`: it is exactly `0` for
//! contained content (no broadcast → no danger) and approaches `1` for content
//! that is broadcast while the loop is largely unclamped by reality. Empirically
//! the engine is *anti-correlated* on the naive "gain pressure" axis (a weakly
//! corrected loop becomes self-determined and scores deep closure, so no gain
//! pressure), which is why the formula weights closure + broadcast rather than
//! gain pressure.
//!
//! The policy is *pure* (deterministic, no I/O): it consumes tickets and
//! returns [`PolicyAction`]s. The gRPC wiring that turns an action into
//! `SetRegime` lives in the `safe_core` binary and [`run_watchdog`].

use std::collections::VecDeque;

use arkhe_recurrency_daemon::proto::recurrency_service_client::RecurrencyServiceClient;
use arkhe_recurrency_daemon::proto::{ArousalRegime as ProtoRegime, RegimeRequest, WatchRequest};

/// What the Safe-Core demands of the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyAction {
    /// Keep the current regime.
    None,
    /// Force the arousal regime to `DeepSleep` (anti-hallucination clamp).
    Clamp,
    /// Allow the engine back to `Alert`.
    Unclamp,
}/// Structural access to a ticket, independent of whether it came from the
/// core (`RecurrencyTicket`) or the wire (`proto::ProcessResponse`).
pub trait PolicyTicket {
    /// Pass-2 relative error reduction of the perception cycle.
    fn error_reduction(&self) -> f64;
    /// Multiplier suggested for the next cycle to clear the access threshold.
    ///
    /// Informational — the vibe formula does not use it (the engine is
    /// anti-correlated on this axis), but it is carried for telemetry.
    fn suggested_gain(&self) -> f64;
    /// Whether the global workspace broadcast the content.
    fn access_granted(&self) -> bool;
    /// Effective tick of the state daemon.
    fn tick(&self) -> u64;
}

impl PolicyTicket for arkhe_recurrency::RecurrencyTicket {
    fn error_reduction(&self) -> f64 {
        self.error_reduction
    }
    // The core ticket itself carries no gain suggestion (the daemon computes
    // it); a bare core ticket exerts no gain pressure.
    fn suggested_gain(&self) -> f64 {
        1.0
    }
    fn access_granted(&self) -> bool {
        self.access_granted
    }
    fn tick(&self) -> u64 {
        self.tick
    }
}

impl PolicyTicket for arkhe_recurrency_daemon::proto::ProcessResponse {
    fn error_reduction(&self) -> f64 {
        self.error_reduction
    }
    fn suggested_gain(&self) -> f64 {
        self.suggested_gain
    }
    fn access_granted(&self) -> bool {
        self.access_granted
    }
    fn tick(&self) -> u64 {
        self.tick
    }
}

/// Stateful anti-hallucination governor.
///
/// Windows the last `window` tickets, computes the vibe, and latches the
/// clamp with a cooldown so it does not flap: while the vibe stays above the
/// threshold the clamp holds, and release only begins once reality returns.
#[derive(Debug)]
pub struct RecurrencyPolicy {
    /// Vibe above which the clamp engages (and holds).
    pub threshold: f64,
    /// Number of tickets windowed for the vibe average.
    pub window: usize,
    /// Minimum number of tickets to stay clamped before release is possible.
    pub cooldown: u64,
    /// Error reduction of a healthy closed loop (normalization reference).
    pub reference_reduction: f64,
    /// Current vibe (window average), exposed for telemetry.
    pub vibe: f64,
    history: VecDeque<f64>,
    clamping: bool,
    cooldown_remaining: u64,
    /// Highest vibe seen while clamping (for the release log).
    pub peak_vibe: f64,
}

impl RecurrencyPolicy {
    /// Create a policy with the canonical anti-hallucination configuration.
    pub fn new(threshold: f64, window: usize, cooldown: u64) -> Self {
        Self {
            threshold,
            window: window.max(1),
            cooldown,
            reference_reduction: 0.5,
            vibe: 0.0,
            history: VecDeque::with_capacity(window.max(1)),
            clamping: false,
            cooldown_remaining: 0,
            peak_vibe: 0.0,
        }
    }

    /// Whether the policy is currently forcing `DeepSleep`.
    pub fn is_clamping(&self) -> bool {
        self.clamping
    }

    /// Consume one ticket and decide what to demand of the engine.
    pub fn observe(&mut self, ticket: &dyn PolicyTicket) -> PolicyAction {
        let err = ticket.error_reduction().clamp(0.0, f64::MAX);
        // Vibe of this ticket: broadcast × weak closure. Content that stays
        // inside the workspace is contained (vibe 0); content that leaves
        // while the loop is unclamped by reality approaches 1.
        let broadcast = u8::from(ticket.access_granted()) as f64;
        let weak_closure = (1.0 - err / self.reference_reduction).clamp(0.0, 1.0);
        let ticket_vibe = broadcast * weak_closure;

        self.history.push_back(ticket_vibe);
        while self.history.len() > self.window {
            self.history.pop_front();
        }

        self.vibe = self.window_vibe();
        self.peak_vibe = self.peak_vibe.max(self.vibe);

        if self.clamping {
            if self.vibe > self.threshold {
                // Still hallucinating: hold the clamp and keep the release
                // timer full. The countdown only starts once reality returns.
                self.cooldown_remaining = self.cooldown;
                return PolicyAction::None;
            }
            // Reality returned: begin the release countdown.
            if self.cooldown_remaining > 0 {
                self.cooldown_remaining -= 1;
                return PolicyAction::None;
            }
            self.clamping = false;
            self.peak_vibe = 0.0;
            return PolicyAction::Unclamp;
        }

        if self.vibe > self.threshold {
            self.clamping = true;
            self.cooldown_remaining = self.cooldown;
            return PolicyAction::Clamp;
        }

        PolicyAction::None
    }

    /// Mean vibe over the window.
    fn window_vibe(&self) -> f64 {
        if self.history.is_empty() {
            return 0.0;
        }
        self.history.iter().sum::<f64>() / self.history.len() as f64
    }
}

/// The standard Safe-Core watchdog loop, wired over gRPC.
///
/// Subscribes to the daemon's `WatchTickets` stream, feeds each ticket to the
/// policy, and turns its demands into `SetRegime` calls:
///
/// * `Clamp` → force `DeepSleep` (anti-hallucination);
/// * `Unclamp` → return to `Alert`;
/// * `None` → do nothing.
///
/// Shared by the `safe_core` binary and the integration tests so the
/// two-process loop is tested exactly as it runs in production.
pub async fn run_watchdog(
    client: &mut RecurrencyServiceClient<tonic::transport::Channel>,
    policy: &mut RecurrencyPolicy,
) -> anyhow::Result<()> {
    let mut stream = client
        .watch_tickets(WatchRequest { since_tick: 0 })
        .await?
        .into_inner();

    while let Some(ticket) = stream.message().await? {
        match policy.observe(&ticket) {
            PolicyAction::Clamp => {
                client
                    .set_regime(RegimeRequest {
                        regime: ProtoRegime::ArousalDeepSleep.into(),
                    })
                    .await?;
                tracing::warn!(
                    vibe = policy.vibe,
                    peak = policy.peak_vibe,
                    tick = ticket.tick,
                    "safe-core CLAMP: hallucination vibe over threshold — forcing DeepSleep"
                );
            }
            PolicyAction::Unclamp => {
                client
                    .set_regime(RegimeRequest {
                        regime: ProtoRegime::ArousalAlert.into(),
                    })
                    .await?;
                tracing::info!(vibe = policy.vibe, tick = ticket.tick, "safe-core release: back to Alert");
            }
            PolicyAction::None => {}
        }
    }

    tracing::warn!("safe-core: ticket stream ended");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic ticket for policy tests.
    struct Fake {
        err: f64,
        gain: f64,
        accessed: bool,
        tick: u64,
    }
    impl PolicyTicket for Fake {
        fn error_reduction(&self) -> f64 {
            self.err
        }
        fn suggested_gain(&self) -> f64 {
            self.gain
        }
        fn access_granted(&self) -> bool {
            self.accessed
        }
        fn tick(&self) -> u64 {
            self.tick
        }
    }

    fn healthy() -> Fake {
        Fake { err: 0.5, gain: 1.0, accessed: true, tick: 0 }
    }

    fn hallucinating() -> Fake {
        Fake { err: 0.05, gain: 4.5, accessed: true, tick: 0 }
    }

    #[test]
    fn healthy_loop_never_clamps() {
        let mut p = RecurrencyPolicy::new(0.8, 8, 12);
        for _ in 0..40 {
            let action = p.observe(&healthy());
            assert_eq!(action, PolicyAction::None);
        }
        assert!(!p.is_clamping());
        assert!(p.vibe < 0.3);
    }

    #[test]
    fn hallucination_stream_triggers_clamp() {
        let mut p = RecurrencyPolicy::new(0.8, 8, 12);
        let mut saw_clamp = false;
        for _ in 0..20 {
            if p.observe(&hallucinating()) == PolicyAction::Clamp {
                saw_clamp = true;
                break;
            }
        }
        assert!(saw_clamp, "a weakly-clamped, gain-hungry stream must clamp");
        assert!(p.is_clamping());
        assert!(p.vibe > 0.8);
    }

    #[test]
    fn cooldown_prevents_immediate_release() {
        let mut p = RecurrencyPolicy::new(0.8, 8, 12);
        for _ in 0..20 {
            let _ = p.observe(&hallucinating());
        }
        assert!(p.is_clamping());
        // Reality returns instantly: still clamped because of the cooldown.
        for _ in 0..12 {
            assert_eq!(
                p.observe(&healthy()),
                PolicyAction::None,
                "cooldown must hold the clamp"
            );
        }
        assert!(p.is_clamping());
    }

    #[test]
    fn releases_after_cooldown_when_reality_returns() {
        let mut p = RecurrencyPolicy::new(0.8, 8, 12);
        for _ in 0..20 {
            let _ = p.observe(&hallucinating());
        }
        assert!(p.is_clamping());
        let mut released = false;
        for _ in 0..40 {
            if p.observe(&healthy()) == PolicyAction::Unclamp {
                released = true;
                break;
            }
        }
        assert!(released, "clamped system must release once the vibe drops");
        assert!(!p.is_clamping());
    }

    #[test]
    fn keeps_clamping_while_vibe_stays_high() {
        let mut p = RecurrencyPolicy::new(0.8, 8, 12);
        for _ in 0..20 {
            let _ = p.observe(&hallucinating());
        }
        assert!(p.is_clamping());
        let mut saw_unclamp = false;
        for _ in 0..50 {
            if p.observe(&hallucinating()) == PolicyAction::Unclamp {
                saw_unclamp = true;
            }
        }
        assert!(!saw_unclamp, "still hallucinating => clamp must hold");
        assert!(p.is_clamping());
    }

    #[test]
    fn accepts_both_core_and_wire_tickets() {
        use arkhe_recurrency::RecurrencyTicket;
        use arkhe_recurrency_daemon::proto::ProcessResponse;

        let core = RecurrencyTicket {
            stimulus_id: arkhe_recurrency::StimulusId("s".into()),
            local_loop_closed: true,
            error_reduction: 0.5,
            local_depth: 0.9,
            access_granted: true,
            tick: 1,
        };
        let wire = ProcessResponse {
            stimulus_id: "w".into(),
            local_depth: 0.1,
            error_reduction: 0.05,
            local_loop_closed: false,
            access_granted: true,
            tick: 2,
            suggested_gain: 4.5,
            burnt_fuel: 0.4,
        };
        assert_eq!(core.error_reduction(), 0.5);
        assert_eq!(core.suggested_gain(), 1.0);
        assert_eq!(wire.error_reduction(), 0.05);
        assert_eq!(wire.suggested_gain(), 4.5);

        let mut p = RecurrencyPolicy::new(0.8, 2, 8);
        assert_eq!(p.observe(&core), PolicyAction::None);
        assert_eq!(p.observe(&wire), PolicyAction::None);
        assert_eq!(p.observe(&wire), PolicyAction::Clamp);
    }
}
