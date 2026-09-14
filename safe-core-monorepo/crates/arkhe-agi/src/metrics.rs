//! Where a turn's time goes: per-phase timings, and their aggregate.
//!
//! `AgiCoordinator::process` brackets five phases with `Instant::now()` and
//! hands the deltas to a [`MetricsRecorder`] at the end of every turn. The
//! point is to be able to answer "is this session slow because of the model,
//! the safety check, the memory writes, or the geometric verification?"
//! with measured numbers rather than with a profile taken somewhere else.
//!
//! The recorder is deliberately dumb: it stores what it is handed and
//! computes min/mean/max. It has no sampling, no percentiles, no labels, and
//! no background task — a turn is recorded by the code that just ran it, and
//! the aggregate is computed when somebody asks.
//!
//! ```
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
//! use arkhe_agi::metrics::{MetricsRecorder, PhaseTimings};
//!
//! let metrics = MetricsRecorder::new();
//!
//! // An empty recorder answers with an empty aggregate, not with a default.
//! let empty = metrics.stats().await;
//! assert_eq!(empty.count, 0);
//! assert!(empty.last.is_none());
//!
//! metrics
//!     .record(PhaseTimings {
//!         safety_ms: 1,
//!         inference_ms: 800,
//!         geometric_ms: 2,
//!         memory_ms: 3,
//!         total_ms: 806,
//!     })
//!     .await;
//!
//! let stats = metrics.stats().await;
//! assert_eq!(stats.count, 1);
//! assert_eq!(stats.max_ms, 806);
//! # }
//! ```

use serde::{Deserialize, Serialize};

/// Wall-clock milliseconds spent in each phase of one `process()` call.
///
/// The phases are disjoint spans of the same turn, so `total_ms` is always
/// at least the sum of the other four; the gap is whatever the unmeasured
/// steps cost (history reads, the evidence append, logging). That gap is
/// part of why `total_ms` is stored rather than derived — the sum would be a
/// slightly different, slightly wrong number.
///
/// `Serialize`/`Deserialize` because a session's numbers are worth more
/// outside the process that measured them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseTimings {
    /// Safety verification (`SafetyVerifier::verify`) of the user's input.
    pub safety_ms: u64,

    /// The inference call, including everything the engine does — queueing,
    /// tokenising, sampling, any model load.
    pub inference_ms: u64,

    /// Geometric verification (FI-120) plus the provenance append: building
    /// a [`TurnRecord`](crate::geometry::TurnRecord), canonicalising and
    /// hashing it, running the registered rules, and hashing the artifact
    /// into the evidence chain.
    pub geometric_ms: u64,

    /// The two memory stores and the working/episodic link recorded for the
    /// memory graph (FI-124).
    pub memory_ms: u64,

    /// The whole turn, from entering `process()` to the end of the last
    /// phase — the number a latency budget is actually about.
    pub total_ms: u64,
}

/// Aggregate latency over the turns a [`MetricsRecorder`] has seen.
///
/// The aggregate is over [`PhaseTimings::total_ms`], the turn a caller waits
/// for; the per-phase breakdown of the most recent turn travels alongside it
/// in [`last`](LatencyStats::last).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LatencyStats {
    /// How many turns are covered.
    pub count: usize,

    /// The most recently recorded turn's timings — `None` only when
    /// `count == 0`, so a caller can ask "how long did the last turn take?"
    /// without guessing whether there was one.
    pub last: Option<PhaseTimings>,

    /// Smallest [`total_ms`](PhaseTimings::total_ms) covered. `0` when
    /// `count == 0`: there is no minimum to report, and `0` is the only
    /// value that leaves `min_ms <= avg_ms <= max_ms` true for an empty
    /// aggregate.
    pub min_ms: u64,

    /// Mean [`total_ms`](PhaseTimings::total_ms) over the turns covered.
    /// `0.0` when `count == 0`.
    pub avg_ms: f64,

    /// Largest [`total_ms`](PhaseTimings::total_ms) covered. `0` when
    /// `count == 0`, on the same terms as [`min_ms`](LatencyStats::min_ms).
    pub max_ms: u64,
}

