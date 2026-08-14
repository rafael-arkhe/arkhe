//! Error types for the resistive-MHD phase-field engine.

/// Errors surfaced by [`crate::mhd`] and [`crate::retro`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MhdError {
    /// The grid must be at least `3 × 3` to form central derivatives.
    #[error("grid too small: expected at least 3×3, got {nx}×{ny}")]
    GridTooSmall { nx: usize, ny: usize },

    /// Non-finite state (the explicit Euler exploded — raise `ν` or shrink `dt`).
    #[error("non-finite field value at ({i}, {j})")]
    NonFinite { i: usize, j: usize },

    /// A retrocausal read was requested before any echo was due.
    #[error("no echo due at t = {t:.4}")]
    NoEchoDue { t: f64 },
}

/// Largest stable explicit step for the diffusion operator `ν∇²` on a grid with
/// spacing `h`: the CFL limit `dt ≤ h² / (2ν)`.
pub fn diffusive_cfl_dt(h: f64, nu: f64) -> f64 {
    if h <= 0.0 || nu <= 0.0 {
        return f64::INFINITY;
    }
    (h * h) / (2.0 * nu)
}