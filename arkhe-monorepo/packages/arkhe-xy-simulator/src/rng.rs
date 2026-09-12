//! Deterministic reproducible RNG (glibc-style 64-bit LCG).
//!
//! Mirrors the `Lcg` used by `arkhe-neurogenesis` so sweep results are
//! reproducible. No external `rand` dependency keeps the dependency surface
//! minimal (Simplicity-2) and the sampler cryptographically-irrelevant.

/// Deterministic 64-bit linear congruential generator (glibc constants).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// New generator from an arbitrary seed.
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_add(0x9E3779B97F4A7C15))
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    /// Uniform sample in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform sample in `[-scale, scale)`.
    pub fn signed(&mut self, scale: f64) -> f64 {
        (self.unit() * 2.0 - 1.0) * scale
    }

    /// Standard-normal sample (Box-Muller).
    pub fn gaussian(&mut self) -> f64 {
        let u1 = self.unit().max(1e-300);
        let u2 = self.unit();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    /// Quenched natural frequency drawn from a Lorentzian distribution
    /// `g(w) = gamma / (pi * (w^2 + gamma^2))` (inverse-CDF transform).
    pub fn lorentzian(&mut self, gamma: f64) -> f64 {
        gamma * (std::f64::consts::PI * (self.unit() - 0.5)).tan()
    }

    /// Uniform angle in `[-pi, pi)`.
    pub fn angle(&mut self) -> f64 {
        self.unit().mul_add(std::f64::consts::TAU, -std::f64::consts::PI)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_given_seed() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..100 {
            assert_eq!(a.next(), b.next());
        }
    }

    #[test]
    fn gaussian_moments_sane() {
        let mut rng = Rng::new(1);
        let v: Vec<f64> = (0..50_000).map(|_| rng.gaussian()).collect();
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64;
        assert!(mean.abs() < 0.02, "mean too far: {mean}");
        assert!((var - 1.0).abs() < 0.05, "variance off: {var}");
    }

    #[test]
    fn lorentzian_tail_heavy() {
        let mut rng = Rng::new(2);
        let v: Vec<f64> = (0..20_000).map(|_| rng.lorentzian(1.0)).collect();
        // The 99th percentile of a standard Lorentzian is ~63, so heavy tails
        // must be present: max absolute value clearly exceeds Gaussian range.
        let max_abs = v.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
        assert!(max_abs > 5.0, "no heavy tail observed: {max_abs}");
    }
}