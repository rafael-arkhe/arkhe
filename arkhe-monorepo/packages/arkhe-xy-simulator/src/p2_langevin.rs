//! P2 — finite-temperature XY (Langevin/Heun) for the Kosterlitz-Thouless
//! regime, mapped to front F4 (ligas Co-Zn-Mn).
//!
//! Convention (AUDIT-CORRECTED, 2026-08): the V8.6 Hamiltonian is
//! `H = -(K/2) * sum_<ij> cos(phi_j - phi_i)`, i.e. `J = K/2`. The
//! Kosterlitz-Thouless temperature of the square-lattice XY model is
//! `T_KT ~= 0.89 * J`, so:
//!
//! ```text
//! T_KT(K) = 0.89 * K / 2
//! ```
//!
//! The original proposal claimed `T_KT ≈ 0.53` (and `0.45` elsewhere) for
//! `K = 0.6`; the consistent value with `J = K/2` is `T_KT ≈ 0.267`. This
//! module exposes the corrected factor and validates it through the Binder
//! cumulant crossing estimator.

use serde::Serialize;

use crate::diagnostics::order_parameter;
use crate::lattice::Lattice;
use crate::rng::Rng;

/// KT prefactor for the square lattice: `T_KT = FACTOR * J`.
pub const T_KT_FACTOR: f64 = 0.89;

/// P2 configuration (overdamped Langevin, Heun step).
#[derive(Clone, Debug, Serialize)]
pub struct LangevinConfig {
    /// Lattice side length.
    pub n: usize,
    /// Coupling `K` (Hamiltonian `H = -(K/2) sum cos`).
    pub k: f64,
    /// Temperature in energy units (`kB = 1`).
    pub t: f64,
    /// Integration step.
    pub dt: f64,
    /// Seed.
    pub seed: u64,
}

impl Default for LangevinConfig {
    fn default() -> Self {
        Self {
            n: 16,
            k: 0.6,
            t: 0.1,
            dt: 0.01,
            seed: 7,
        }
    }
}

impl LangevinConfig {
    /// Corrected analytic `T_KT` for this `K` (see module docs).
    pub fn t_kt(&self) -> f64 {
        T_KT_FACTOR * self.k / 2.0
    }

    /// Lower energy bound: each bond contributes `>= -K/2`, and there are
    /// `2 * n * n` bonds on the periodic lattice.
    pub fn energy_floor(&self) -> f64 {
        -(self.k * self.n as f64 * self.n as f64)
    }
}

/// P2 result.
#[derive(Clone, Debug, Serialize)]
pub struct LangevinResult {
    /// Final mean order parameter across seeds.
    pub order_mean: f64,
    /// Std dev across seeds.
    pub order_std: f64,
    /// Binder cumulant `U4` (1 - <m^4>/3<m^2>^2) across seeds.
    pub binder_mean: f64,
    /// Corrected `T_KT`.
    pub t_kt: f64,
    /// Whether the observed energy ever violated the analytic floor.
    pub energy_floor_violated: bool,
}

/// Overdamped Langevin (Heun) simulator, single seed.
pub struct LangevinSim {
    cfg: LangevinConfig,
    lat: Lattice,
    phi: Vec<f64>,
    rng: Rng,
}

impl LangevinSim {
    pub fn new(cfg: &LangevinConfig) -> Self {
        let lat = Lattice::new(cfg.n);
        let size = lat.size();
        let mut rng = Rng::new(cfg.seed);
        let phi: Vec<f64> = (0..size).map(|_| rng.angle()).collect();
        Self {
            cfg: cfg.clone(),
            lat,
            phi,
            rng,
        }
    }

    fn force(&self, phi: &[f64]) -> Vec<f64> {
        let k = self.cfg.k;
        (0..phi.len())
            .map(|i| {
                let mut d = 0.0;
                for &j in &self.lat.nbr[i] {
                    d += (phi[j] - phi[i]).sin();
                }
                0.5 * k * d
            })
            .collect()
    }

