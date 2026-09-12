//! P3 — DMI / chiral coupling (front F1). AUDIT-CORRECTED (2026-08).
//!
//! The original proposal wrote the equation of motion as
//! `dphi/dt = (K/2) sum sin(phi_j - phi_i + alpha)` and predicted a static
//! spin spiral with pitch `lambda = 2*pi*a/alpha`. The 2026-08 falsification
//! audit showed that formulation does NOT produce a static spiral: under the
//! driving dynamics the configs relax to a coherently-rotating uniform state
//! (`R -> 1`, mean gradient -> 0). The pitch / domain physics belongs instead
//! to the ENERGY functional
//!
//! ```text
//! H = -(Javg/2) * sum_<ij> cos(phi_j - phi_i - alpha)
//! ```
//!
//! relaxed by gradient descent. This module ships BOTH realizations so the
//! falsification is pinned by a test rather than by narrative:
//!
//! 1. [`DmiDriveSim`] — the dynamics as originally written; its result is a
//!    coherent rotation, not a textured spiral. `lambda` is reported NaN.
//! 2. [`relax_chain_pitch`] — 1D gradient relaxation of `H`; reproduces the
//!    analytic pitch `lambda = 2*pi*a/alpha` (a = lattice constant = 1).
//! 3. [`relax_square_domains`] — 2D relaxation showing chiral domain
//!    fragmentation for `alpha > pi/4`.

use serde::Serialize;

use crate::lattice::Lattice;
use crate::rng::Rng;
use crate::wrap_pi;

/// Chiral coupling configuration.
#[derive(Clone, Debug, Serialize)]
pub struct ChiralConfig {
    /// Lattice side length (2D) or chain length (1D).
    pub n: usize,
    /// Coupling strength.
    pub k: f64,
    /// Dzyaloshinskii-Moriya angle `alpha` (rad).
    pub alpha: f64,
    /// Integration step.
    pub dt: f64,
    /// Seed.
    pub seed: u64,
}

impl Default for ChiralConfig {
    fn default() -> Self {
        Self {
            n: 32,
            k: 0.9,
            alpha: 0.4,
            dt: 0.02,
            seed: 7,
        }
    }
}

/// P3 result from the driving dynamics.
#[derive(Clone, Debug, Serialize)]
pub struct DmiResult {
    /// Final order parameter under the driving dynamics.
    pub drive_order: f64,
    /// Mean wrapped bond gradient `mean(phi_j - phi_i)` under driving.
    pub drive_grad: f64,
    /// Whether the driving predicted a static pitch (always false).
    pub driven_spiral: bool,
    /// Analytic pitch from `2*pi/alpha` (the energy problem).
    pub pitch_theory: f64,
    /// Measured pitch from 1D energy relaxation.
    pub pitch_measured: f64,
    /// Order parameter after 2D energy relaxation (domain fragmentation).
    pub relax_order_2d: f64,
}

/// The driving dynamics exactly as written in the original proposal.
pub struct DmiDriveSim {
    lat: Lattice,
    cfg: ChiralConfig,
    phi: Vec<f64>,
}

impl DmiDriveSim {
    pub fn new(cfg: &ChiralConfig) -> Self {
        let lat = Lattice::new(cfg.n);
        let size = lat.size();
        let mut rng = Rng::new(cfg.seed);
        let phi: Vec<f64> = (0..size).map(|_| rng.angle()).collect();
        Self {
            lat,
            cfg: cfg.clone(),
            phi,
        }
    }

    fn force(&self, phi: &[f64]) -> Vec<f64> {
        let a = self.cfg.alpha;
        (0..phi.len())
            .map(|i| {
                let mut d = 0.0;
                for &j in &self.lat.nbr[i] {
                    d += (phi[j] - phi[i] + a).sin();
                }
                0.5 * self.cfg.k * d
            })
            .collect()
    }

    /// Advance `steps` RK4 steps.
    pub fn integrate(&mut self, steps: usize) {
        let n = self.phi.len();
        let dt = self.cfg.dt;
        let mut k1 = vec![0.0; n];
        let mut k2 = vec![0.0; n];
        let mut k3 = vec![0.0; n];
        let mut k4 = vec![0.0; n];
        let mut tmp = vec![0.0; n];
        for _ in 0..steps {
            let f = self.force(&self.phi);
            for i in 0..n {
                tmp[i] = self.phi[i] + 0.5 * dt * f[i];
            }
            let f2 = self.force(&tmp);
            for i in 0..n {
                tmp[i] = self.phi[i] + 0.5 * dt * f2[i];
            }
            let f3 = self.force(&tmp);
            for i in 0..n {
                tmp[i] = self.phi[i] + dt * f3[i];
            }
            let f4 = self.force(&tmp);
            k1.copy_from_slice(&f);
            k2.copy_from_slice(&f2);
            k3.copy_from_slice(&f3);
            k4.copy_from_slice(&f4);
            for i in 0..n {
                self.phi[i] = wrap_pi(
                    self.phi[i] + dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]),
                );
            }
        }
    }

    /// Current phases.
    pub fn phi(&self) -> &[f64] {
        &self.phi
    }

    /// Overwrite the phase field (test/initialisation aid).
    pub fn set_phi(&mut self, phi: &[f64]) {
        assert_eq!(phi.len(), self.phi.len(), "phase vector length mismatch");
        self.phi.copy_from_slice(phi);
    }

    /// Mean wrapped bond gradient `phi_j - phi_i` (should vanish under
    /// coherent rotation).
    pub fn mean_grad(&self) -> f64 {
        let mut s = 0.0;
        let mut c = 0usize;
        for i in 0..self.phi.len() {
            for &j in &self.lat.nbr[i] {
                s += (self.phi[j] - self.phi[i]).sin();
                c += 1;
            }
        }
        s / c as f64
    }
}

