//! Error type for the SVD compression protocol.

/// Errors surfaced by [`crate::svd`] operations.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SvdError {
    /// The singular value decomposition could not be produced (e.g. the
    /// truncated basis was not returned by the backend).
    #[error("SVD decomposition failed")]
    DecompositionFailed,

    /// The supplied matrix/matrices had incompatible dimensions.
    #[error("invalid dimensions: expected {expected:?}, got {got:?}")]
    InvalidDimensions {
        expected: (usize, usize),
        got: (usize, usize),
    },

    /// The matrix carries no exploitable signal (all singular values zero).
    #[error("zero total energy: no exploitable signal")]
    ZeroEnergy,

    /// A binary payload could not be parsed.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// The frame batch was empty.
    #[error("empty frame batch")]
    EmptyBatch,
}