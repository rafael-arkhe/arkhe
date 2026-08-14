use nalgebra::DVector;

use crate::hamiltonian::{Hamiltonian, Lcg};
use crate::solver::Rk4Solver;
use crate::tree::Neuron;

/// Deterministic dendritic-growth simulation configuration.
///
/// The full outcome is a pure function of these seven values.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SimulationConfig {
    /// Number of lattice sites (also the initial node count).
    pub n: usize,
    /// RK4 time step.
    pub dt: f64,
    /// Total integration time.
    pub t_max: f64,
    /// Seed driving the Hamiltonian, the initial state and the dendrite angles.
    pub seed: u64,
    /// A site grows a branch whenever its amplitude `ψ_i²` exceeds this value.
    pub threshold: f64,
    /// Whether the threshold rule is active.
    pub growth_allowed: bool,
    /// Whether parity–time symmetry is applied to the gain/loss vector.
    pub parity_time_sym: bool,
}

impl Default for SimulationConfig {
    /// I16-flavoured reference defaults, used as the reproducibility baseline
    /// for the parameter sweep.
    fn default() -> Self {
        Self {
            n: 16,
            dt: 0.01,
            t_max: 3.0,
            seed: 7,
            threshold: 0.3,
            growth_allowed: true,
            parity_time_sym: true,
        }
    }
}

/// Outcome of a [`SimulationConfig::run`]: the total-amplitude time series, the
/// grown neuron, and the metrics used by the constitutional invariants.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SimulationResult {
    /// Total amplitude `Σ ψ_i²` per step (index 0 is the initial state).
    pub amplitude_series: Vec<f64>,
    /// The neuron after growth: lattice nodes plus dendrite branches.
    pub neuron: Neuron,
    /// Peak relative deviation of the total amplitude from its initial value
    /// (I18 fidelity metric).
    pub peak_drift: f64,
    /// Final relative deviation of the total amplitude (I18 fidelity metric).
    pub final_drift: f64,
    /// Time at which the peak deviation occurred.
    pub peak_time: f64,
    /// Number of dendrite branches grown (I16 complexity metric).
    pub branches: usize,
}

impl SimulationConfig {
    /// Run the full simulation: build the Hamiltonian, integrate the amplitude
    /// field with RK4, and grow dendrite branches at every site whose
    /// amplitude crosses the threshold (each site sprouts at most once).
    pub fn run(&self) -> SimulationResult {
        let mut ham = Hamiltonian::antisymmetric(self.n, self.seed);
        if self.parity_time_sym {
            ham.add_parity_time_symmetry();
        }
        let h = ham.hamiltonian();
        let psi0 = self.initial_state();
        let solver = Rk4Solver::new(self.dt, self.t_max);
        let amplitudes = solver.solve_amplitudes(&h, &psi0);
        let norms = solver.solve(&h, &psi0);

        let mut neuron = Neuron::seed((0.0, 0.0), self.seed);
        for i in 0..self.n {
            neuron.add_node((i as f64, 0.0));
        }

        let mut grown = vec![false; self.n];
        for amps in amplitudes.iter() {
            neuron.tick();
            if self.growth_allowed {
                for j in 0..self.n {
                    if !grown[j] && amps[j] > self.threshold && neuron.grow(j).is_some() {
                        grown[j] = true;
                    }
                }
            }
        }

        let initial = norms[0];
        let mut peak_drift = 0.0_f64;
        let mut peak_idx = 0usize;
        for (i, &x) in norms.iter().enumerate() {
            let dev = (x - initial).abs() / initial;
            if dev > peak_drift {
                peak_drift = dev;
                peak_idx = i;
            }
        }
        let final_drift = (norms[norms.len() - 1] - initial).abs() / initial;
        let branches = neuron.branches.len();

        SimulationResult {
            amplitude_series: norms,
            neuron,
            peak_drift,
            final_drift,
            peak_time: peak_idx as f64 * self.dt,
            branches,
        }
    }

    /// Deterministic, normalized initial state: a slightly perturbed uniform
    /// amplitude vector derived from `seed + 777`.
    fn initial_state(&self) -> DVector<f64> {
        let mut rng = Lcg::new(self.seed + 777);
        let mut psi = DVector::from_fn(self.n, |_, _| 1.0 + 0.1 * rng.signed(1.0));
        psi /= psi.norm();
        psi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_is_deterministic() {
        let cfg = SimulationConfig {
            n: 8,
            dt: 0.01,
            t_max: 1.0,
            seed: 21,
            threshold: 0.3,
            growth_allowed: true,
            parity_time_sym: true,
        };
        let a = cfg.run();
        let b = cfg.run();
        assert_eq!(a.amplitude_series, b.amplitude_series);
        assert_eq!(a.neuron.branches, b.neuron.branches);
    }

    #[test]
    fn disabled_growth_never_grows() {
        let cfg = SimulationConfig {
            n: 8,
            dt: 0.01,
            t_max: 1.0,
            seed: 21,
            threshold: 0.0,
            growth_allowed: false,
            parity_time_sym: true,
        };
        let res = cfg.run();
        assert!(res.neuron.branches.is_empty());
        assert_eq!(res.neuron.nodes.len(), cfg.n + 1);
    }
}
