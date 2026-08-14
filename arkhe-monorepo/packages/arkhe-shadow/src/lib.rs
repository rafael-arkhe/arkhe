#![deny(unsafe_code)]

pub mod shadow;

pub use shadow::{EchoFromShadow, Shadow, ShadowHealer};

use nalgebra::DMatrix;

pub const VERSION: &str = "0.1.0-ARKHE-SHADOW-2026-08-02";

/// A completed Shadow cycle over a state matrix.
#[derive(Debug, Clone)]
pub struct ShadowCycle {
    /// The full source matrix.
    pub original: DMatrix<f64>,
    /// The low-rank reconstruction from the `k` principal modes.
    pub visible: DMatrix<f64>,
    /// The extracted tail spectrum (the Shadow).
    pub shadow: Shadow,
    /// `original + λ·reconstruct(shadow)` — the reintegrated state.
    pub healed: DMatrix<f64>,
    /// Handovers triggered this cycle (reserved).
    pub handover_count: u32,
}

impl ShadowCycle {
    /// Run the full cycle: compress, capture the shadow, and heal.
    ///
    /// `cut` is the number of principal modes kept *visible*; `integration_rate`
    /// is the fraction of the Shadow re-admitted into the state per cycle.
    pub fn run(data: DMatrix<f64>, cut: usize, integration_rate: f64) -> Self {
        let svd = data.clone().svd(true, true);
        let u = svd.u.expect("full-SVD left basis available from nalgebra");
        let v = svd.v_t.expect("full-SVD right basis available from nalgebra");
        let s = svd.singular_values;

        let k = cut.min(s.len());
        let (visible, shadow) = Shadow::split(&u, &v, &s, k);

        let shadow_matrix = shadow.reconstruct();
        let healed = &data + integration_rate * &shadow_matrix;

        Self {
            original: data,
            visible,
            shadow,
            healed,
            handover_count: 0,
        }
    }

    /// Squared energy of the healed ("complete-truth") state.
    pub fn total_energy(&self) -> f64 {
        self.healed.iter().map(|&x| x * x).sum()
    }

    /// Squared energy carried by the visible (conscious) subset.
    pub fn visible_energy(&self) -> f64 {
        self.visible.iter().map(|&x| x * x).sum()
    }

    /// Squared energy attributed to the Shadow — what was temporarily forgotten.
    pub fn shadow_energy(&self) -> f64 {
        self.shadow.energy_frac * self.visible_energy()
    }
}