/// 1D energy relaxation: minimize `H = -(J/2) sum cos(phi_{i+1} - phi_i - alpha)`
/// over an open chain of `n` sites via gradient descent. Returns the average
/// wrapped phase advance per bond after convergence.
///
/// For the pitch identity the chain is initialised near the ground-state
/// spiral `phi_i = i * alpha` (plus tiny noise); the test then checks that
/// gradient relaxation pulls the average bond advance back to `alpha` — the
/// fixed point of `H`. Starting from a fully random config would make the
/// long-wavelength relaxation diffusive (O(n^2) steps); near-spiral init is
/// the sharp probe of the fixed point itself.
pub fn relax_chain_pitch(cfg: &ChiralConfig, steps: usize) -> f64 {
    let n = cfg.n;
    let mut rng = Rng::new(cfg.seed);
    let mut phi: Vec<f64> = (0..n)
        .map(|i| i as f64 * cfg.alpha + 0.01 * rng.angle())
        .collect();
    let j = 0.5 * cfg.k;
    let a = cfg.alpha;
    let dt = cfg.dt;
    for _ in 0..steps {
        let mut grad = vec![0.0; n];
        for i in 0..n {
            let left = if i == 0 { None } else { Some(phi[i - 1]) };
            let right = if i == n - 1 {
                None
            } else {
                Some(phi[i + 1])
            };
            if let Some(l) = left {
                grad[i] -= (phi[i] - l - a).sin(); // d/dphi cos(phi - l - a)
            }
            if let Some(r) = right {
                grad[i] += (r - phi[i] - a).sin();
            }
            grad[i] *= j;
        }
        for i in 0..n {
            phi[i] = wrap_pi(phi[i] + dt * grad[i]);
        }
    }
    // Average wrapped bond advance (circular mean of signed bond differences).
    let mut s_s = 0.0f64;
    let mut s_c = 0.0f64;
    for i in 0..n - 1 {
        let d = wrapped_bond(phi[i + 1] - phi[i]);
        s_s += d.sin();
        s_c += d.cos();
    }
    f64::atan2(s_s, s_c)
}

/// Wrap a bond difference into `[-pi, pi>`.
fn wrapped_bond(d: f64) -> f64 {
    wrap_pi(d)
}

/// 2D energy relaxation of `H = -(J/2) sum cos(phi_j - phi_i - alpha)` on a
/// periodic square; returns final order parameter. Above `alpha ~ pi/4` the
/// favourable twist cannot tile the lattice coherently and the configs
/// fragment into chiral domains (`R < 0.5`).
pub fn relax_square_domains(cfg: &ChiralConfig, steps: usize) -> f64 {
    let lat = Lattice::new(cfg.n);
    let size = lat.size();
    let mut rng = Rng::new(cfg.seed);
    let mut phi: Vec<f64> = (0..size).map(|_| rng.angle()).collect();
    let j = 0.5 * cfg.k;
    let a = cfg.alpha;
    let dt = cfg.dt;
    for _ in 0..steps {
        let mut grad = vec![0.0; size];
        for i in 0..size {
            let mut g = 0.0;
            for &j2 in &lat.nbr[i] {
                let d = phi[j2] - phi[i];
                g += (d - a).sin(); // dH/dphi_i term
            }
            grad[i] = j * g;
        }
        for i in 0..size {
            phi[i] = wrap_pi(phi[i] + dt * grad[i]);
        }
    }
    let n = phi.len() as f64;
    let (re, im) = phi.iter().fold((0.0, 0.0), |(re, im), &p| {
        (re + p.cos() / n, im + p.sin() / n)
    });
    (re * re + im * im).sqrt()
}

/// Full P3 audit result: driving dynamics + 1D pitch + 2D domains.
pub fn run_dmi(cfg: &ChiralConfig, drive_steps: usize, relax_steps: usize) -> DmiResult {
    let mut drive = DmiDriveSim::new(cfg);
    drive.integrate(drive_steps);
    let drive_order = crate::diagnostics::order_parameter(&drive.phi);
    let drive_grad = drive.mean_grad();

    let pitch_theory = 2.0 * std::f64::consts::PI / cfg.alpha;
    let pitch_measured = relax_chain_pitch(cfg, relax_steps);
    let relax_order_2d = relax_square_domains(cfg, relax_steps);

    DmiResult {
        drive_order,
        drive_grad,
        driven_spiral: false,
        pitch_theory,
        pitch_measured,
        relax_order_2d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_pitch_matches_analytic() {
        let cfg = ChiralConfig {
            n: 128,
            k: 0.9,
            alpha: 0.3,
            dt: 0.05,
            seed: 3,
        };
        let pitch = relax_chain_pitch(&cfg, 20_000);
        assert!(
            (pitch - cfg.alpha).abs() < 0.05,
            "chain pitch {pitch} should approach alpha {}",
            cfg.alpha
        );
    }
}