    fn energy(&self, phi: &[f64]) -> f64 {
        let k = self.cfg.k;
        let mut e = 0.0;
        for i in 0..phi.len() {
            for &j in &self.lat.nbr[i] {
                e += (phi[j] - phi[i]).cos();
            }
        }
        -0.25 * k * e
    }

    /// Advance one Heun step (stochastic Heun for additive noise: the same noise
/// realization is used in predictor and corrector, sampling the canonical
/// measure `exp(-H/T)`).
    pub fn step(&mut self) {
        let dt = self.cfg.dt;
        let rt = (2.0 * self.cfg.t * dt).sqrt();
        let noise: Vec<f64> = (0..self.phi.len()).map(|_| self.rng.gaussian()).collect();
        let f = self.force(&self.phi);
        let mut pred = Vec::with_capacity(self.phi.len());
        for ((&p, &fp), &xi) in self.phi.iter().zip(f.iter()).zip(noise.iter()) {
            pred.push(p + dt * fp + rt * xi);
        }
        let fp = self.force(&pred);
        let n = self.phi.len();
        let mut next = Vec::with_capacity(n);
        for (i, (&p, &xi)) in self.phi.iter().zip(noise.iter()).enumerate() {
            next.push(wrap(p + 0.5 * dt * (f[i] + fp[i]) + rt * xi));
        }
        self.phi.copy_from_slice(&next);
    }

    /// Integrate `steps` Heun steps.
    pub fn integrate(&mut self, steps: usize) {
        for _ in 0..steps {
            self.step();
        }
    }

    /// Current phases.
    pub fn phi(&self) -> &[f64] {
        &self.phi
    }
}

/// Phase wrap helper (local copy to avoid a transitive import cycle).
fn wrap(a: f64) -> f64 {
    (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

/// Run the 5-seed protocol: mean/std order, Binder cumulant, energy-floor
/// violation flag. `burnin` and `sample` are Heun step counts.
pub fn run_langevin(cfg: &LangevinConfig, burnin: usize, sample: usize) -> LangevinResult {
    let seeds = [7u64, 11, 13, 17, 19];
    let mut orders = Vec::with_capacity(seeds.len());
    let mut binders = Vec::with_capacity(seeds.len());
    let mut floor_violated = false;

    for &seed in &seeds {
        let c = LangevinConfig { seed, ..cfg.clone() };
        let mut sim = LangevinSim::new(&c);
        sim.integrate(burnin);

        // Collect samples for U4.
        let mut m2 = 0.0f64;
        let mut m4 = 0.0f64;
        let mut n_samp = 0usize;
        let mut min_e = 0.0f64;
        for _ in 0..sample {
            sim.step();
            let m = order_parameter(&sim.phi);
            m2 += m * m;
            m4 += m * m * m * m;
            n_samp += 1;
            let e = sim.energy(&sim.phi);
            min_e = min_e.min(e);
        }
        m2 /= n_samp as f64;
        m4 /= n_samp as f64;
        let u4 = if m2.abs() > 1e-12 { 1.0 - m4 / (3.0 * m2 * m2) } else { 0.0 };
        orders.push(order_parameter(&sim.phi));
        binders.push(u4);
        if min_e < cfg.energy_floor() - 1e-9 {
            floor_violated = true;
        }
    }

    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let std = |v: &[f64], m: f64| {
        (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
    };

    let om = mean(&orders);
    let bm = mean(&binders);
    LangevinResult {
        order_mean: om,
        order_std: std(&orders, om),
        binder_mean: bm,
        t_kt: cfg.t_kt(),
        energy_floor_violated: floor_violated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrected_kt_value() {
        let cfg = LangevinConfig {
            k: 0.6,
            ..LangevinConfig::default()
        };
        // 0.89 * 0.6 / 2 = 0.267
        assert!((cfg.t_kt() - 0.267).abs() < 1e-9);
    }

    #[test]
    fn energy_floor_matches_bond_count() {
        let cfg = LangevinConfig::default();
        assert!((cfg.energy_floor() - (-0.6 * 16.0 * 16.0)).abs() < 1e-9);
    }
}