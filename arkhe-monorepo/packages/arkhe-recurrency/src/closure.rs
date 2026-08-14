//! Causal-closure measurement — the falsifiable core of Table 2.
//!
//! **Depth of causal closure** (paper glossary): *the degree to which the
//! successive states of a system are determined by its own current and
//! preceding states, relative to its exogenous drivers.*
//!
//! Operationalization, per site `i`, with `y = states[t+1][i]`:
//!
//! * **exogenous model:** `y ~ drive[t][i]` — how much of the future is
//!   explained by the input alone (`R²_exo`);
//! * **closed-loop model:** `y ~ states[t][i]` — how much of the future is
//!   explained by the system's own state (`R²_self`).
//!
//! Depth is `R²_self − R²_exo`. This separates three regimes cleanly:
//!
//! * feedforward copy (`state = input`): both models coincide → depth `≈ 0`;
//! * shallow memory (low-pass, no internal recurrency): both `R²` are high and
//!   close → depth small;
//! * transport-dominated recurrency (state decorrelates from the input): input
//!   alone predicts poorly, own state predicts well → depth large.
//!
//! Both models are ridge-regularised least squares, so the measurement is
//! deterministic and does not overfit on short trajectories.

use nalgebra::{DMatrix, DVector};

/// Result of a causal-closure measurement.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct ClosureReport {
    /// `R²_self − R²_exo`, clamped to `[0, 1]`.
    pub depth: f64,
    /// Total squared prediction error of the exogenous (input-only) model.
    pub exogenous_mse: f64,
    /// Total squared prediction error of the closed-loop (own-state) model.
    pub closed_loop_mse: f64,
}

/// Measure the depth of causal closure of a trajectory.
///
/// `states` and `drive` must be equally long (≥ 4 entries). Each entry of
/// `drive` is the exogenous input present at that step; each entry of `states`
/// is the system's internal state. Sites are regressed independently and the
/// squared errors and the total variance are summed across sites.
pub fn closure_depth(states: &[DVector<f64>], drive: &[DVector<f64>]) -> ClosureReport {
    assert!(
        states.len() == drive.len(),
        "states and drive must be equally long"
    );
    let n_sites = states[0].len();
    let n_steps = states.len();
    let mut exo_err = 0.0;
    let mut self_err = 0.0;
    let mut var_total = 0.0;
    for site in 0..n_sites {
        if n_steps < 4 {
            continue;
        }
        let mut y = Vec::with_capacity(n_steps - 1);
        let mut x_exo = Vec::with_capacity(n_steps - 1);
        let mut x_self = Vec::with_capacity(n_steps - 1);
        for t in 0..n_steps - 1 {
            y.push(states[t + 1][site]);
            x_exo.push([1.0, drive[t][site]]);
            x_self.push([1.0, states[t][site]]);
        }
        let y = DVector::from_iterator(y.len(), y);
        let x_exo = DMatrix::from_fn(y.len(), 2, |r, c| x_exo[r][c]);
        let x_self = DMatrix::from_fn(y.len(), 2, |r, c| x_self[r][c]);
        let var_y = y.variance() * y.len() as f64;
        var_total += var_y;
        if var_y < 1e-12 {
            continue;
        }
        if let (Some(e1), Some(e2)) = (
            ridge_prediction_error(&x_exo, &y, 1e-6),
            ridge_prediction_error(&x_self, &y, 1e-6),
        ) {
            exo_err += e1;
            self_err += e2;
        }
    }
    let depth = if var_total > 1e-12 {
        (exo_err - self_err).max(0.0) / var_total
    } else {
        0.0
    };
    ClosureReport {
        depth: depth.clamp(0.0, 1.0),
        exogenous_mse: exo_err,
        closed_loop_mse: self_err,
    }
}

