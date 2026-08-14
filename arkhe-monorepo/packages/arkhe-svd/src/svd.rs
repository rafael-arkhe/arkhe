//! Truncated SVD compression over `nalgebra` dense matrices.
//!
//! Deterministic, pure-Rust (no external BLAS): the Singular Value
//! Decomposition is computed by `nalgebra`'s built-in solver, which keeps the
//! crate compilable on any target, including Windows without an LAPACK stack.

use nalgebra::{DMatrix, DVector};

use crate::error::SvdError;

/// Convex-linking threshold for the frame selector: a batch is deemed
/// "coherent" when σ₁/σ₂ exceeds √π.
const COHERENCE_RATIO: f64 = 1.772_453_850_905_516; // π.sqrt()

/// A matrix compressed into its truncated SVD factorisation `U·Σ·Vᵀ`.
///
/// The `m×n` input is stored as `U` (`m×k`), the singular values `Σ` (`k`),
/// and `Vᵀ` (`k×n`), keeping only the `k` dominant modes.
#[derive(Debug, Clone, PartialEq)]
pub struct SvdCompressed {
    /// Original row count.
    pub m: usize,
    /// Original column count.
    pub n: usize,
    /// Effective rank kept.
    pub k: usize,
    /// Left singular vectors, `m×k`.
    pub u: DMatrix<f64>,
    /// Singular values, length `k`.
    pub sigma: DVector<f64>,
    /// Right singular vectors transposed, `k×n`.
    pub vt: DMatrix<f64>,
    /// Fraction of the Frobenius energy retained.
    pub energy_retained: f64,
}

impl SvdCompressed {
    /// Truncate `data` to its `k` dominant singular modes.
    ///
    /// `k` is clamped to `min(m, n)`. Returns [`SvdError`] if the matrix has
    /// no signal (zero energy) or fewer than one usable mode.
    pub fn compress(data: &DMatrix<f64>, k: usize) -> Result<Self, SvdError> {
        let (m, n) = data.shape();
        let k = k.min(m).min(n);
        if k == 0 {
            return Err(SvdError::InvalidDimensions {
                expected: (m.min(n), m.min(n)),
                got: (0, 0),
            });
        }

        let svd = data.clone().svd(true, true);
        let u_full = svd.u.ok_or(SvdError::DecompositionFailed)?;
        let v_full = svd.v_t.ok_or(SvdError::DecompositionFailed)?;
        let s = svd.singular_values;

        let total_energy: f64 = s.iter().map(|&x| x * x).sum();
        if total_energy == 0.0 {
            return Err(SvdError::ZeroEnergy);
        }
        let retained_energy: f64 = s.iter().take(k).map(|&x| x * x).sum();

        Ok(SvdCompressed {
            m,
            n,
            k,
            u: u_full.columns(0, k).into_owned(),
            sigma: s.rows(0, k).into_owned(),
            vt: v_full.rows(0, k).into_owned(),
            energy_retained: retained_energy / total_energy,
        })
    }

    /// Reconstruct the `m×n` matrix as `U·Σ·Vᵀ`.
    pub fn reconstruct(&self) -> DMatrix<f64> {
        let diag = DMatrix::from_diagonal(&self.sigma);
        &self.u * &diag * &self.vt
    }

    /// Serialize to a compact byte vector.
    ///
    /// Layout: `[m:u32][n:u32][k:u32][energy:f64][U(m*k×f64)][Σ(k×f64)][Vᵀ(k*n×f64)]`.
    /// `energy_retained` is stored in the header so `from_bytes` round-trips
    /// losslessly instead of using a placeholder.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(20 + (self.u.len() + self.sigma.len() + self.vt.len()) * 8);
        bytes.extend_from_slice(&(self.m as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.n as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.k as u32).to_le_bytes());
        bytes.extend_from_slice(&self.energy_retained.to_le_bytes());

