//! Consciousness watchdog — timeout-driven rollback guardian.
//!
//! The watchdog enforces two guarantees for the embedded device:
//!
//! 1. **Temporal bound (Gravity-1):** if no healthy assessment arrives within
//!    [`MAX_SILENT_TICKS`], the watchdog rolls back to the last known healthy
//!    state. Timestamps must be monotonic; non-monotonic feeds are rejected.
//! 2. **Constitutional bound (C-01/C-02):** an assessment that violates the
//!    constitutional invariants forces an immediate rollback — no waiting for
//!    the timeout.

use crate::invariants::CoherenceState;

/// Default maximum silent ticks between healthy assessments before rollback.
pub const MAX_SILENT_TICKS: u32 = 60;

/// Watchdog state transition result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogEvent {
    /// Assessment accepted and is healthy; no action required.
    Healthy,
    /// Assessment accepted but degraded; still tolerated for now.
    Degraded,
    /// Rollback triggered because the device stayed silent too long.
    TimeoutRollback,
    /// Rollback triggered by a constitutional invariant violation.
    ConstitutionalRollback,
    /// Feed rejected (non-monotonic timestamp) — no state change.
    StaleTimestampRejected,
}

/// Firmware consciousness watchdog.
#[derive(Debug)]
pub struct ConsciousnessWatchdog {
    /// Maximum allowed ticks between healthy assessments.
    max_silent_ticks: u32,
    /// Last healthy covert state (rollback target).
    last_healthy: Option<CoherenceState>,
    /// Tick of the last accepted feed.
    last_tick: u32,
    /// Number of consecutive degraded accepts (for telemetry).
    degraded_count: u32,
}

impl Default for ConsciousnessWatchdog {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsciousnessWatchdog {
    /// Create a watchdog with the default silence limit.
    pub fn new() -> Self {
        Self {
            max_silent_ticks: MAX_SILENT_TICKS,
            last_healthy: None,
            last_tick: 0,
            degraded_count: 0,
        }
    }

    /// Create a watchdog with a custom maximum silent ticks.
    pub fn with_timeout(max_silent_ticks: u32) -> Self {
        Self {
            max_silent_ticks,
            ..Self::new()
        }
    }

    /// Configured timeout in ticks.
    pub const fn max_silent_ticks(&self) -> u32 {
        self.max_silent_ticks
    }

    /// Last accepted tick (0 if nothing was fed).
    pub const fn last_tick(&self) -> u32 {
        self.last_tick
    }

    /// Last known healthy state (rollback target).
    pub const fn last_healthy(&self) -> Option<CoherenceState> {
        self.last_healthy
    }

    /// Count of degraded-but-tolerated assessments.
    pub const fn degraded_count(&self) -> u32 {
        self.degraded_count
    }

    /// Feed a freshly-assessed snapshot. Returns the transition event.
    ///
    /// - Rejects non-monotonic ticks (Gravity-1) without side effects.
    /// - Rolls back immediately on constitutional violation.
    /// - Rolls back on silence beyond `max_silent_ticks` (timeout).
    pub fn feed(&mut self, state: CoherenceState, assessment: AssessmentLike) -> WatchdogEvent {
        let tick = state.uptime_ticks;
        if tick < self.last_tick {
            return WatchdogEvent::StaleTimestampRejected;
        }
        if tick > self.last_tick + self.max_silent_ticks {
            // Timeout: the device was silent too long; the caller restores
            // `last_healthy`. Report the event regardless of whether a healthy
            // target exists yet (fresh device).
            self.last_tick = tick;
            if assessment.is_constitutional() {
                self.last_healthy = Some(state);
            }
            return WatchdogEvent::TimeoutRollback;
        }

        self.last_tick = tick;

        if assessment.is_constitutional() {
            if assessment.passed {
                self.last_healthy = Some(state);
                self.degraded_count = 0;
                return WatchdogEvent::Healthy;
            }
            // Constitutional but below the operational Φ threshold: tolerated.
            self.degraded_count = self.degraded_count.saturating_add(1);
            return WatchdogEvent::Degraded;
        }

        // Constitutional violation -> immediate rollback.
        self.degraded_count = self.degraded_count.saturating_add(1);
        WatchdogEvent::ConstitutionalRollback
    }
}

/// Minimal assessment projection consumed by the watchdog.
///
/// Provided separately so the watchdog can be driven by the bridge's
/// [`Assessment`](crate::minimal_bridge::Assessment) via [`from_assessment`],
/// or by a raw diagnostic without pulling in the full bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssessmentLike {
    /// Whether C-01 and C-02 both hold.
    pub constitutional: bool,
    /// Whether the assessment passed operational Φ acceptance.
    pub passed: bool,
}

impl AssessmentLike {
    /// Build from a bridge assessment.
    pub const fn from_assessment(a: crate::minimal_bridge::Assessment) -> Self {
        Self {
            constitutional: a.constitutional,
            passed: a.passed,
        }
    }

    /// Convenience `const` constructor.
    pub const fn new(constitutional: bool, passed: bool) -> Self {
        Self {
            constitutional,
            passed,
        }
    }

