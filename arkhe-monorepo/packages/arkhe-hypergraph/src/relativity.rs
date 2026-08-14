//! Special-relativity transforms for the S03 (CTC Geometry) portal lane.
//!
//! Pure-math, dependency-free Lorentz kinematics: proper-time dilation,
//! length contraction, relativity of simultaneity, 1D frame transforms, and
//! the Minkowski invariant. This module is genuinely verifiable: it is unit
//! tested against closed-form γ = 1/√(1−β²) and the invariant-preservation
//! identity.
//!
//! # Honesty notes
//! - `ctc_possible` / `wormhole_effective_velocity` are **modelling
//!   conventions, not physics**: SR forbids v ≥ c, so treating δ>1 as a
//!   "superluminal wormhole regime" is a fictional extension of this theory.
//!   They are kept out of `PortalGunHypergraph` and marked explicitly.
//! - The accompanying Lean 4 "formal proof" in the spec is NOT load-bearing:
//!   it is `sorry`-stubbed and we have no Lean toolchain on this host to check
//!   it, so this crate does not claim a machine-checked proof.

use serde::{Deserialize, Serialize};

/// Speed of light in m/s.
pub const C: f64 = 299_792_458.0;

/// Error for velocities at or above `c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuperluminalError;

impl std::fmt::Display for SuperluminalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "velocity must satisfy |v| < c")
    }
}

impl std::error::Error for SuperluminalError {}

/// Result of a 1D Lorentz transform.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LorentzTransform {
    /// Lorentz factor γ = 1/√(1−β²).
    pub gamma: f64,
    /// Dimensionless velocity β = v/c.
    pub beta: f64,
}

impl LorentzTransform {
    /// Build a transform from an absolute velocity `v` (m/s).
    pub fn new(v: f64) -> Result<Self, SuperluminalError> {
        Self::from_beta(v / C)
    }

    /// Build a transform from a dimensionless velocity `beta = v/c`.
    pub fn from_beta(beta: f64) -> Result<Self, SuperluminalError> {
        if beta.abs() >= 1.0 {
            return Err(SuperluminalError);
        }
        let gamma = 1.0 / (1.0 - beta * beta).sqrt();
        Ok(Self { gamma, beta })
    }

    /// Δt = γ·Δτ — coordinate time from proper time.
    pub fn dilate_time(&self, proper_time: f64) -> f64 {
        self.gamma * proper_time
    }

    /// Δτ = Δt/γ — proper time from coordinate time.
    pub fn proper_from_coord(&self, coord_time: f64) -> f64 {
        coord_time / self.gamma
    }

    /// L = L₀/γ — observed length contraction.
    pub fn contract_length(&self, rest_length: f64) -> f64 {
        rest_length / self.gamma
    }

    /// L₀ = γ·L — rest length back.
    pub fn rest_from_contracted(&self, contracted_length: f64) -> f64 {
        self.gamma * contracted_length
    }

    /// Δt = γ·β·Δx' / c — simultaneity gap.
    pub fn simultaneity_gap(&self, delta_x_prime: f64) -> f64 {
        self.gamma * self.beta * delta_x_prime
    }

    /// (t', x') → (t, x): t = γ(t' + vx'/c²), x = γ(x' + vt').
    pub fn transform_to_rest(&self, t_prime: f64, x_prime: f64) -> (f64, f64) {
        let t = self.gamma * (t_prime + self.beta * x_prime / C);
        let x = self.gamma * (x_prime + self.beta * C * t_prime);
        (t, x)
    }

    /// (t, x) → (t', x'): t' = γ(t − vx/c²), x' = γ(x − vt).
    pub fn transform_to_moving(&self, t: f64, x: f64) -> (f64, f64) {
        let t_prime = self.gamma * (t - self.beta * x / C);
        let x_prime = self.gamma * (x - self.beta * C * t);
        (t_prime, x_prime)
    }

    /// Minkowski interval s² = −(cΔt)² + (Δx)².
    pub fn minkowski_interval(&self, dt: f64, dx: f64) -> f64 {
        -(C * dt).powi(2) + dx.powi(2)
    }

    /// Assert the frame transform composed with its inverse is the identity
    /// to a tolerance matched to the largest operand magnitude (defaults to
    /// ~1e8 for modest t, so an absolute bound would be meaningless).
    pub fn round_trip(&self, t: f64, x: f64) -> bool {
        let (tp, xp) = self.transform_to_moving(t, x);
        let (tb, xb) = self.transform_to_rest(tp, xp);
        let scale = (C * t).abs().max(1.0);
        (tb - t).abs() < 1e-9 * scale && (xb - x).abs() < 1e-9 * scale
    }

