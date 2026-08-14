use nalgebra::{DMatrix, DVector};

/// Fourth-order Runge–Kutta integrator for `dψ/dt = H ψ`.
pub struct Rk4Solver {
    pub dt: f64,
    pub t_max: f64,
}

impl Rk4Solver {
    pub fn new(dt: f64, t_max: f64) -> Self {
        Self { dt, t_max }
    }

    /// Integrate and return the total amplitude `Σ ψ_i²` at every step
    /// (including the initial state at index 0).
    pub fn solve(&self, h: &DMatrix<f64>, psi0: &DVector<f64>) -> Vec<f64> {
        self.trajectory(h, psi0)
            .iter()
            .map(|s| s.norm_squared())
            .collect()
    }

    /// Integrate and return the per-site amplitudes `ψ_i²` at every step
    /// (including the initial state at index 0).
    pub fn solve_amplitudes(&self, h: &DMatrix<f64>, psi0: &DVector<f64>) -> Vec<DVector<f64>> {
        self.trajectory(h, psi0)
            .iter()
            .map(|s| s.map(|x| x * x))
            .collect()
    }

    fn trajectory(&self, h: &DMatrix<f64>, psi0: &DVector<f64>) -> Vec<DVector<f64>> {
        let steps = (self.t_max / self.dt).round().max(1.0) as usize;
        let mut psi = psi0.clone();
        let mut traj = Vec::with_capacity(steps + 1);
        traj.push(psi.clone());
        for _ in 0..steps {
            let k1 = h * &psi;
            let k2 = h * &(psi.clone() + k1.clone() * (0.5 * self.dt));
            let k3 = h * &(psi.clone() + k2.clone() * (0.5 * self.dt));
            let k4 = h * &(psi.clone() + k3.clone() * self.dt);
            psi += (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (self.dt / 6.0);
            traj.push(psi.clone());
        }
        traj
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_hamiltonian_preserves_initial_state() {
        let n = 4;
        let h = DMatrix::<f64>::zeros(n, n);
        let psi0 = DVector::from_fn(n, |i, _| (i as f64) + 1.0);
        let solver = Rk4Solver::new(0.01, 0.5);
        let norms = solver.solve(&h, &psi0);
        assert!(
            norms.iter().all(|x| (x - 30.0).abs() < 1e-12),
            "H=0 must leave the state untouched"
        );
    }

    #[test]
    fn skew_hamiltonian_conserves_total_amplitude() {
        let n = 8;
        // Pure anti-symmetric transport (no gain/loss) must conserve Σψ_i².
        let mut c = DMatrix::zeros(n, n);
        for i in 0..n {
            for j in (i + 1)..n {
                c[(i, j)] = 0.3 + 0.05 * ((i * 7 + j) as f64 % 1.0);
                c[(j, i)] = -c[(i, j)];
            }
        }
        let psi0 = DVector::from_fn(n, |i, _| 1.0 + 0.1 * (i as f64));
        let norm = psi0.norm();
        let psi0 = psi0 / norm;        let solver = Rk4Solver::new(0.001, 0.5);
        let norms = solver.solve(&c, &psi0);
        let max_dev = norms.iter().map(|x| (x - 1.0).abs()).fold(0.0, f64::max);
        assert!(max_dev < 1e-6, "skew transport must conserve the total amplitude");
    }
}
