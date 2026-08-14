//! The Shadow — the latent memory left after SVD compression.
//!
//! Compressing an `m×n` matrix by keeping `k` singular modes leaves a head and
//! a tail:
//!
//! ```text
//! A ≈ U_k·Σ_k·V_kᵀ  +  Σ_{i>k} σᵢ uᵢ vᵢᵀ      (the second term is the Shadow)
//! ```
//!
//! The Shadow is the *memory the system compressed but could not forget*: it
//! carries the structured residuals that keep influencing phase space even
//! though the visible low-rank state no longer exposes them.

use nalgebra::{DMatrix, DVector};

/// The Shadow — the tail spectrum of a truncated SVD.
#[derive(Debug, Clone)]
pub struct Shadow {
    /// Left tail basis, `m × t` (t = discarded mode count).
    pub tail_u: DMatrix<f64>,
    /// Right tail basis transposed, `t × n`.
    pub tail_vt: DMatrix<f64>,
    /// Tail singular values, length `t`.
    pub tail_singular: Vec<f64>,
    /// Fraction of the total energy carried by the tail: `Σσ²_tail / Σσ²_all`.
    pub energy_frac: f64,
    /// Index where the principal/visible cut was made.
    pub cut_rank: usize,
    /// Total number of singular values (`min(m,n)`).
    pub total_rank: usize,
}

impl Shadow {
    /// Split a full SVD into the visible `k`-rank reconstruction and the tail.
    ///
    /// `u` is the `m×r` left basis, `v` the `r×n` right-transposed basis, and
    /// `s` the length-`r` singular values. Returns `(visible, shadow)` where
    /// `visible = U_k·Σ_k·V_kᵀ`.
    pub fn split(
        u: &DMatrix<f64>,
        v: &DMatrix<f64>,
        s: &DVector<f64>,
        cut: usize,
    ) -> (DMatrix<f64>, Self) {
        let total_rank = s.len();
        let k = cut.min(total_rank);
        let tail_len = total_rank - k;

        // Principal (visible) part.
        let u_k = u.columns(0, k).into_owned();
        let v_k = v.rows(0, k).into_owned();
        let s_k = DVector::from_iterator(k, s.iter().take(k).copied());
        let diag_k = DMatrix::from_diagonal(&s_k);
        let visible = &u_k * &diag_k * &v_k;

        // Tail (shadow) part.
        let tail_u = if tail_len > 0 {
            u.columns(k, tail_len).into_owned()
        } else {
            DMatrix::zeros(u.nrows(), 0)
        };
        let tail_vt = if tail_len > 0 {
            v.rows(k, tail_len).into_owned()
        } else {
            DMatrix::zeros(0, v.ncols())
        };
        let tail_singular: Vec<f64> = s.iter().skip(k).copied().collect();

        let total_energy: f64 = s.iter().map(|&x| x * x).sum();
        let tail_energy: f64 = tail_singular.iter().map(|&x| x * x).sum();
        let energy_frac = if total_energy > 0.0 {
            tail_energy / total_energy
        } else {
            0.0
        };

        let shadow = Self {
            tail_u,
            tail_vt,
            tail_singular,
            energy_frac,
            cut_rank: k,
            total_rank,
        };
        (visible, shadow)
    }

    /// Reconstruct the shadow as the `m×n` matrix `U_τ·Σ_τ·V_τᵀ`.
    pub fn reconstruct(&self) -> DMatrix<f64> {
        if self.tail_singular.is_empty() {
            return DMatrix::zeros(self.tail_u.nrows(), self.tail_vt.ncols());
        }
        let diag = DMatrix::from_diagonal(&DVector::from_iterator(
            self.tail_singular.len(),
            self.tail_singular.iter().copied(),
        ));
        &self.tail_u * &diag * &self.tail_vt
    }

    /// The "strength" of the shadow — the square root of its energy fraction.
    pub fn strength(&self) -> f64 {
        self.energy_frac.sqrt()
    }

    /// Whether the shadow is significant enough to be "active".
    pub fn is_active(&self, threshold: f64) -> bool {
        self.energy_frac > threshold
    }

    /// Number of tail modes.
    pub fn len(&self) -> usize {
        self.tail_singular.len()
    }