/// Sum of squared prediction errors of `y ~ X` with ridge-regularised LS.
///
/// (Sum — not mean — so it is directly comparable with `var_y`, which is also
/// a sum of squared deviations.)
fn ridge_prediction_error(x: &DMatrix<f64>, y: &DVector<f64>, ridge: f64) -> Option<f64> {
    let beta = ridge_solve(x, y, ridge)?;
    let pred = x * &beta;
    Some((pred - y).norm_squared())
}

/// Solve `β = (XᵀX + λI)⁻¹ Xᵀy` via LU.
fn ridge_solve(x: &DMatrix<f64>, y: &DVector<f64>, ridge: f64) -> Option<DVector<f64>> {
    let xtx = x.transpose() * x;
    let xty = x.transpose() * y;
    let reg = DMatrix::identity(x.ncols(), x.ncols()) * ridge;
    (xtx + reg).lu().solve(&xty)
}

/// Mean cosine similarity between two equally long trajectories.
///
/// Used for the perturbational test: `1.0` means the perturbation left no
/// trace (feedforward-like), values below `1.0` mean the perturbation
/// propagated through the system's own recurrency.
pub fn perturbational_agreement(base: &[DVector<f64>], probe: &[DVector<f64>]) -> f64 {
    assert_eq!(base.len(), probe.len(), "trajectories must be equally long");
    if base.is_empty() {
        return 1.0;
    }
    let mut acc = 0.0;
    for (a, b) in base.iter().zip(probe) {
        acc += cosine(a, b);
    }
    acc / base.len() as f64
}

fn cosine(a: &DVector<f64>, b: &DVector<f64>) -> f64 {
    let na = a.norm();
    let nb = b.norm();
    if na <= 1e-12 || nb <= 1e-12 {
        1.0
    } else {
        (a.dot(b) / (na * nb)).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smooth_drive(n_sites: usize, t_max: usize) -> Vec<DVector<f64>> {
        (0..t_max)
            .map(|t| {
                DVector::from_fn(n_sites, |i, _| {
                    (0.5 * (i + 1) as f64 * t as f64 * 0.02).sin() + 0.1 * (i as f64)
                })
            })
            .collect()
    }

    /// A purely feedforward system: `state[t] == drive[t]` (no memory).
    #[test]
    fn feedforward_trajectory_scores_zero_depth() {
        let drive = smooth_drive(4, 80);
        let states = drive.clone();
        let rep = closure_depth(&states, &drive);
        assert!(rep.depth < 0.1, "feedforward must score ~0, got {}", rep.depth);
    }

    /// A system whose next state is dominated by its own (rotational) dynamics
    /// must score above 0: the slow input cannot predict the fast rotation.
    #[test]
    fn self_determined_trajectory_scores_above_zero() {
        let drive = smooth_drive(3, 80);
        // Undamped rotation (0.15 rad/step) with negligible forcing: the state
        // is generated by its own dynamics, not shaped by the input.
        let theta = 0.15_f64;
        let rot = DMatrix::from_fn(3, 3, |i, j| match (i, j) {
            (0, 0) => theta.cos(),
            (0, 1) => theta.sin(),
            (1, 0) => -theta.sin(),
            (1, 1) => theta.cos(),
            (2, 2) => 0.9,
            _ => 0.0,
        });
        let mut states = Vec::with_capacity(drive.len());
        let mut psi = drive[0].clone();
        states.push(psi.clone());
        for d in drive.iter().take(drive.len() - 1) {
            psi = &rot * &psi + d.clone() * 0.001;
            states.push(psi.clone());
        }
        let rep = closure_depth(&states, &drive);
        assert!(
            rep.depth > 0.2,
            "self-determined must score above 0, got {}",
            rep.depth
        );
    }

    #[test]
    fn perturbational_agreement_is_high_for_identical_trajectories() {
        let n = 3;
        let v = DVector::from_element(n, 1.0);
        let traj = vec![v.clone(), v.clone() * 2.0];
        let same = traj.clone();
        assert!((perturbational_agreement(&traj, &same) - 1.0).abs() < 1e-9);
    }
}
