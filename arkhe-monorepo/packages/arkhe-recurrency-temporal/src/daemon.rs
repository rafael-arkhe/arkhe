//! Temporal state daemon (v0.4.3).
//!
//! The async arousal daemon wraps the pure [`StateDaemon`] of the core and adds
//! a *temporal integrity history*: every perception cycle is recorded into an
//! exponentially-weighted moving average of `success_rate` and `mean_latency`,
//! plus a count of currently open (failed) loops. The v0.4.3 patch fixes an
//! inconsistency in which an iGuard-forced `Coma` left the temporal EWMA at
//! `success_rate == 1.0` — because the failing cycle was never recorded. Here
//! [`TemporalStateDaemon::record_cycle`] is the single source of truth and
//! must be called *before* [`TemporalStateDaemon::set_regime`].

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use arkhe_recurrency::{ArousalRegime, StateDaemon};
use tokio::sync::broadcast;

use crate::perception::Perception;

/// Snapshot of the temporal integrity history.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TemporalIntegrity {
    /// EWMA of the per-cycle success flag.
    pub success_rate: f64,
    /// Number of failed cycles currently open in the history window.
    pub open_loops: usize,
    /// EWMA of the per-cycle latency in milliseconds.
    pub mean_latency_ms: f64,
    /// Size of the sliding history window.
    pub window: usize,
}

/// One recorded perception cycle.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TemporalRecord {
    /// The action that closed the loop.
    pub action_id: crate::perception::ActionId,
    /// Perceived integrity (0.0 = hostile/unclamped).
    pub integrity: f64,
    /// Whether the cycle graded as successful (`integrity >= threshold`).
    pub success: bool,
    /// Latency of the cycle in milliseconds.
    pub latency_ms: u64,
    /// Daemon tick at which the cycle was recorded.
    pub tick: u64,
}

/// Events published by the temporal daemon to its modulation stream.
#[derive(Clone, Debug, serde::Serialize)]
pub enum TemporalEvent {
    /// A perception cycle was recorded.
    CycleRecorded(TemporalRecord),
    /// The arousal regime was switched.
    RegimeChanged {
        /// Regime before the switch.
        previous: ArousalRegime,
        /// Regime after the switch.
        current: ArousalRegime,
    },
}

struct Inner {
    arousal: StateDaemon,
    alpha: f64,
    success_threshold: f64,
    success_rate: f64,
    mean_latency_ms: f64,
    open_loops: usize,
    history: VecDeque<TemporalRecord>,
    window_capacity: usize,
    clock: u64,
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner")
            .field("arousal", &self.arousal)
            .field("alpha", &self.alpha)
            .field("success_threshold", &self.success_threshold)
            .field("success_rate", &self.success_rate)
            .field("mean_latency_ms", &self.mean_latency_ms)
            .field("open_loops", &self.open_loops)
            .field("history", &self.history)
            .field("window_capacity", &self.window_capacity)
            .field("clock", &self.clock)
            .finish()
    }
}

/// Async arousal daemon with temporal integrity history.
#[derive(Clone, Debug)]
pub struct TemporalStateDaemon {
    inner: Arc<Mutex<Inner>>,
    events: broadcast::Sender<TemporalEvent>,
}

impl TemporalStateDaemon {
    /// Create a daemon in the given regime with default history parameters
    /// (EWMA `alpha = 0.3`, success threshold `0.5`, window of 16 cycles).
    pub fn new(regime: ArousalRegime) -> Self {
        Self::new_with(regime, 0.3, 0.5, 16)
    }

    /// Create a daemon with explicit history parameters.
    pub fn new_with(
        regime: ArousalRegime,
        alpha: f64,
        success_threshold: f64,
        window_capacity: usize,
    ) -> Self {
        let (events, _) = broadcast::channel(64);
        Self {
            inner: Arc::new(Mutex::new(Inner {
                arousal: StateDaemon::new(regime),
                alpha,
                success_threshold,
                success_rate: 1.0,
                mean_latency_ms: 0.0,
                open_loops: 0,
                history: VecDeque::with_capacity(window_capacity),
                window_capacity,
                clock: 0,
            })),
            events,
        }
    }

    /// Current arousal regime.
    pub fn regime(&self) -> ArousalRegime {
        self.inner.lock().expect("daemon mutex").arousal.regime()
    }

    /// Switch the arousal regime, returning the previous one.
    ///
    /// **v0.4.3:** a failing perception must be recorded via
    /// [`TemporalStateDaemon::record_cycle`] *before* forcing `Coma`, or the
    /// temporal EWMA stays inconsistent with reality.
    pub fn set_regime(&self, regime: ArousalRegime) -> ArousalRegime {
        let previous = {
            let mut inner = self.inner.lock().expect("daemon mutex");
            inner.arousal.set_regime(regime)
        };
        let _ = self.events.send(TemporalEvent::RegimeChanged {
            previous,
            current: regime,
        });
        previous
    }

