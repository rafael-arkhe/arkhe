//! Error type shared by the temporal safety layer.

use crate::ArousalRegime;

/// Errors of the cyber-physical safety layer.
#[derive(Debug, thiserror::Error)]
pub enum RecurrencyError {
    /// The current arousal regime forbids the operation (e.g. `Coma` rejects
    /// `tick_correct` because the global-access threshold is unreachable).
    #[error("regime {0:?} forbids this operation")]
    RegimeForbidden(ArousalRegime),
    /// A network anomaly was detected for the given MAC / aggregate.
    #[error("network anomaly: {0}")]
    NetworkAnomaly(String),
    /// The lateral (spatial) geometry integrity fell below the gate.
    #[error("spatial anomaly: lateral integrity {0:.3} below gate")]
    SpatialAnomaly(f64),
    /// The temporal history was found inconsistent (dimension mismatch etc.).
    #[error("inconsistent temporal state: {0}")]
    InconsistentTemporalState(String),
    /// The modulation event stream was dropped.
    #[error("modulation stream closed")]
    StreamClosed,
    /// Underlying I/O failure (webhook bind, HTTP transport).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// HTTP/transport failure while notifying a webhook.
    #[error("transport: {0}")]
    Transport(String),
}
