//! Errors of the THz sensor domain.

/// Errors raised by the THz sensor model and its engine integration.
#[derive(Debug, thiserror::Error)]
pub enum ThzSensorError {
    /// A frequency outside the valid simulation band was requested.
    #[error("frequency {0} THz is outside the valid 0.5–12.0 THz band")]
    FrequencyOutOfRange(f64),
    /// The substrate dimension of the `RecurrencyEngine` differs from the
    /// sensor feature dimension.
    #[error("engine substrate dimension {engine} != sensor feature dimension {sensor}")]
    DimensionMismatch {
        /// Dimension of the engine's `PerceptionLoop`.
        engine: usize,
        /// Sensor feature dimension ([`crate::FEATURE_DIM`]).
        sensor: usize,
    },
}
