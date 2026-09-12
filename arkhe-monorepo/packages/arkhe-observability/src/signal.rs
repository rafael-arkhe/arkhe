//! Telegraph ξM-field signal types and topic constants.
//!
//! These mirror the wire contract implemented by the installed runtime
//! (`telegraph.js`): a [`Signal`] is the atom of the bus, and the topics are
//! the channels the Cathedral v26.5 dashboard subscribes to.

use serde::Serialize;

/// Topics published by the Cathedral v26.5 dashboard.
pub mod topic {
    /// Dashboard control channel (bidirectional).
    pub const CONTROL: &str = "/cathedral/control";
    /// Cathedral v26.5 runtime statistics.
    pub const STATS: &str = "/cathedral/stats";
    /// Attestation / security events.
    pub const EVENTS: &str = "/cathedral/events";
    /// Global coherence parameter.
    pub const SIGNAL_PHI: &str = "/signal/phi";
    /// Dynamic System Analysis (DSA) order parameter — r_DSA.
    pub const COHERENCE_DSA: &str = "/coherence/dsa";
}

/// Source tag used by the observability bridge on every signal it emits.
pub const SOURCE: &str = "arkhe-observability";

/// One atom of the ξM-field bus, as `telegraph.js` serialises it.
///
/// The seal is computed server-side on publish, so a client only needs to
/// provide `source`, `topic`, `metric`, `value`, `unit` and `timestamp`.
#[derive(Debug, Clone, Serialize)]
pub struct Signal {
    pub source: String,
    pub topic: String,
    pub metric: String,
    pub value: serde_json::Value,
    pub unit: String,
    pub timestamp: String,
}

impl Signal {
    /// Build a signal tagged with the observability bridge as its source.
    pub fn new(topic: &str, metric: &str, value: serde_json::Value, unit: &str) -> Self {
        Self {
            source: SOURCE.to_string(),
            topic: topic.to_string(),
            metric: metric.to_string(),
            value,
            unit: unit.to_string(),
            timestamp: chrono_rfc3339_now(),
        }
    }
}

/// RFC 3339 timestamp for the signal envelope.
///
/// Uses the system clock (Gravity-1 monotonicity is enforced by the bus, which
/// stamps on arrival); the `timestamp` field is informational for the ledger.
pub fn chrono_rfc3339_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let millis = now.subsec_millis();
    // Decompose the unix epoch into an RFC 3339 date (days -> civil date).
    let (date, hh, mm, ss) = civil_from_days(secs as i64 / 86_400, (secs % 86_400) as u32);
    format!("{date}T{hh:02}:{mm:02}:{ss:02}.{millis:03}Z")
}

/// Convert days-since-epoch plus seconds-of-day into `YYYY-MM-DD`, `HH`, `MM`, `SS`.
fn civil_from_days(days: i64, sod: u32) -> (String, u32, u32, u32) {
    // Howard Hinnant's civil-from-days algorithm (public domain).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let hh = sod / 3600;
    let mm = (sod % 3600) / 60;
    let ss = sod % 60;
    (format!("{y:04}-{m:02}-{d:02}"), hh, mm, ss)
}
