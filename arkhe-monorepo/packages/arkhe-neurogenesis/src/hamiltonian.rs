use nalgebra::{DMatrix, DVector};

/// Amplitude-coupling scale applied to anti-symmetric coupling entries.
const COUPLING_SCALE: f64 = 1.0;

/// Gain/loss scale applied to the diagonal gain/loss vector.
const GAIN_LOSS_SCALE: f64 = 0.1;

/// Deterministic 64-bit linear congruential generator (glibc-style constants).
///
/// Shared crate-internally so the Hamiltonian, the initial state and the
/// dendritic growth all draw from reproducible streams.
#[derive(Clone, Debug)]
pub(crate) struct Lcg(u64);

impl Lcg {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0
    }

    /// Uniform sample in `[0, 1)`.
    pub(crate) fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform sample in `[-scale, scale)`.
    pub(crate) fn signed(&mut self, scale: f64) -> f64 {
        (self.unit() * 2.0 - 1.0) * scale
    }
}

/// Non-Hermitian generator `H = C + D` of the amplitude evolution.
///
/// * `coupling` — real anti-symmetric matrix `C` (`C[i][j] = -C[j][i]`,
///   zero diagonal): conservative transport of amplitude between sites.
/// * `gain_loss` — diagonal rates `g`: positive entries amplify a site,
///   negative entries decay it.
pub struct Hamiltonian {
    pub coupling: DMatrix<f64>,
    pub gain_loss: DVector<f64>,
}

impl Hamiltonian {
    /// Build an `n`-site anti-symmetric coupling matrix and a gain/loss vector,
    /// both drawn deterministically from `seed`.
    pub fn antisymmetric(n: usize, seed: u64) -> Self {
        let mut lcg = Lcg::new(seed);
        let mut coupling = DMatrix::zeros(n, n);
        for i in 0..n {
            for j in (i + 1)..n {
                let v = lcg.signed(COUPLING_SCALE);
                coupling[(i, j)] = v;
                coupling[(j, i)] = -v;
            }
        }
        let mut gain = Lcg::new(seed ^ 0x5F5F);
        let gain_loss = DVector::from_fn(n, |_, _| gain.signed(GAIN_LOSS_SCALE));
        Self { coupling, gain_loss }
    }

    /// Apply parity–time symmetry to the gain/loss vector:
    /// `gain_loss[i] = -gain_loss[n-1-i]` (and the middle site, if any, is
    /// neutralized). Total gain then exactly balances total loss, which keeps
    /// the total amplitude bounded over the evolution.
    pub fn add_parity_time_symmetry(&mut self) {
        let n = self.gain_loss.len();
        for i in 0..n / 2 {
            self.gain_loss[n - 1 - i] = -self.gain_loss[i];
        }
        if n % 2 == 1 {
            self.gain_loss[n / 2] = 0.0;
        }
    }

    /// Assemble the non-Hermitian generator `H = C + D`.
    pub fn hamiltonian(&self) -> DMatrix<f64> {
        let mut h = self.coupling.clone();
        for i in 0..self.gain_loss.len() {
            h[(i, i)] += self.gain_loss[i];
        }
        h
    }

    /// Expectation value `⟨ψ|H|ψ⟩`.
    ///
    /// Because `C` is anti-symmetric it contributes nothing to the quadratic
    /// form, so this equals the gain/loss-weighted amplitude `Σ g_i ψ_i²`.
    pub fn energy(&self, psi: &DVector<f64>) -> f64 {
        let h = self.hamiltonian();
        psi.dot(&(h * psi))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coupling_is_antisymmetric_with_zero_diagonal() {
        let n = 8;
        let h = Hamiltonian::antisymmetric(n, 42);
        for i in 0..n {
            assert_eq!(h.coupling[(i, i)], 0.0, "diagonal must be zero");
            for j in 0..n {
                assert!(
                    (h.coupling[(i, j)] + h.coupling[(j, i)]).abs() < 1e-12,
                    "C must be anti-symmetric"
                );
            }
        }
    }

    #[test]
    fn construction_is_deterministic() {
        let a = Hamiltonian::antisymmetric(8, 7);
        let b = Hamiltonian::antisymmetric(8, 7);
        assert_eq!(a.coupling, b.coupling);
        assert_eq!(a.gain_loss, b.gain_loss);
    }

    #[test]
    fn parity_time_symmetry_balances_gain_and_loss() {
        let n = 8;
        let mut h = Hamiltonian::antisymmetric(n, 5);
        h.add_parity_time_symmetry();
        for i in 0..n / 2 {
            assert!(
                (h.gain_loss[i] + h.gain_loss[n - 1 - i]).abs() < 1e-12,
                "gain_loss[i] must equal -gain_loss[n-1-i]"
            );
        }
        let total: f64 = h.gain_loss.iter().sum();
        assert!(total.abs() < 1e-12, "total gain must balance total loss");
    }

    #[test]
    fn skew_coupling_cannot_change_total_amplitude() {
        let n = 8;
        let h = Hamiltonian::antisymmetric(n, 11);
        let psi = DVector::from_fn(n, |i, _| 0.5 + i as f64);
        let hh = h.hamiltonian();
        let skew_part = &hh - hh.transpose();
        assert!(skew_part.norm() > 1e-6, "coupling should be non-trivial");
        // ⟨ψ|C|ψ⟩ must vanish for anti-symmetric C, so energy equals gain term.
        let energy = h.energy(&psi);
        let gain_only: f64 = (0..n).map(|i| h.gain_loss[i] * psi[i] * psi[i]).sum();
        assert!((energy - gain_only).abs() < 1e-9);
    }
}
