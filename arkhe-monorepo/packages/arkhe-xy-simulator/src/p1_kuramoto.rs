//! P1 — Kuramoto phase-lattice with natural-frequency disorder, plus the
//! bidirectional PLVR pair (EEG <-> laser) mapped to front F2.

use serde::Serialize;

use crate::diagnostics::order_parameter;
use crate::lattice::Lattice;
use crate::rng::Rng;
use crate::wrap_pi;

/// Natural-frequency distribution for the quenched site disorder.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum FreqDist {
    /// `g(w) = exp(-w^2 / 2 sigma^2) / sqrt(2*pi) * sigma` — EEG theta band.
    Gaussian { sigma: f64 },
    /// `g(w) = gamma / (pi * (w^2 + gamma^2))` — broad-gain laser line.
    Lorentzian { gamma: f64 },
}

impl FreqDist {
    /// All-to-all Kuramoto critical coupling `Kc = 2 / (pi * g(0))`.
    ///
    /// * Gaussian: `Kc = sqrt(8/pi) * sigma ~= 1.596 * sigma`
    /// * Lorentzian: `Kc = 2 * gamma`
    ///
    /// On a 4-NN lattice the effective per-site coupling is larger, so the
    /// observed transition sits below this bound; the DOCUMENTED prediction is
    /// therefore `Kc_lattice <= Kc_all_to_all`.
    pub fn kc_all_to_all(&self) -> f64 {
        match *self {
            FreqDist::Gaussian { sigma } => (8.0 / std::f64::consts::PI).sqrt() * sigma,
            FreqDist::Lorentzian { gamma } => 2.0 * gamma,
        }
    }

    fn draw(&self, rng: &mut Rng) -> f64 {
        match *self {
            FreqDist::Gaussian { sigma } => sigma * rng.gaussian(),
            FreqDist::Lorentzian { gamma } => rng.lorentzian(gamma),
        }
    }
}

/// P1 configuration.
#[derive(Clone, Debug, Serialize)]
pub struct KuramotoConfig {
    /// Lattice side length.
    pub n: usize,
    /// Integration step.
    pub dt: f64,
    /// Coupling strength `K` (per the V8.6 equation, `dphi = w + (K/2) sum sin`).
    pub k: f64,
    /// Natural-frequency distribution.
    pub dist: FreqDist,
    /// Seed for frequencies and initial phases.
    pub seed: u64,
}

impl Default for KuramotoConfig {
    fn default() -> Self {
        Self {
            n: 8,
            dt: 0.02,
            k: 0.9,
            dist: FreqDist::Gaussian { sigma: 0.5 },
            seed: 7,
        }
    }
}

/// P1 result.
#[derive(Clone, Debug, Serialize)]
pub struct KuramotoResult {
    /// Final order parameter `R = |<exp(i phi)>|`.
    pub final_orders: Vec<f64>,
    /// Mean final order across seeds.
    pub final_order_mean: f64,
    /// Std dev of final order across seeds.
    pub final_order_std: f64,
    /// All-to-all analytic `Kc` for the chosen distribution.
    pub kc_all_to_all: f64,
    /// Whether the order-parameter variation over the last 1000 steps was
    /// below `1e-6` (convergence requirement).
    pub converged: bool,
}

/// Kuramoto simulator over one seed; call [`KuramotoSim::run`] for the
/// 5-seed protocol helper or build directly for fine-grained control.
pub struct KuramotoSim {
    lat: Lattice,
    cfg: KuramotoConfig,
    phi: Vec<f64>,
    omega: Vec<f64>,
}

impl KuramotoSim {
    /// Build from a config.
    pub fn new(cfg: &KuramotoConfig) -> Self {
        let lat = Lattice::new(cfg.n);
        let size = lat.size();
        let mut rng = Rng::new(cfg.seed);
        let phi: Vec<f64> = (0..size).map(|_| rng.angle()).collect();
        let omega: Vec<f64> = (0..size).map(|_| cfg.dist.draw(&mut rng)).collect();
        Self {
            lat,
            cfg: cfg.clone(),
            phi,
            omega,
        }
    }

    fn force(&self, phi: &[f64], out: &mut [f64]) {
        for i in 0..phi.len() {
            let mut d = self.omega[i];
            for &j in &self.lat.nbr[i] {
                d += 0.5 * self.cfg.k * (phi[j] - phi[i]).sin();
            }
            out[i] = d;
        }
    }

