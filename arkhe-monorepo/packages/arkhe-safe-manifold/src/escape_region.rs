//! Escape regions for the ARKHE‑χ security manifold (16 invariants).
//!
//! Regions classify the severity of invariant violations using a monotonic
//! scale based on [`SystemState::violation_count`].

use serde::{Deserialize, Serialize};

/// Severity region of a system state relative to the safe envelope.
///
/// The classification is monotonic: more violations → more severe region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscapeRegion {
    /// All 16 invariants satisfied.
    Safe,
    /// Exactly 1 invariant violated — early warning.
    Warning,
    /// Exactly 2 invariants violated — degraded but recoverable.
    Boundary,
    /// 3–5 invariants violated — critical, likely unsafe.
    Continuum,
    /// 6+ invariants violated — outside safe envelope.
    Outside,
}