        for i in 0..self.m {
            for j in 0..self.k {
                bytes.extend_from_slice(&self.u[(i, j)].to_le_bytes());
            }
        }
        for i in 0..self.k {
            bytes.extend_from_slice(&self.sigma[i].to_le_bytes());
        }
        for i in 0..self.k {
            for j in 0..self.n {
                bytes.extend_from_slice(&self.vt[(i, j)].to_le_bytes());
            }
        }
        bytes
    }

    /// Recover a [`SvdCompressed`] from [`to_bytes`](SvdCompressed::to_bytes).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SvdError> {
        if bytes.len() < 20 {
            return Err(SvdError::SerializationError(
                "buffer too small for header".to_string(),
            ));
        }
        let m = u32::from_le_bytes(bytes[0..4].try_into().expect("4 bytes")) as usize;
        let n = u32::from_le_bytes(bytes[4..8].try_into().expect("4 bytes")) as usize;
        let k = u32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes")) as usize;
        let energy_retained = f64::from_le_bytes(bytes[12..20].try_into().expect("8 bytes"));

        let expected = 20 + (m * k + k + k * n) * 8;
        if bytes.len() != expected {
            return Err(SvdError::SerializationError(format!(
                "expected {expected} bytes, got {}",
                bytes.len()
            )));
        }

        let mut u = DMatrix::zeros(m, k);
        let mut offset = 20;
        for i in 0..m {
            for j in 0..k {
                let chunk: [u8; 8] = bytes[offset..offset + 8].try_into().expect("8 bytes");
                u[(i, j)] = f64::from_le_bytes(chunk);
                offset += 8;
            }
        }

        let mut sigma = DVector::zeros(k);
        for i in 0..k {
            let chunk: [u8; 8] = bytes[offset..offset + 8].try_into().expect("8 bytes");
            sigma[i] = f64::from_le_bytes(chunk);
            offset += 8;
        }

        let mut vt = DMatrix::zeros(k, n);
        for i in 0..k {
            for j in 0..n {
                let chunk: [u8; 8] = bytes[offset..offset + 8].try_into().expect("8 bytes");
                vt[(i, j)] = f64::from_le_bytes(chunk);
                offset += 8;
            }
        }

        Ok(SvdCompressed {
            m,
            n,
            k,
            u,
            sigma,
            vt,
            energy_retained,
        })
    }

    /// Relative reconstruction error in the Frobenius norm of `original`.
    pub fn relative_error(&self, original: &DMatrix<f64>) -> Result<f64, SvdError> {
        if original.shape() != (self.m, self.n) {
            return Err(SvdError::InvalidDimensions {
                expected: (self.m, self.n),
                got: original.shape(),
            });
        }
        let diff = original - self.reconstruct();
        let error = diff.norm();
        let norm = original.norm();
        if norm == 0.0 {
            return Err(SvdError::ZeroEnergy);
        }
        Ok(error / norm)
    }
}

/// Compress an EVO/state matrix and return the reconstructed (decompressed)
/// matrix directly.
pub fn compress_evo(evo_matrix: &DMatrix<f64>, k: usize) -> Result<DMatrix<f64>, SvdError> {
    let compressed = SvdCompressed::compress(evo_matrix, k)?;
    Ok(compressed.reconstruct())
}