    /// Integrate `steps` RK4 steps in place.
    pub fn step(&mut self, steps: usize) {
        let n = self.phi.len();
        let dt = self.cfg.dt;
        let mut k1 = vec![0.0; n];
        let mut k2 = vec![0.0; n];
        let mut k3 = vec![0.0; n];
        let mut k4 = vec![0.0; n];
        let mut tmp = vec![0.0; n];
        for _ in 0..steps {
            self.force(&self.phi, &mut k1);
            for i in 0..n {
                tmp[i] = self.phi[i] + 0.5 * dt * k1[i];
            }
            self.force(&tmp, &mut k2);
            for i in 0..n {
                tmp[i] = self.phi[i] + 0.5 * dt * k2[i];
            }
            self.force(&tmp, &mut k3);
            for i in 0..n {
                tmp[i] = self.phi[i] + dt * k3[i];
            }
            self.force(&tmp, &mut k4);
            for i in 0..n {
                self.phi[i] += dt
                    * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i])
                        / 6.0;
                self.phi[i] = wrap_pi(self.phi[i]);
            }
        }
    }

    /// Current phases.
    pub fn phi(&self) -> &[f64] {
        &self.phi
    }

    /// Run the 5-seed protocol: mean/std of the final order parameter, and
    /// whether the trajectory converged. `steps` is the number of RK4 steps
    /// per seed; `settle` how many trailing steps count for convergence.
    pub fn run(cfg: &KuramotoConfig, steps: usize, settle: usize) -> KuramotoResult {
        let kc_all_to_all = cfg.dist.kc_all_to_all();
        let mut sim = Self::new(cfg);
        sim.step(steps + settle);
        let r_ref = order_parameter(&sim.phi);

        // 5 independent seeds for the reported statistics (protocol).
        let seeds = [11u64, 13, 17, 19, 23];
        let mut final_orders = vec![r_ref];
        final_orders.extend(seeds.iter().map(|&seed| {
            let c = KuramotoConfig { seed, ..cfg.clone() };
            let mut s = Self::new(&c);
            s.step(steps + settle);
            order_parameter(&s.phi)
        }));

        let mean = final_orders.iter().sum::<f64>() / final_orders.len() as f64;
        let std = (final_orders
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>()
            / final_orders.len() as f64)
            .sqrt();

        // Convergence: max |R(t) - R(t-1000)| < 1e-6 over a 1000-step window.
        let mut lag = 0.0_f64;
        let mut prev_r = order_parameter(&sim.phi);
        for _ in 0..1000 {
            sim.step(1);
            let r = order_parameter(&sim.phi);
            lag = lag.max((r - prev_r).abs());
            prev_r = r;
        }
        let converged = lag < 1e-6;

        KuramotoResult {
            final_orders,
            final_order_mean: mean,
            final_order_std: std,
            kc_all_to_all,
            converged,
        }
    }
}

/// Bidirectional PLVR pair: one EEG oscillator (theta band ~4-8 Hz) and one
/// laser oscillator, coupled with strength `k` exactly as in F2:
/// `dphi_l = w_l + k sin(phi_e - phi_l)`, `dphi_e = w_e + k sin(phi_l - phi_e)`.
#[derive(Clone, Debug, Serialize)]
pub struct PlvrPair {
    /// Laser phase (rad).
    pub phi_l: f64,
    /// EEG phase (rad).
    pub phi_e: f64,
    /// Laser natural frequency `w_l` (rad/s).
    pub omega_l: f64,
    /// EEG natural frequency `w_e` (rad/s).
    pub omega_e: f64,
    /// Bidirectional coupling strength.
    pub k: f64,
    /// Integration step.
    pub dt: f64,
    /// Initial seed (only fixed points are analytic, so phases here are
    /// deterministic if the seed is fixed).
    pub seed: u64,
}

impl Default for PlvrPair {
    /// Defaults: mid-theta EEG (6 Hz = 37.7 rad/s) vs a detuned laser.
    fn default() -> Self {
        Self {
            phi_l: 0.0,
            phi_e: 0.0,
            omega_l: 2.0,
            omega_e: 1.0,
            k: 1.0,
            dt: 0.02,
            seed: 1,
        }
    }
}

