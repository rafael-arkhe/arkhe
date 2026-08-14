//! The retrocausal echo layer of the Timechain.
//!
//! Each node runs its own `EvoField` and emits an **echo** — a compressed
//! prediction of the field's future topology (`delta_h`) carried at a speed
//! governed by the `v_eco(k)` dispersion relation of the turbulence spectrum.
//!
//! ```text
//! v_eco(k) = v_ref · (k_ref / k)^{1/3}
//! ```
//!
//! This is a one-third-power (Kolmogorov-like) dispersion: high-wavenumber
//! (small-scale, strong-topology) structures move *slower*, so distant echoes
//! are signalled by the large-scale, slow-drifting modes the Shadow retains.

use serde::{Deserialize, Serialize};

/// One-third Kolmogorov power of the dispersion relation.
const DISPERSION_EXP: f64 = 1.0 / 3.0;

/// The `v_eco(k)` dispersion relation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Dispersion {
    /// Reference propagation speed at the reference wavenumber `k_ref`.
    pub v_ref: f64,
    /// Reference wavenumber (units of inverse length).
    pub k_ref: f64,
}

impl Dispersion {
    /// Build the dispersion relation.
    pub fn new(v_ref: f64, k_ref: f64) -> Self {
        Self { v_ref, k_ref }
    }

    /// Echo propagation speed at wavenumber `k`.
    ///
    /// Larger `k` (smaller scales) propagate slower — `v ∝ k^{-1/3}`.
    pub fn velocity(&self, k: f64) -> f64 {
        if k <= 0.0 {
            return self.v_ref;
        }
        self.v_ref * (self.k_ref / k).powf(DISPERSION_EXP)
    }

    /// One-way propagation time over `distance` at wavenumber `k`.
    pub fn one_way_delay(&self, k: f64, distance: f64) -> f64 {
        distance / self.velocity(k)
    }

    /// Round-trip echo delay for a retrocausal confirmation.
    pub fn round_trip_delay(&self, k: f64, distance: f64) -> f64 {
        2.0 * self.one_way_delay(k, distance)
    }
}

/// A compressed prediction a node shares with the peer network.
///
/// The echo is `bincode`/`serde`-serialisable so it can be forwarded over UDP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EchoSignal {
    /// Which node produced the echo.
    pub node_id: u64,
    /// Block height the echo refers to.
    pub height: u64,
    /// Phase-clock time the echo was emitted.
    pub emitted_at: f64,
    /// Height the echo originates from (normally the same as `height`).
    pub origin_height: u64,
    /// Representative wavenumber of the carried mode.
    pub wavenumber: f64,
    /// Chern–Simons phase of the predicted topology (radians).
    pub phase: f64,
    /// Predicted change in helicity across the block.
    pub predicted_delta_h: f64,
    /// Strength of the echo — the trailing Shadow's energy fraction.
    pub strength: f64,
    /// The tail-singular pattern being re-admitted.
    pub pattern: Vec<f64>,
}

impl EchoSignal {
    /// Assemble a new echo.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        node_id: u64,
        height: u64,
        origin_height: u64,
        emitted_at: f64,
        wavenumber: f64,
        phase: f64,
        predicted_delta_h: f64,
        strength: f64,
        pattern: Vec<f64>,
    ) -> Self {
        Self {
            node_id,
            height,
            emitted_at,
            origin_height,
            wavenumber,
            phase,
            predicted_delta_h,
            strength,
            pattern,
        }
    }

    /// Travel time for this echo to a peer a distance `d` away.
    pub fn travel_time(&self, dispersion: &Dispersion, distance: f64) -> f64 {
        dispersion.one_way_delay(self.wavenumber, distance)
    }

    /// Virtual arrival clock time.
    pub fn arrival_at(&self, dispersion: &Dispersion, distance: f64) -> f64 {
        self.emitted_at + self.travel_time(dispersion, distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispersion_slows_high_wavenumber() {
        let d = Dispersion::new(2.5, 4.0);
        let v_lo = d.velocity(1.0);
        let v_hi = d.velocity(64.0);
        assert!(v_hi < v_lo, "high-k should be slower: {v_hi} < {v_lo}");
    }

    #[test]
    fn one_third_power_holds() {
        let d = Dispersion::new(3.0, 8.0);
        let v = d.velocity(8.0);
        assert!((v - 3.0).abs() < 1e-9, "at k == k_ref speed must equal v_ref: {v}");
    }
}