impl LatencyStats {
    /// Aggregates `samples`, which may be empty.
    fn of(samples: &[PhaseTimings]) -> Self {
        let count = samples.len();
        if count == 0 {
            return Self {
                count: 0,
                last: None,
                min_ms: 0,
                avg_ms: 0.0,
                max_ms: 0,
            };
        }

        let mut min_ms = u64::MAX;
        let mut max_ms = 0;
        for sample in samples {
            min_ms = min_ms.min(sample.total_ms);
            max_ms = max_ms.max(sample.total_ms);
        }

        // Summed in `u128` so a long session cannot overflow the mean.
        let total: u128 = samples.iter().map(|s| u128::from(s.total_ms)).sum();
        let avg_ms = total as f64 / count as f64;

        Self {
            count,
            last: samples.last().copied(),
            min_ms,
            avg_ms,
            max_ms,
        }
    }
}

/// Records one [`PhaseTimings`] per turn and answers questions about them.
///
/// Interior mutability, like [`ToolRegistry`](crate::executor::ToolRegistry)
/// and for the same reason: the coordinator holds the recorder by value and
/// shares itself across tasks, so `record` takes `&self` rather than `&mut
/// self`.
///
/// Every sample is kept. That is a deliberate trade: an eviction policy
/// (a ring buffer, a reservoir) would cap memory but make `avg_ms` and
/// `max_ms` describe a window nobody named, and would answer "what was our
/// slowest turn?" with a lie once that turn aged out. A `PhaseTimings` is
/// 40 bytes, and a session is measured in thousands of turns, not billions;
/// [`samples`](MetricsRecorder::samples) hands the series back when a caller
/// wants a different aggregate than [`stats`](MetricsRecorder::stats)
/// computes.
pub struct MetricsRecorder {
    samples: tokio::sync::RwLock<Vec<PhaseTimings>>,
}

impl MetricsRecorder {
    /// A recorder that has seen nothing yet.
    pub fn new() -> Self {
        Self {
            samples: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    /// Records one turn's timings. Recording is append-only and cannot fail.
    pub async fn record(&self, timings: PhaseTimings) {
        self.samples.write().await.push(timings);
    }

    /// Aggregate over every turn recorded so far, in recording order.
    pub async fn stats(&self) -> LatencyStats {
        let samples = self.samples.read().await;
        LatencyStats::of(&samples)
    }

    /// Every recorded turn, oldest first.
    ///
    /// The aggregate above only covers `total_ms`; this is the per-phase
    /// series behind it, for a caller that wants a phase distribution rather
    /// than a mean.
    pub async fn samples(&self) -> Vec<PhaseTimings> {
        self.samples.read().await.clone()
    }
}

impl Default for MetricsRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timings(total_ms: u64) -> PhaseTimings {
        PhaseTimings {
            safety_ms: 1,
            inference_ms: total_ms - 4,
            geometric_ms: 1,
            memory_ms: 2,
            total_ms,
        }
    }

    #[tokio::test]
    async fn an_empty_recorder_reports_an_empty_aggregate() {
        let stats = MetricsRecorder::new().stats().await;
        assert_eq!(stats.count, 0);
        assert!(stats.last.is_none());
        assert_eq!(stats.min_ms, 0);
        assert_eq!(stats.avg_ms, 0.0);
        assert_eq!(stats.max_ms, 0);
    }

    #[tokio::test]
    async fn the_aggregate_covers_every_recording() {
        let metrics = MetricsRecorder::new();
        metrics.record(timings(10)).await;
        metrics.record(timings(30)).await;
        metrics.record(timings(20)).await;

        let stats = metrics.stats().await;
        assert_eq!(stats.count, 3);
        assert_eq!(stats.min_ms, 10);
        assert_eq!(stats.max_ms, 30);
        assert_eq!(stats.avg_ms, 20.0);
        assert_eq!(stats.last, Some(timings(20)));
    }

    #[tokio::test]
    async fn the_min_average_max_order_holds_for_a_single_turn() {
        let metrics = MetricsRecorder::new();
        metrics.record(timings(7)).await;

        let stats = metrics.stats().await;
        assert!(stats.min_ms as f64 <= stats.avg_ms);
        assert!(stats.avg_ms <= stats.max_ms as f64);
    }

    #[tokio::test]
    async fn the_per_phase_series_is_available_behind_the_aggregate() {
        let metrics = MetricsRecorder::new();
        metrics.record(timings(10)).await;
        metrics.record(timings(20)).await;

        let samples = metrics.samples().await;
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].inference_ms, 6);
        assert_eq!(samples[1].total_ms, 20);
    }

    #[test]
    fn timings_survive_a_round_trip() {
        let json = serde_json::to_string(&timings(42)).unwrap();
        assert_eq!(serde_json::from_str::<PhaseTimings>(&json).unwrap(), timings(42));
    }
}
