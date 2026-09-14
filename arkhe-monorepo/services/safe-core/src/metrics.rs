//! In-memory metrics registry for the Safe-Core watchdog — the state behind
//! the Power BI REST endpoints.
//!
//! The registry mirrors the policy's decisions into counters and a bounded
//! ring buffer of recent observations. It is intentionally *derived state*:
//! the source of truth remains the JSONL log (Loopseal-2), this struct only
//! answers "how is the governor doing *right now*".
//!
//! All access goes through a single `std::sync::Mutex` — recorders are called
//! synchronously from the watchdog loop, and handlers only ever take short
//! snapshots, so there is no contention to speak of.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::telemetry::{PolicyAction, PolicyObservation};

/// Capacity of the recent-observations ring buffer served by
/// `GET /api/v1/policy/tickets`.
pub const RECENT_CAPACITY: usize = 1024;

/// Point-in-time summary served by `GET /api/v1/policy/metrics`.
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSummary {
    /// Unix timestamp (seconds) the watchdog started.
    pub started_at: u64,
    /// Total tickets observed.
    pub tickets_total: u64,
    /// Times the clamp was engaged.
    pub clamps_total: u64,
    /// Times the clamp was released.
    pub releases_total: u64,
    /// Last daemon tick seen (0 if none yet).
    pub last_tick: u64,
    /// Current windowed vibe.
    pub vibe: f64,
    /// Highest vibe since the current clamp began (0.0 when not clamping).
    pub peak_vibe: f64,
    /// Whether the clamp latch is currently held.
    pub clamping: bool,
    /// Policy configuration echo — Power BI renders it as the threshold line.
    pub threshold: f64,
    pub window: usize,
    pub cooldown: u64,
}

#[derive(Debug)]
struct Inner {
    started_at: u64,
    tickets_total: u64,
    clamps_total: u64,
    releases_total: u64,
    last_tick: u64,
    vibe: f64,
    peak_vibe: f64,
    clamping: bool,
    threshold: f64,
    window: usize,
    cooldown: u64,
    recent: VecDeque<PolicyObservation>,
}

/// Clonable handle onto the shared registry state.
#[derive(Debug, Clone)]
pub struct MetricsRegistry {
    inner: Arc<Mutex<Inner>>,
}

impl MetricsRegistry {
    /// Create a registry echoing the policy's canonical configuration.
    pub fn new(policy: &crate::RecurrencyPolicy) -> Self {
        let inner = Inner {
            started_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default(),
            tickets_total: 0,
            clamps_total: 0,
            releases_total: 0,
            last_tick: 0,
            vibe: policy.vibe,
            peak_vibe: policy.peak_vibe,
            clamping: policy.is_clamping(),
            threshold: policy.threshold,
            window: policy.window,
            cooldown: policy.cooldown,
            recent: VecDeque::with_capacity(RECENT_CAPACITY),
        };
        Self { inner: Arc::new(Mutex::new(inner)) }
    }

    /// Fold one observation into the counters and ring buffer.
    pub fn record(&self, observation: &PolicyObservation) {
        let mut inner = self.lock();
        inner.tickets_total += 1;
        inner.last_tick = observation.tick;
        inner.vibe = observation.vibe;
        inner.clamping = observation.clamping;
        match observation.action {
            PolicyAction::Clamp => {
                // A fresh engagement resets the peak tracker, mirroring the
                // policy's own lifecycle.
                inner.peak_vibe = observation.vibe;
                inner.clamps_total += 1;
            }
            PolicyAction::Unclamp => {
                inner.peak_vibe = 0.0;
                inner.releases_total += 1;
            }
            PolicyAction::None => {}
        }
        if inner.recent.len() == RECENT_CAPACITY {
            inner.recent.pop_front();
        }
        inner.recent.push_back(observation.clone());
    }

    /// Current aggregate snapshot.
    pub fn snapshot(&self) -> MetricsSummary {
        let inner = self.lock();
        MetricsSummary {
            started_at: inner.started_at,
            tickets_total: inner.tickets_total,
            clamps_total: inner.clamps_total,
            releases_total: inner.releases_total,
            last_tick: inner.last_tick,
            vibe: inner.vibe,
            peak_vibe: inner.peak_vibe,
            clamping: inner.clamping,
            threshold: inner.threshold,
            window: inner.window,
            cooldown: inner.cooldown,
        }
    }

    /// The most recent `limit` observations, oldest first.
    pub fn recent(&self, limit: usize) -> Vec<PolicyObservation> {
        let inner = self.lock();
        let skip = inner.recent.len().saturating_sub(limit);
        inner.recent.iter().skip(skip).cloned().collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl crate::telemetry::TelemetryRecorder for MetricsRegistry {
    fn record(&mut self, observation: &PolicyObservation) {
        MetricsRegistry::record(self, observation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RecurrencyPolicy;

    fn observation(vibe: f64, action: PolicyAction, clamping: bool) -> PolicyObservation {
        PolicyObservation {
            ts: 42,
            tick: 1,
            error_reduction: 0.05,
            access_granted: true,
            suggested_gain: 1.0,
            vibe,
            action,
            clamping,
        }
    }

    #[test]
    fn counters_track_clamp_lifecycle() {
        let policy = RecurrencyPolicy::new(0.8, 8, 12);
        let registry = MetricsRegistry::new(&policy);

        registry.record(&observation(0.3, PolicyAction::None, false));
        registry.record(&observation(0.9, PolicyAction::Clamp, true));
        // While latched, plain observations still run under the clamp.
        registry.record(&observation(0.85, PolicyAction::None, true));

        let snap = registry.snapshot();
        assert_eq!(snap.tickets_total, 3);
        assert_eq!(snap.clamps_total, 1);
        assert_eq!(snap.releases_total, 0);
        assert!(snap.clamping);
        assert!((snap.peak_vibe - 0.9).abs() < 1e-12);
        assert!((snap.threshold - 0.8).abs() < 1e-12);
        assert_eq!(snap.window, 8);
        assert_eq!(snap.cooldown, 12);

        registry.record(&observation(0.1, PolicyAction::Unclamp, false));
        let snap = registry.snapshot();
        assert_eq!(snap.releases_total, 1);
        assert!(!snap.clamping);
        assert_eq!(snap.peak_vibe, 0.0);
    }

    #[test]
    fn recent_buffer_is_bounded_and_ordered() {
        let policy = RecurrencyPolicy::new(0.8, 8, 12);
        let registry = MetricsRegistry::new(&policy);
        for i in 0..(RECENT_CAPACITY + 50) {
            registry.record(&observation(i as f64 / 1000.0, PolicyAction::None, false));
        }
        let all = registry.recent(RECENT_CAPACITY);
        assert_eq!(all.len(), RECENT_CAPACITY);
        // Oldest-first ordering, newest observation last.
        assert_eq!(all[0].vibe, 50.0 / 1000.0);
        let tail = registry.recent(5);
        assert_eq!(tail.len(), 5);
        assert_eq!(
            tail.last().unwrap().vibe,
            all.last().unwrap().vibe,
            "recent(limit) must end at the newest observation"
        );
    }
}