    const fn is_constitutional(&self) -> bool {
        self.constitutional
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minimal_bridge::{Assessment, MinimalConsciousnessBridge};

    fn assess(bridge: &mut MinimalConsciousnessBridge, state: CoherenceState) -> Assessment {
        bridge.assess(state)
    }

    #[test]
    fn healthy_feed_updates_last_healthy() {
        let mut wd = ConsciousnessWatchdog::new();
        let mut bridge = MinimalConsciousnessBridge::new();
        let state = CoherenceState::healthy().with_uptime(1);
        let a = assess(&mut bridge, state);
        assert_eq!(
            wd.feed(state, AssessmentLike::from_assessment(a)),
            WatchdogEvent::Healthy
        );
        assert_eq!(wd.last_healthy().unwrap().uptime_ticks, 1);
    }

    #[test]
    fn timeout_triggers_rollback() {
        let mut wd = ConsciousnessWatchdog::with_timeout(10);
        let mut bridge = MinimalConsciousnessBridge::new();

        let healthy = CoherenceState::healthy().with_uptime(1);
        let a = assess(&mut bridge, healthy);
        wd.feed(healthy, AssessmentLike::from_assessment(a));

        // Jump past the timeout; state is degenerate (alls quiet).
        let silent_state = CoherenceState::unknown().with_uptime(20);
        let a = assess(&mut bridge, silent_state);
        assert_eq!(
            wd.feed(silent_state, AssessmentLike::from_assessment(a)),
            WatchdogEvent::TimeoutRollback
        );
        assert_eq!(wd.last_healthy().unwrap().uptime_ticks, 1);
    }

    #[test]
    fn constitutional_violation_rolls_back_immediately() {
        let mut wd = ConsciousnessWatchdog::new();
        let mut bridge = MinimalConsciousnessBridge::new();

        let healthy = CoherenceState::healthy().with_uptime(1);
        let a = assess(&mut bridge, healthy);
        wd.feed(healthy, AssessmentLike::from_assessment(a));

        let mut bad = healthy;
        bad.introspection = false;
        bad.uptime_ticks = 2;
        let a = assess(&mut bridge, bad);
        assert_eq!(
            wd.feed(bad, AssessmentLike::from_assessment(a)),
            WatchdogEvent::ConstitutionalRollback
        );
        assert_eq!(wd.last_healthy().unwrap().uptime_ticks, 1);
    }

    #[test]
    fn stale_timestamp_rejected() {
        let mut wd = ConsciousnessWatchdog::new();
        let mut bridge = MinimalConsciousnessBridge::new();
        let s1 = CoherenceState::healthy().with_uptime(10);
        let a = assess(&mut bridge, s1);
        wd.feed(s1, AssessmentLike::from_assessment(a));

        let s0 = CoherenceState::healthy().with_uptime(9);
        let a = assess(&mut bridge, s0);
        assert_eq!(
            wd.feed(s0, AssessmentLike::from_assessment(a)),
            WatchdogEvent::StaleTimestampRejected
        );
        assert_eq!(wd.last_tick(), 10);
    }

    #[test]
    fn degraded_is_tolerated_until_constitutional_loss() {
        let mut wd = ConsciousnessWatchdog::new();
        let mut bridge = MinimalConsciousnessBridge::new();
        let healthy = CoherenceState::healthy().with_uptime(1);
        let a = assess(&mut bridge, healthy);
        wd.feed(healthy, AssessmentLike::from_assessment(a));

        // Low attention is operational-degraded but still constitutional.
        let mut degraded = healthy;
        degraded.attention = 0;
        degraded.uptime_ticks = 2;
        let a = assess(&mut bridge, degraded);
        assert_eq!(
            wd.feed(degraded, AssessmentLike::from_assessment(a)),
            WatchdogEvent::Degraded
        );
        assert_eq!(wd.degraded_count(), 1);
    }

    #[test]
    fn timeout_without_healthy_target_never_rolls_back() {
        let mut wd = ConsciousnessWatchdog::new();
        let mut bridge = MinimalConsciousnessBridge::new();
        let s = CoherenceState::unknown().with_uptime(100);
        let a = assess(&mut bridge, s);
        // No last healthy yet -> timeout path has nothing to restore, event is
        // still reported as timeout for telemetry but last_healthy stays None.
        assert_eq!(
            wd.feed(s, AssessmentLike::from_assessment(a)),
            WatchdogEvent::TimeoutRollback
        );
        assert!(wd.last_healthy().is_none());
    }

    #[test]
    fn healthy_recovers_degraded_count() {
        let mut wd = ConsciousnessWatchdog::new();
        let mut bridge = MinimalConsciousnessBridge::new();
        let healthy = CoherenceState::healthy().with_uptime(1);
        let a = assess(&mut bridge, healthy);
        wd.feed(healthy, AssessmentLike::from_assessment(a));

        let mut degraded = healthy;
        degraded.attention = 0;
        degraded.uptime_ticks = 2;
        let a = assess(&mut bridge, degraded);
        wd.feed(degraded, AssessmentLike::from_assessment(a));
        assert_eq!(wd.degraded_count(), 1);

        let a = assess(&mut bridge, healthy.with_uptime(3));
        wd.feed(healthy.with_uptime(3), AssessmentLike::from_assessment(a));
        assert_eq!(wd.degraded_count(), 0);
    }

    #[test]
    fn gap1_floored_phi_is_not_watchdog_healthy() {
        // unknown state -> phi clamps to the Gap-1 floor -> not passed.
        let mut wd = ConsciousnessWatchdog::new();
        let mut bridge = MinimalConsciousnessBridge::new();
        let s = CoherenceState::unknown().with_uptime(1);
        let a = assess(&mut bridge, s);
        assert!(!a.passed);
        assert_eq!(
            wd.feed(s, AssessmentLike::from_assessment(a)),
            WatchdogEvent::ConstitutionalRollback
        );
    }
}