impl PlvrPair {
    /// Analytic lock condition for the two-oscillator system:
    /// the phase-locked solution exists iff `|w_l - w_e| / (2 k) <= 1`.
    pub fn analytic_lock(&self) -> bool {
        ((self.omega_l - self.omega_e) / (2.0 * self.k)).abs() <= 1.0
    }

    /// Integrate `steps` steps (RK4) and return the state.
    pub fn integrate(&mut self, steps: usize) {
        let dt = self.dt;
        for _ in 0..steps {
            let we = self.omega_l + self.k * (self.phi_e - self.phi_l).sin();
            let fe = self.omega_e + self.k * (self.phi_l - self.phi_e).sin();
            // RK4 on the pair (phi_l, phi_e).
            let (e1l, e1e) = (we, fe);
            let (l2, e2) = (
                self.omega_l + self.k * (self.phi_e + 0.5 * dt * e1e - self.phi_l - 0.5 * dt * e1l).sin(),
                self.omega_e + self.k * (self.phi_l + 0.5 * dt * e1l - self.phi_e - 0.5 * dt * e1e).sin(),
            );
            let (l3, e3) = (
                self.omega_l + self.k * (self.phi_e + 0.5 * dt * e2 - self.phi_l - 0.5 * dt * l2).sin(),
                self.omega_e + self.k * (self.phi_l + 0.5 * dt * l2 - self.phi_e - 0.5 * dt * e2).sin(),
            );
            let (l4, e4) = (
                self.omega_l + self.k * ((self.phi_e + dt * e3) - (self.phi_l + dt * l3)).sin(),
                self.omega_e + self.k * ((self.phi_l + dt * l3) - (self.phi_e + dt * e3)).sin(),
            );
            self.phi_l += dt / 6.0 * (e1l + 2.0 * l2 + 2.0 * l3 + l4);
            self.phi_e += dt / 6.0 * (e1e + 2.0 * e2 + 2.0 * e3 + e4);
            self.phi_l = wrap_pi(self.phi_l);
            self.phi_e = wrap_pi(self.phi_e);
        }
    }

    /// Wrapped phase difference `phi_l - phi_e` in `[-pi, pi>`.
    pub fn wrapped_diff(&self) -> f64 {
        wrap_pi(self.phi_l - self.phi_e)
    }
}

/// P1 result binder.
#[derive(Clone, Debug, Serialize)]
pub struct PlvrResult {
    /// Whether the two-oscillator system reached phase lock after settling.
    pub locked: bool,
    /// Final wrapped phase difference.
    pub diff_rad: f64,
}

/// Settle and report lock status for a PLVR pair (mid-band EEG vs laser).
///
/// A two-oscillator Kuramoto system locks to the phase difference
/// `delta* = asin((omega_l - omega_e) / (2 k))`; the pair is reported locked
/// when the natural frequencies admit a fixed point AND the settled phase
/// difference lands on that analytic target.
pub fn plvr_lock(cfg: &PlvrPair, settle_steps: usize) -> PlvrResult {
    let mut pair = cfg.clone();
    pair.integrate(settle_steps);
    let target = if cfg.analytic_lock() {
        ((cfg.omega_l - cfg.omega_e) / (2.0 * cfg.k)).asin()
    } else {
        f64::NAN
    };
    let diff = pair.wrapped_diff();
    let locked = cfg.analytic_lock()
        && (diff - target).abs() < 1e-3
        && diff.is_finite();
    PlvrResult {
        locked,
        diff_rad: diff,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_kc_formula() {
        let d = FreqDist::Gaussian { sigma: 0.5 };
        assert!((d.kc_all_to_all() - (8.0 / std::f64::consts::PI).sqrt() * 0.5).abs() < 1e-12);
    }

    #[test]
    fn plvr_analytic_lock_correct() {
        let pair = PlvrPair {
            phi_l: 0.0,
            phi_e: 0.0,
            omega_l: 1.0,
            omega_e: 0.0,
            k: 1.0,
            dt: 0.02,
            seed: 1,
        };
        assert!(pair.analytic_lock()); // |1-0|/(2*1) = 0.5 <= 1
        let pair2 = PlvrPair {
            omega_l: 3.0,
            omega_e: 0.0,
            k: 1.0,
            ..pair
        };
        assert!(!pair2.analytic_lock()); // 1.5 > 1
    }
}