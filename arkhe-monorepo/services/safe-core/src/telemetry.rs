//! Telemetry sink for the Safe-Core watchdog — the export leg of the
//! observability pipeline (Loopseal-2: append-only, tamper-evident logs).
//!
//! Every ticket the policy observes is serialized as one JSON object per line
//! (JSONL) and appended to a log file. The format was chosen to be directly
//! consumable by Power BI / Power Query without an intermediate transform:
//!
//! ```json
//! {"ts":1767225600,"tick":42,"error_reduction":0.05,"access_granted":true,
//!  "suggested_gain":4.5,"vibe":0.9375,"action":"clamp","clamping":true}
//! ```
//!
//! The sink *never* blocks or kills the watchdog: if the file cannot be
//! opened, recording degrades to a logged warning and the policy keeps
//! running. Telemetry is best-effort; enforcement is not.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// What the policy demanded after observing a ticket.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    /// Keep the current regime.
    None,
    /// Force `DeepSleep` (anti-hallucination clamp engaged).
    Clamp,
    /// Return to `Alert` (clamp released).
    Unclamp,
}

impl From<crate::PolicyAction> for PolicyAction {
    fn from(action: crate::PolicyAction) -> Self {
        match action {
            crate::PolicyAction::None => PolicyAction::None,
            crate::PolicyAction::Clamp => PolicyAction::Clamp,
            crate::PolicyAction::Unclamp => PolicyAction::Unclamp,
        }
    }
}

/// One observation of the policy over one ticket — the atomic unit of
/// Safe-Core telemetry. Flat by design so Power Query needs zero expansion.
#[derive(Clone, Debug, Serialize)]
pub struct PolicyObservation {
    /// Unix timestamp (seconds) of the observation.
    pub ts: u64,
    /// Effective tick of the state daemon.
    pub tick: u64,
    /// Pass-2 relative error reduction of the perception cycle.
    pub error_reduction: f64,
    /// Whether the global workspace broadcast the content.
    pub access_granted: bool,
    /// Gain multiplier suggested by the daemon (telemetry only).
    pub suggested_gain: f64,
    /// Windowed vibe at the moment of the decision.
    pub vibe: f64,
    /// Demand the policy made (`none` | `clamp` | `unclamp`).
    pub action: PolicyAction,
    /// Whether the clamp latch is currently held.
    pub clamping: bool,
}

impl PolicyObservation {
    /// Snapshot a policy decision into a serializable observation.
    pub fn new(
        tick: u64,
        error_reduction: f64,
        access_granted: bool,
        suggested_gain: f64,
        policy: &crate::RecurrencyPolicy,
        action: crate::PolicyAction,
    ) -> Self {
        Self {
            ts: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default(),
            tick,
            error_reduction,
            access_granted,
            suggested_gain,
            vibe: policy.vibe,
            action: action.into(),
            clamping: policy.is_clamping(),
        }
    }

    /// Render one JSONL line (no trailing newline).
    pub fn to_jsonl(&self) -> String {
        serde_json::to_string(self).expect("PolicyObservation is always serializable")
    }
}

/// Append-only JSONL telemetry sink.
///
/// Clonable: every clone shares the same underlying file handle.
pub struct TelemetrySink {
    inner: std::sync::Arc<Mutex<Option<std::fs::File>>>,
    path: PathBuf,
}

impl std::fmt::Debug for TelemetrySink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelemetrySink").field("path", &self.path).finish()
    }
}

impl Clone for TelemetrySink {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone(), path: self.path.clone() }
    }
}

