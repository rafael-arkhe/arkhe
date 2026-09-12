//! P4 — pinned topological-charge centres (`phi` fixed at a subset of sites).
//!
//! In the Cathedral V8.6 mesh each skyrmion node carries `Q = 24`; treating
//! those nodes as *pinning centres* with a preferred phase is the P4
//! hypothesis: a mask of sites whose phase is restored after every step.
//!
//! AUDIT NOTE: P1/P3 already capture the essence of the interactions; P4 is a
//! refinement for when F1 produces real mesh phase measurements per node. The
//! mask is a boolean array; pinned sites are clamped to their set phase.

use serde::Serialize;

use crate::diagnostics::order_parameter;
use crate::lattice::Lattice;
use crate::rng::Rng;
use crate::wrap_pi;

/// P4 configuration.
#[derive(Clone, Debug, Serialize)]
pub struct PinnedConfig {
    /// Lattice side length.
    pub n: usize,
    /// Coupling strength.
    pub k: f64,
    /// Integration step.
    pub dt: f64,
    /// Seed for the unpinned initial phases.
    pub seed: u64,
    /// Pinning mask: `(site, phase_rad)` pairs.
    pub pin_sites: Vec<(usize, f64)>,
}

impl Default for PinnedConfig {
    fn default() -> Self {
        Self {
            n: 8,
            k: 0.6,
            dt: 0.02,
            seed: 7,
            pin_sites: Vec::new(),
        }
    }
}

/// XY dynamics with a pin mask (RK4 + clamp).
pub struct PinnedSim {
    cfg: PinnedConfig,
    lat: Lattice,
    phi: Vec<f64>,
    /// Per-site pinned phase, `None` when free.
    pin_map: Vec<Option<f64>>,
}

impl PinnedSim {
    pub fn new(cfg: &PinnedConfig) -> Self {
        let lat = Lattice::new(cfg.n);
        let size = lat.size();
        let mut rng = Rng::new(cfg.seed);
        let phi: Vec<f64> = (0..size).map(|_| rng.angle()).collect();
        let mut pin_map = vec![None; size];
        for &(s, p) in &cfg.pin_sites {
            assert!(s < size, "pin site out of range");
            pin_map[s] = Some(p);
        }
        Self {
            cfg: cfg.clone(),
            lat,
            phi,
            pin_map,
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

    /// Debug: neighbour list for a site.
    pub fn debug_nbr(&self, i: usize) -> Vec<usize> {
        self.lat.nbr[i].to_vec()
    }

    /// Debug: current force on every site.
    pub fn debug_force(&self) -> Vec<f64> {
        self.force(&self.phi)
    }

    /// Advance `steps` RK4 steps, clamping pinned sites afterwards.
    pub fn integrate(&mut self, steps: usize) {
        let n = self.phi.len();
        let dt = self.cfg.dt;
        let mut k1 = vec![0.0; n];
        let mut k2 = vec![0.0; n];
        let mut k3 = vec![0.0; n];
        let mut k4 = vec![0.0; n];
        let mut tmp = vec![0.0; n];
        for _ in 0..steps {
            let f1 = self.force(&self.phi);
            for i in 0..n {
                tmp[i] = self.phi[i] + 0.5 * dt * f1[i];
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
            k1.copy_from_slice(&f1);
            k2.copy_from_slice(&f2);
            k3.copy_from_slice(&f3);
            k4.copy_from_slice(&f4);
            for i in 0..n {
                self.phi[i] = wrap_pi(
                    self.phi[i] + dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]),
                );
            }
            for (i, &phase) in self.pin_map.iter().enumerate() {
                if let Some(p) = phase {
                    self.phi[i] = p;
                }
            }
        }
    }

    /// Current phases.
    pub fn phi(&self) -> &[f64] {
        &self.phi
    }

    /// Final order parameter.
    pub fn order(&self) -> f64 {
        order_parameter(&self.phi)
    }

    /// Mean cosine to the nearest pinned phase — measures how strongly the
    /// lattice has been dragged into the pin's alignment (1.0 = fully locked
    /// to a pin; ~0 for random disorder).
    pub fn local_align(&self) -> f64 {
        let free: Vec<(usize, f64)> = self
            .phi
            .iter()
            .enumerate()
            .filter(|(i, _)| self.pin_map[*i].is_none())
            .map(|(i, &p)| (i, p))
            .collect();
        if free.is_empty() {
            return 1.0;
        }
        free.iter()
            .map(|&(i, ph)| {
                let target = self.nearest_pin_phase(i);
                (ph - target).cos()
            })
            .sum::<f64>()
            / free.len() as f64
    }

    /// Mean cosine to the nearest pinned phase within `max_dist` grid sites of
    /// a pin — isolates the halo the pin actually controls (vortices trap the
    /// bulk on a torus, so global order is not the right observable here).
    pub fn halo_align(&self, max_dist: usize) -> f64 {
        let halo: Vec<usize> = self
            .phi
            .iter()
            .enumerate()
            .filter(|(i, _)| self.pin_map[*i].is_none())
            .map(|(i, _)| i)
            .filter(|&i| {
                self.pin_map.iter().enumerate().any(|(j, &p)| {
                    p.is_some() && self.lat.dist(i, j) <= max_dist
                })
            })
            .collect();
        if halo.is_empty() {
            return 0.0;
        }
        halo.iter()
            .map(|&i| (self.phi[i] - self.nearest_pin_phase(i)).cos())
            .sum::<f64>()
            / halo.len() as f64
    }

    fn nearest_pin_phase(&self, i: usize) -> f64 {
        self.pin_map
            .iter()
            .enumerate()
            .filter_map(|(j, &p)| p.map(|ph| (j, ph)))
            .map(|(j, ph)| (self.lat.dist(i, j), ph))
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .map(|(_, ph)| ph)
            .unwrap_or(0.0)
    }
}

/// Mean shortest-path (Manhattan) distance to the nearest pin.
pub fn run_pinned(cfg: &PinnedConfig, steps: usize, seeds: &[u64]) -> (f64, f64) {
    let mut orders = Vec::with_capacity(seeds.len());
    for &seed in seeds {
        let c = PinnedConfig { seed, ..cfg.clone() };
        let mut sim = PinnedSim::new(&c);
        sim.integrate(steps);
        orders.push(sim.order());
    }
    let mean = orders.iter().sum::<f64>() / orders.len() as f64;
    let std = (orders.iter().map(|o| (o - mean).powi(2)).sum::<f64>() / orders.len() as f64).sqrt();
    (mean, std)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_pinned_gives_exact_order() {
        let n = 6;
        let pins: Vec<(usize, f64)> = (0..n * n).map(|s| (s, 1.0)).collect();
        let cfg = PinnedConfig {
            n,
            pin_sites: pins,
            ..PinnedConfig::default()
        };
        let (mean, std) = run_pinned(&cfg, 100, &[1, 2, 3]);
        assert!((mean - 1.0).abs() < 1e-9);
        assert!(std < 1e-9);
    }
}