    /// The shadow is empty when every singular value was retained.
    pub fn is_empty(&self) -> bool {
        self.tail_singular.is_empty()
    }
}

/// An echo — the shadow returned by the retrocausal channel of Block 21.
#[derive(Debug, Clone)]
pub struct EchoFromShadow {
    /// Phase-time the echo was emitted.
    pub timestamp: f64,
    /// Strength of the returning correction.
    pub strength: f64,
    /// Spectrum rank the echo originates from.
    pub origin_rank: usize,
    /// The tail pattern being re-admitted.
    pub pattern: DMatrix<f64>,
}

/// The healing process: reintegrating the Shadow into the visible state.
#[derive(Debug, Clone)]
pub struct ShadowHealer {
    /// The shadow being reintegrated.
    pub shadow: Shadow,
    /// Speed at which the shadow is re-admitted (`λ`).
    pub integration_rate: f64,
    /// The original pre-compression state.
    pub original: DMatrix<f64>,
}

impl ShadowHealer {
    /// Build a healer from the original data and its extracted shadow.
    pub fn new(original: DMatrix<f64>, shadow: Shadow) -> Self {
        Self {
            shadow,
            integration_rate: 0.1,
            original,
        }
    }

    /// One integration step: fold `λ·reconstruct(shadow)` back into the state.
    pub fn step(&self) -> DMatrix<f64> {
        &self.original + self.integration_rate * &self.shadow.reconstruct()
    }

    /// "Read backward": locate the strongest tail mode (its peak origin).
    pub fn trace_origin(&self) -> (f64, usize) {
        self.shadow
            .tail_singular
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(idx, sigma)| (sigma, idx))
            .unwrap_or((0.0, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    /// A rank-`rank` structured matrix plus weak noise.
    fn structured(rows: usize, cols: usize, rank: usize, seed: u64) -> DMatrix<f64> {
        let mut rng = StdRng::seed_from_u64(seed);
        let a = DMatrix::from_fn(rows, rank, |_, _| rng.gen::<f64>() - 0.5);
        let b = DMatrix::from_fn(rank, cols, |_, _| rng.gen::<f64>() - 0.5);
        let signal = &a * &b;
        signal.map(|x| x + 0.05 * rng.gen::<f64>())
    }

    #[test]
    fn split_keeps_visible_shape_and_shadow_has_energy() {
        let data = structured(50, 40, 5, 7);
        let svd = data.clone().svd(true, true);
        let (visible, shadow) = Shadow::split(
            &svd.u.expect("u"),
            &svd.v_t.expect("v"),
            &svd.singular_values,
            5,
        );
        assert_eq!(visible.shape(), (50, 40));
        assert_eq!(shadow.tail_singular.len(), 35); // total_rank(40) - cut(5)
        assert!(shadow.energy_frac > 0.0 && shadow.energy_frac < 1.0);
    }

    #[test]
    fn reconstruct_is_original_for_zero_cut() {
        let data = structured(20, 20, 3, 3);
        let svd = data.clone().svd(true, true);
        let (_, shadow) = Shadow::split(
            &svd.u.expect("u"),
            &svd.v_t.expect("v"),
            &svd.singular_values,
            0,
        );
        // cut=0 => tail holds every mode => reconstruct ≈ original.
        let rebuilt = shadow.reconstruct();
        assert_eq!(rebuilt.shape(), data.shape());
        let err = (&data - &rebuilt).norm() / data.norm();
        assert!(err < 1e-9, "rebuilt err={err}");
    }

    #[test]
    fn healer_step_adds_shadow_energy_back() {
        let data = structured(40, 30, 4, 9);
        let svd = data.clone().svd(true, true);
        let (_, shadow) = Shadow::split(
            &svd.u.expect("u"),
            &svd.v_t.expect("v"),
            &svd.singular_values,
            5,
        );
        let healer = ShadowHealer::new(data.clone(), shadow);
        let healed = healer.step();
        let e0: f64 = data.iter().map(|&x| x * x).sum();
        let e1: f64 = healed.iter().map(|&x| x * x).sum();
        assert!(e1 > e0, "healing should re-add shadow energy: {e0} -> {e1}");
    }
}