/// Return the frame index judged most coherent in a batch.
///
/// Frames are stacked as rows of an `n_frames × frame_len` matrix whose SVD
/// singular spectrum is inspected: when the dominant mode dominates (σ₁/σ₂ > √π)
/// the batch is coherent and frame 0 (the dominant-mode direction) is returned;
/// otherwise the last frame is returned as the "no strong gain" sentinel.
pub fn svd_frame_selector(frames: &[DVector<f64>]) -> Result<usize, SvdError> {
    if frames.is_empty() {
        return Err(SvdError::EmptyBatch);
    }
    let n_frames = frames.len();
    let frame_len = frames[0].len();
    if frame_len == 0 {
        return Err(SvdError::EmptyBatch);
    }

    let mut mat = DMatrix::zeros(n_frames, frame_len);
    for (i, frame) in frames.iter().enumerate() {
        if frame.len() != frame_len {
            return Err(SvdError::InvalidDimensions {
                expected: (frame_len, 1),
                got: (frame.len(), 1),
            });
        }
        for (j, &val) in frame.iter().enumerate() {
            mat[(i, j)] = val;
        }
    }

    let singular = mat.svd(false, false).singular_values;
    if singular.len() < 2 || singular[1].abs() <= 1e-15 {
        // Rank-deficient or effectively rank-1 batch: fully coherent.
        return Ok(0);
    }
    let ratio = singular[0] / singular[1];
    if ratio > COHERENCE_RATIO {
        Ok(0)
    } else {
        Ok(n_frames - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    fn random_matrix(rows: usize, cols: usize, seed: u64) -> DMatrix<f64> {
        let mut rng = StdRng::seed_from_u64(seed);
        DMatrix::from_fn(rows, cols, |_, _| rng.gen::<f64>())
    }

    #[test]
    fn svd_compression_roundtrip() {
        let data = random_matrix(50, 30, 1);
        let compressed = SvdCompressed::compress(&data, 10).expect("compress");
        assert_eq!(compressed.k, 10);
        assert!(compressed.energy_retained > 0.0 && compressed.energy_retained <= 1.0);
        let reconstructed = compressed.reconstruct();
        assert_eq!(reconstructed.shape(), (50, 30));
        let rel = compressed.relative_error(&data).expect("error");
        assert!(rel < 0.5, "relative error too high: {rel}");
    }

    #[test]
    fn svd_serialization_roundtrip() {
        let data = random_matrix(20, 15, 7);
        let compressed = SvdCompressed::compress(&data, 5).expect("compress");
        let bytes = compressed.to_bytes();
        let recovered = SvdCompressed::from_bytes(&bytes).expect("parse");
        assert_eq!(compressed.m, recovered.m);
        assert_eq!(compressed.n, recovered.n);
        assert_eq!(compressed.k, recovered.k);
        assert!((compressed.u - recovered.u).norm() < 1e-9);
        assert!((compressed.sigma - recovered.sigma).norm() < 1e-9);
        assert!((compressed.vt - recovered.vt).norm() < 1e-9);
        assert!((compressed.energy_retained - recovered.energy_retained).abs() < 1e-12);
    }

    #[test]
    fn svd_image_rank_one_gradient() {
        // (i/100)*(j/100) is rank 1, so 5 modes reconstruct almost exactly.
        let mut img = DMatrix::zeros(100, 100);
        for i in 0..100 {
            for j in 0..100 {
                img[(i, j)] = (i as f64 / 100.0) * (j as f64 / 100.0);
            }
        }
        let comp = SvdCompressed::compress(&img, 5).expect("compress");
        let err = comp.relative_error(&img).expect("error");
        assert!(err < 1e-10, "rank-one image compress shouldn't err, got {err}");
    }

    #[test]
    fn compress_evo_preserves_shape() {
        let evo = random_matrix(10, 10, 3);
        let rec = compress_evo(&evo, 5).expect("compress_evo");
        assert_eq!(rec.shape(), (10, 10));
    }

    #[test]
    fn frame_selector_rejects_empty_batch() {
        let frames: Vec<DVector<f64>> = Vec::new();
        assert_eq!(svd_frame_selector(&frames), Err(SvdError::EmptyBatch));
    }

    #[test]
    fn frame_selector_degenerate_batch_is_coherent() {
        // All frames identical => rank 1 => must not panic and reports mode 0.
        let f = DVector::from_fn(8, |i, _| (i as f64) * 0.5);
        let frames = vec![f.clone(), f.clone()];
        assert_eq!(svd_frame_selector(&frames).expect("selector"), 0);
    }

    #[test]
    fn frame_selector_identity_batch_not_coherent() {
        // Identically-zero frames => zero signal; singular[1]~0 => treated as
        // rank-deficient => coherent (0), not a panic.
        let f0 = DVector::zeros(8);
        let frames = vec![f0.clone(), f0.clone()];
        assert_eq!(svd_frame_selector(&frames).expect("selector"), 0);
    }
}