    /// Record one perception cycle into the temporal history.
    ///
    /// Updates the EWMA `success_rate`, `mean_latency_ms` and the `open_loops`
    /// counter (failed cycles currently in the window), appends the record and
    /// publishes a [`TemporalEvent::CycleRecorded`].
    pub fn record_cycle(&self, perception: &Perception) -> TemporalRecord {
        let success = {
            let inner = self.inner.lock().expect("daemon mutex");
            perception.integrity >= inner.success_threshold
        };
        let record = {
            let mut inner = self.inner.lock().expect("daemon mutex");
            inner.clock += 1;
            let tick = inner.clock;
            inner.success_rate =
                inner.alpha * (success as u8 as f64) + (1.0 - inner.alpha) * inner.success_rate;
            inner.mean_latency_ms =
                inner.alpha * perception.latency_ms as f64 + (1.0 - inner.alpha) * inner.mean_latency_ms;
            let record = TemporalRecord {
                action_id: perception.action_id.clone(),
                integrity: perception.integrity,
                success,
                latency_ms: perception.latency_ms,
                tick,
            };
            inner.history.push_back(record.clone());
            while inner.history.len() > inner.window_capacity {
                inner.history.pop_front();
            }
            inner.open_loops = inner.history.iter().filter(|r| !r.success).count();
            record
        };
        let _ = self.events.send(TemporalEvent::CycleRecorded(record.clone()));
        record
    }

    /// Current temporal integrity snapshot.
    pub fn current_integrity(&self) -> TemporalIntegrity {
        let inner = self.inner.lock().expect("daemon mutex");
        TemporalIntegrity {
            success_rate: inner.success_rate,
            open_loops: inner.open_loops,
            mean_latency_ms: inner.mean_latency_ms,
            window: inner.window_capacity,
        }
    }

    /// Subscribe to the modulation event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<TemporalEvent> {
        self.events.subscribe()
    }

    /// Daemon tick of the most recently recorded cycle.
    pub fn clock(&self) -> u64 {
        self.inner.lock().expect("daemon mutex").clock
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perception::ActionId;

    fn perception(integrity: f64, latency_ms: u64) -> Perception {
        Perception {
            action_id: ActionId("probe".into()),
            delta: vec![0.5; 8],
            latency_ms,
            integrity,
        }
    }

    #[test]
    fn healthy_cycles_keep_success_rate_at_one() {
        let d = TemporalStateDaemon::new(ArousalRegime::Alert);
        for _ in 0..8 {
            d.record_cycle(&perception(1.0, 120));
        }
        let i = d.current_integrity();
        assert_eq!(i.success_rate, 1.0);
        assert_eq!(i.open_loops, 0);
    }

    #[test]
    fn a_failed_cycle_drops_success_rate_and_opens_a_loop() {
        let d = TemporalStateDaemon::new(ArousalRegime::Alert);
        for _ in 0..8 {
            d.record_cycle(&perception(1.0, 120));
        }
        let r = d.record_cycle(&perception(0.0, 900));
        let i = d.current_integrity();
        assert!(!r.success);
        assert!(i.success_rate < 1.0, "EWMA must drop, got {}", i.success_rate);
        assert!(i.open_loops >= 1, "failed cycle must open a loop");
        assert!(i.mean_latency_ms > 0.0);
    }

    #[test]
    fn set_regime_returns_previous_and_publishes_event() {
        let d = TemporalStateDaemon::new(ArousalRegime::Alert);
        let mut rx = d.subscribe();
        assert_eq!(d.set_regime(ArousalRegime::DeepSleep), ArousalRegime::Alert);
        assert_eq!(d.regime(), ArousalRegime::DeepSleep);
        match rx.try_recv() {
            Ok(TemporalEvent::RegimeChanged {
                previous,
                current,
            }) => {
                assert_eq!(previous, ArousalRegime::Alert);
                assert_eq!(current, ArousalRegime::DeepSleep);
            }
            other => panic!("expected RegimeChanged, got {other:?}"),
        }
    }

    #[test]
    fn record_cycle_publishes_event_on_the_stream() {
        let d = TemporalStateDaemon::new(ArousalRegime::Alert);
        let mut rx = d.subscribe();
        d.record_cycle(&perception(0.0, 700));
        match rx.try_recv() {
            Ok(TemporalEvent::CycleRecorded(r)) => {
                assert!(!r.success);
                assert_eq!(r.latency_ms, 700);
            }
            other => panic!("expected CycleRecorded, got {other:?}"),
        }
    }
}