    /// Whether the interval is preserved to a relative tolerance.
    pub fn verify_invariant(&self, t: f64, x: f64, tolerance: f64) -> bool {
        let s_rest = self.minkowski_interval(t, x);
        let (tp, xp) = self.transform_to_moving(t, x);
        let s_moving = self.minkowski_interval(tp, xp);
        let scale = (s_rest.abs() + s_moving.abs()).max(1e-12);
        (s_rest - s_moving).abs() / scale < tolerance
    }

    /// Human-readable summary.
    pub fn summary(&self) -> String {
        format!(
            "Lorentz(β={:.6}, γ={:.6}) | v={:.0} m/s",
            self.beta,
            self.gamma,
            self.beta * C
        )
    }
}

/// Rows of (v, γ, Δt) for a proper-time interval of 1 s.
pub fn time_dilation_table() -> Vec<(f64, f64, f64)> {
    let fractions = [0.0, 0.1, 0.5, 0.8, 0.866, 0.9, 0.95, 0.99, 0.999, 0.9999];
    fractions
        .iter()
        .filter_map(|&f| LorentzTransform::from_beta(f).ok())
        .map(|lt| (lt.beta * C, lt.gamma, lt.dilate_time(1.0)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lorentz_factor_0_5c() {
        let lt = LorentzTransform::from_beta(0.5).unwrap();
        let expected = 1.0 / (1.0f64 - 0.25).sqrt();
        assert!((lt.gamma - expected).abs() < 1e-12);
        assert!((lt.beta - 0.5).abs() < 1e-12);
    }

    #[test]
    fn lorentz_factor_0_866c_is_2() {
        let lt = LorentzTransform::from_beta(0.866).unwrap();
        assert!((lt.gamma - 2.0).abs() < 0.01);
        assert!((lt.dilate_time(1.0) - 2.0).abs() < 0.01);
    }

    #[test]
    fn length_contraction_roundtrip() {
        let lt = LorentzTransform::from_beta(0.6).unwrap();
        let rest = 100.0;
        let contracted = lt.contract_length(rest);
        assert!(contracted < rest);
        assert!((lt.rest_from_contracted(contracted) - rest).abs() < 1e-9);
    }

    #[test]
    fn minkowski_interval_preserved() {
        let lt = LorentzTransform::from_beta(0.5).unwrap();
        assert!(lt.round_trip(10.0, 0.0));
        assert!(lt.round_trip(3.0, 4.0));
        assert!(lt.verify_invariant(10.0, 0.0, 1e-9));
        assert!(lt.verify_invariant(3.0, 4.0, 1e-9));
    }

    #[test]
    fn lightlike_stays_lightlike() {
        // A null interval (cΔt = Δx) stays null under the boost. Cancelling
        // terms are ~(c·t)² ≈ 6e16; the residue must be tiny relative to that.
        let lt = LorentzTransform::from_beta(0.8).unwrap();
        let term_mag = (C * 1.0).powi(2); // ~6e16
        let s_rest = lt.minkowski_interval(1.0, C);
        let (tp, xp) = lt.transform_to_moving(1.0, C);
        let s_moving = lt.minkowski_interval(tp, xp);
        assert!(s_rest.abs() < 1e-6 * term_mag);
        assert!(s_moving.abs() < 1e-6 * term_mag);
    }

    #[test]
    fn rejects_superluminal() {
        assert!(LorentzTransform::from_beta(1.0).is_err());
        assert!(LorentzTransform::from_beta(1.5).is_err());
        assert!(LorentzTransform::new(C).is_err());
        assert!(LorentzTransform::new(C + 1.0).is_err());
    }

    #[test]
    fn simultaneity_gap() {
        let lt = LorentzTransform::from_beta(0.8).unwrap();
        assert!(lt.simultaneity_gap(1.0).abs() > 0.0);
        let lt_rest = LorentzTransform::from_beta(0.0).unwrap();
        assert!(lt_rest.simultaneity_gap(1.0).abs() < 1e-12);
    }

    #[test]
    fn table_monotonic() {
        let table = time_dilation_table();
        assert_eq!(table.len(), 10);
        assert!((table[0].2 - 1.0).abs() < 1e-12);
        // γ strictly increases with β; it diverges toward 1.
        for w in table.windows(2) {
            assert!(w[1].1 > w[0].1, "γ must increase with v");
        }
    }
}