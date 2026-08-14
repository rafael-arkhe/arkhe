//! RF-plane spatial detectors (CSI lateral field).

use nalgebra::{Complex, DMatrix};

/// A complex channel-state-information (CSI) matrix of the RF plane.
#[derive(Clone, Debug)]
pub struct CsiMatrix {
    /// `n × n` complex channel coefficients.
    pub matrix: DMatrix<Complex<f64>>,
}

/// Deterministic LCG so CSI construction is reproducible across runs.
fn lcg(seed: u64) -> impl FnMut() -> f64 {
    let mut s = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    move || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        // `s >> 33` spans [0, 2^31); normalize to [0, 1).
        (s >> 33) as f64 / (1u64 << 31) as f64
    }
}

fn random_matrix(n: usize, seed: u64) -> DMatrix<Complex<f64>> {
    let mut rng = lcg(seed);
    DMatrix::from_fn(n, n, |_, _| Complex::new(rng() * 2.0 - 1.0, rng() * 2.0 - 1.0))
}

impl CsiMatrix {
    /// Random `n × n` CSI matrix seeded deterministically.
    pub fn random(n: usize, seed: u64) -> Self {
        Self {
            matrix: random_matrix(n, seed),
        }
    }

    /// The matrix under an RF perturbation.
    ///
    /// Adds random channel noise of the given scale and renormalizes to the
    /// original magnitude, so a strong perturbation *decorrelates* the
    /// snapshot (a fresh channel realization) instead of slowly drifting it.
    pub fn perturbed(&self, scale: f64, seed: u64) -> Self {
        let mut rng = lcg(seed);
        let noise = DMatrix::from_fn(
            self.matrix.nrows(),
            self.matrix.ncols(),
            |_, _| Complex::new(rng() * 2.0 - 1.0, rng() * 2.0 - 1.0),
        );
        let scaled = noise * Complex::new(scale, 0.0);
        let sum = &self.matrix + &scaled;
        let target = self.matrix.norm();
        let n = sum.norm();
        let matrix = if n > 1e-12 {
            sum * Complex::new(target / n, 0.0)
        } else {
            sum
        };
        Self { matrix }
    }

    /// Normalized complex cosine similarity between two CSI matrices.
    pub fn cosine(&self, other: &CsiMatrix) -> f64 {
        if self.matrix.len() != other.matrix.len() {
            return 0.0;
        }
        let mut dot = Complex::new(0.0, 0.0);
        let mut na = 0.0;
        let mut nb = 0.0;
        for (x, y) in self.matrix.iter().zip(other.matrix.iter()) {
            dot += x.conj() * y;
            na += x.norm_sqr();
            nb += y.norm_sqr();
        }
        if na <= 1e-12 || nb <= 1e-12 {
            return 0.0;
        }
        (dot.re / (na * nb).sqrt()).clamp(0.0, 1.0)
    }
}

/// Sliding lateral field of CSI snapshots with a live integrity measure.
///
/// `integrity` is an EWMA of the cosine similarity between consecutive
/// snapshots: `1.0` under a stable RF plane, dropping toward `0.0` once an RF
/// perturbation decorrelates the channel.
#[derive(Clone, Debug)]
pub struct CsiLateralField {
    matrices: Vec<CsiMatrix>,
    capacity: usize,
    integrity: f64,
}

impl CsiLateralField {
    /// Empty field with the given snapshot capacity, integrity `1.0`.
    pub fn new(capacity: usize) -> Self {
        Self {
            matrices: Vec::with_capacity(capacity),
            capacity,
            integrity: 1.0,
        }
    }

    /// Current lateral integrity, `[0, 1]`.
    pub fn integrity(&self) -> f64 {
        self.integrity
    }

    /// Number of snapshots held.
    pub fn len(&self) -> usize {
        self.matrices.len()
    }

    /// Whether the field has no snapshots.
    pub fn is_empty(&self) -> bool {
        self.matrices.is_empty()
    }

    /// Push a new CSI snapshot and update the integrity EWMA.
    pub fn update(&mut self, snapshot: CsiMatrix) {
        if let Some(prev) = self.matrices.last() {
            self.integrity = 0.5 * self.integrity + 0.5 * prev.cosine(&snapshot);
        }
        self.matrices.push(snapshot);
        while self.matrices.len() > self.capacity {
            self.matrices.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_rf_plane_keeps_integrity() {
        let mut field = CsiLateralField::new(8);
        let base = CsiMatrix::random(4, 100);
        for _ in 0..6 {
            field.update(base.clone());
        }
        assert!(
            field.integrity() >= 0.9,
            "stable snapshots should stay coherent, got {}",
            field.integrity()
        );
    }

    #[test]
    fn rf_perturbation_drops_integrity_below_half() {
        let mut field = CsiLateralField::new(8);
        let base = CsiMatrix::random(4, 42);
        for _ in 0..4 {
            field.update(base.clone());
        }
        assert!(field.integrity() >= 0.95);
        let mut p = base.perturbed(3.0, 7);
        for k in 0..6 {
            p = p.perturbed(3.0, 1000 + k);
            field.update(p.clone());
        }
        assert!(
            field.integrity() < 0.5,
            "perturbed field must lose integrity, got {}",
            field.integrity()
        );
    }

    #[test]
    fn identical_matrices_are_unit_cosine() {
        let a = CsiMatrix::random(3, 1);
        assert!((a.cosine(&a) - 1.0).abs() < 1e-9);
    }
}
