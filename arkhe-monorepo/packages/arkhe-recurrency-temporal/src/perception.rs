//! Perception and action identifiers consumed by the temporal daemon.

/// Identifier of a concrete action the system performed.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ActionId(pub String);

/// A completed perception–action cycle, graded by how well reality clamped it.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Perception {
    /// The action that closed the loop.
    pub action_id: ActionId,
    /// Perceptual delta (embedding-size); a zero vector here means "no echo".
    pub delta: Vec<f64>,
    /// Latency of the cycle in milliseconds.
    pub latency_ms: u64,
    /// Integrity of the perception. `0.0` = hostile/unclamped (attack).
    pub integrity: f64,
}