impl TelemetrySink {
    /// Open (or create) the JSONL log in append mode.
    ///
    /// A failure to open the file is *not* fatal: the sink is returned in a
    /// degraded state and every write logs a warning instead.
    pub fn open(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new().create(true).append(true).open(&path);
        let inner = match file {
            Ok(file) => Some(file),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "telemetry sink degraded: cannot open log");
                None
            }
        };
        Self { inner: std::sync::Arc::new(Mutex::new(inner)), path }
    }

    /// Path the sink appends to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one observation as a JSONL line.
    #[allow(clippy::let_underscore_must_use)]
    pub fn record(&self, observation: &PolicyObservation) {
        let mut guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        match guard.as_mut() {
            Some(file) => {
                let _ = writeln!(file, "{}", observation.to_jsonl());
            }
            None => {
                // Retry once per call: the file may have become writable.
                match OpenOptions::new().create(true).append(true).open(&self.path) {
                    Ok(mut file) => {
                        let _ = writeln!(file, "{}", observation.to_jsonl());
                        *guard = Some(file);
                        tracing::info!(path = %self.path.display(), "telemetry sink recovered");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "telemetry sink still degraded");
                    }
                }
            }
        }
    }
}

/// A sink that discards everything — used when telemetry is disabled and as
/// the default recorder of [`crate::run_watchdog`].
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSink;

impl TelemetryRecorder for NoopSink {
    fn record(&mut self, _observation: &PolicyObservation) {}
}

/// Anything the watchdog can push observations into.
///
/// Implemented by [`TelemetrySink`] (file), [`MetricsRegistry`](crate::metrics::MetricsRegistry)
/// and [`NoopSink`]; combinators via [`Fanout`].
pub trait TelemetryRecorder {
    /// Consume one observation.
    fn record(&mut self, observation: &PolicyObservation);
}

impl TelemetryRecorder for TelemetrySink {
    fn record(&mut self, observation: &PolicyObservation) {
        TelemetrySink::record(self, observation);
    }
}

/// Send one observation to several recorders (file + registry + ...).
#[derive(Debug)]
pub struct Fanout<A, B>(pub A, pub B);

impl<A: TelemetryRecorder, B: TelemetryRecorder> TelemetryRecorder for Fanout<A, B> {
    fn record(&mut self, observation: &PolicyObservation) {
        self.0.record(observation);
        self.1.record(observation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(vibe: f64, action: PolicyAction) -> PolicyObservation {
        PolicyObservation {
            ts: 1_767_225_600,
            tick: 7,
            error_reduction: 0.05,
            access_granted: true,
            suggested_gain: 4.5,
            vibe,
            action,
            clamping: action == PolicyAction::Clamp,
        }
    }

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("safe-core-telemetry-test-{tag}.jsonl"))
    }

    #[test]
    fn jsonl_line_is_flat_and_parseable() {
        let line = sample(0.9375, PolicyAction::Clamp).to_jsonl();
        let json: serde_json::Value = serde_json::from_str(&line).expect("valid json");
        assert_eq!(json["tick"], 7);
        assert_eq!(json["vibe"], 0.9375);
        assert_eq!(json["action"], "clamp");
        assert_eq!(json["access_granted"], true);
        assert!(json.get("ts").is_some());
    }

    #[test]
    fn sink_appends_one_line_per_record() {
        let path = temp_path("append");
        let _ = std::fs::remove_file(&path);
        let sink = TelemetrySink::open(&path);
        assert_eq!(sink.path(), path.as_path());

        sink.record(&sample(0.9, PolicyAction::Clamp));
        sink.record(&sample(0.2, PolicyAction::None));

        let content = std::fs::read_to_string(&path).expect("log readable");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["vibe"], 0.2);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn degraded_sink_survives_and_recovers() {
        // A directory at the target path can never be opened as a file.
        let path = temp_path("degraded-dir");
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).expect("create blocking dir");

        let sink = TelemetrySink::open(&path); // opens fail -> degraded
        sink.record(&sample(0.5, PolicyAction::None)); // must not panic

        let _ = std::fs::remove_dir_all(&path);
        // After the blocker is gone, the retry-on-write path recovers.
        sink.record(&sample(0.6, PolicyAction::None));
        let content = std::fs::read_to_string(&path).expect("recovered log readable");
        assert_eq!(content.lines().count(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn fanout_reaches_both_recorders() {
        let path = temp_path("fanout");
        let _ = std::fs::remove_file(&path);
        let mut fan = Fanout(NoopSink, TelemetrySink::open(&path));
        fan.record(&sample(0.8, PolicyAction::Clamp));
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 1);
        let _ = std::fs::remove_file(&path);
    }
}
