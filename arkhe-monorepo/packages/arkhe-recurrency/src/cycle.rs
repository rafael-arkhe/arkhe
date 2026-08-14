//! Two-pass predictive loop — the *local* level of recurrency.
//!
//! Local recurrency is what turns a feedforward projection into a phenomenal
//! percept (V1 ↔ V4-style recurrent processing). Here it is simulated as a
//! two-pass cycle on top of the [`arkhe_neurogenesis`] substrate:
//!
//! 1. **Pass 1 (feedforward prediction):** the stimulus is evolved through the
//!    substrate's non-Hermitian dynamics with no corrective feedback. This is
//!    the "raw" read-out.
//! 2. **Pass 2 (error correction):** the evolution is re-run with a corrective
//!    term `k(s − ψ)` — the prediction error closes the loop and refines the
//!    representation toward the stimulus.
//!
//! The loop is *closed* iff pass 2 measurably reduces the reconstruction error
//! relative to pass 1. A feedforward-only system has no such reduction.

use arkhe_neurogenesis::Hamiltonian;
use nalgebra::{DMatrix, DVector};

/// A completed two-pass perception cycle.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Percept {
    /// Final representation after the error-correcting pass.
    pub representation: DVector<f64>,
    /// Squared reconstruction error `‖r_ff − s‖²` after the feedforward pass.
    pub feedforward_error: f64,
    /// Squared reconstruction error `‖r_closed − s‖²` after the closed pass.
    pub corrected_error: f64,
    /// Relative error reduction `(err_ff − err_closed) / err_ff`.
    pub error_reduction: f64,
    /// Whether the loop closed: `error_reduction > closure_threshold`.
    pub local_recurrence: bool,
}

/// Configurable two-pass predictive loop on the neurogenesis substrate.
#[derive(Clone, Debug)]
pub struct PerceptionLoop {
    pub n: usize,
    pub seed: u64,
    pub dt: f64,
    pub steps: usize,
    /// Scale applied to the substrate coupling/gain after construction.
    pub coupling_scale: f64,
    /// Scale on the substrate's own dynamics inside the closed pass.
    pub recurrence_gain: f64,
    /// Gain `k` of the corrective term `k(s − ψ)` in the closed pass.
    pub feedback_gain: f64,
    /// Relative error reduction above which the loop counts as closed.
    pub closure_threshold: f64,
    /// Whether to balance gain/loss so the total amplitude stays bounded.
    pub parity_time_sym: bool,
}

impl Default for PerceptionLoop {
    fn default() -> Self {
        Self {
            n: 8,
            seed: 7,
            dt: 0.02,
            steps: 50,
            coupling_scale: 1.0,
            recurrence_gain: 1.0,
            feedback_gain: 2.0,
            closure_threshold: 1e-3,
            parity_time_sym: true,
        }
    }
}

impl PerceptionLoop {
    /// Build the substrate generator `H = C + D` for this loop.
    pub fn hamiltonian(&self) -> DMatrix<f64> {
        let mut h = Hamiltonian::antisymmetric(self.n, self.seed);
        if self.parity_time_sym {
            h.add_parity_time_symmetry();
        }
        h.coupling *= self.coupling_scale;
        h.gain_loss *= self.coupling_scale;
        h.hamiltonian()
    }

    /// Run the two-pass cycle on a static `stimulus`.
    pub fn perceive(&self, stimulus: &DVector<f64>) -> Percept {
        let s = normalize(stimulus);
        let h = self.hamiltonian();
        let r_ff = self.integrate(&h, &s, None);
        let r_closed = self.integrate(&h, &s, Some(&s));
        let err_ff = (&r_ff - &s).norm_squared();
        let err_closed = (&r_closed - &s).norm_squared();
        let reduction = if err_ff > 1e-12 {
            (err_ff - err_closed) / err_ff
        } else {
            0.0
        };
        Percept {
            representation: r_closed,
            feedforward_error: err_ff,
            corrected_error: err_closed,
            error_reduction: reduction.max(0.0),
            local_recurrence: reduction > self.closure_threshold,
        }
    }

    /// Integrate `steps` RK4 steps of the substrate dynamics.
    ///
    /// With `target = None` the raw feedforward dynamics `dψ/dt = Hψ` are
    /// integrated; with `target = Some(s)` the closed dynamics
    /// `dψ/dt = g·Hψ + k(s − ψ)` are integrated.
    fn integrate(
        &self,
        h: &DMatrix<f64>,
        psi0: &DVector<f64>,
        target: Option<&DVector<f64>>,
    ) -> DVector<f64> {
        let mut psi = psi0.clone();
        for _ in 0..self.steps {
            let field = |p: &DVector<f64>| match target {
                Some(t) => self.recurrence_gain * (h * p) + self.feedback_gain * (t - p),
                None => self.recurrence_gain * (h * p),
            };
            let k1 = field(&psi);
            let k2 = field(&(psi.clone() + k1.clone() * (0.5 * self.dt)));
            let k3 = field(&(psi.clone() + k2.clone() * (0.5 * self.dt)));
            let k4 = field(&(psi.clone() + k3.clone() * self.dt));
            psi += (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (self.dt / 6.0);
        }
        psi
    }

    /// Evolve the closed dynamics through a time-varying stimulus sequence.
    ///
    /// Returns `(states, drive)` where `states[t]` is the internal state after
    /// observing `drive[t]`. The recurrence gain `g` controls how strongly the
    /// system's own state — rather than the exogenous drive — determines the
    /// next state; this trajectory feeds [`crate::closure_depth`].
    pub fn closed_loop_trajectory(
        &self,
        stimuli: &[DVector<f64>],
    ) -> (Vec<DVector<f64>>, Vec<DVector<f64>>) {
        let h = self.hamiltonian();
        let mut states = Vec::with_capacity(stimuli.len());
        let mut psi = stimuli[0].clone();
        states.push(psi.clone());
        for target in stimuli.iter().take(stimuli.len().saturating_sub(1)) {
            let field = |p: &DVector<f64>| {
                self.recurrence_gain * (&h * p) + self.feedback_gain * (target - p)
            };
            let k1 = field(&psi);
            let k2 = field(&(psi.clone() + k1.clone() * (0.5 * self.dt)));
            let k3 = field(&(psi.clone() + k2.clone() * (0.5 * self.dt)));
            let k4 = field(&(psi.clone() + k3.clone() * self.dt));
            psi += (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (self.dt / 6.0);
            states.push(psi.clone());
        }
        (states, stimuli.to_vec())
    }

    /// Perturbational test: perturb one site of the internal state mid-stream
    /// and measure how much the remaining trajectory diverges from baseline.
    ///
    /// Returns the mean cosine similarity between the baseline and the
    /// perturbed trajectory *after* the perturbation site. A value near `1.0`
    /// means the perturbation left no trace (feedforward-like); a value well
    /// below `1.0` means the system's own recurrency propagated the
    /// perturbation (Table 2, state gradient).
    pub fn perturbational_agreement(
        &self,
        stimuli: &[DVector<f64>],
        perturb_at: usize,
        perturb_site: usize,
        amount: f64,
    ) -> f64 {
        let (base, _) = self.closed_loop_trajectory(stimuli);
        let mut probe_states = Vec::with_capacity(stimuli.len());
        let h = self.hamiltonian();
        let mut psi = stimuli[0].clone();
        probe_states.push(psi.clone());
        for (t, target) in stimuli.iter().take(stimuli.len().saturating_sub(1)).enumerate() {
            let field = |p: &DVector<f64>| {
                self.recurrence_gain * (&h * p) + self.feedback_gain * (target - p)
            };
            let k1 = field(&psi);
            let k2 = field(&(psi.clone() + k1.clone() * (0.5 * self.dt)));
            let k3 = field(&(psi.clone() + k2.clone() * (0.5 * self.dt)));
            let k4 = field(&(psi.clone() + k3.clone() * self.dt));
            psi += (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (self.dt / 6.0);
            if t == perturb_at && perturb_site < psi.len() {
                psi[perturb_site] += amount;
            }
            probe_states.push(psi.clone());
        }
        let n = stimuli.len();
        let start = (perturb_at + 1).min(n);
        if start >= n {
            return 1.0;
        }
        let mut acc = 0.0;
        for t in start..n {
            let a = &base[t];
            let b = &probe_states[t];
            acc += cosine(a, b);
        }
        acc / (n - start) as f64
    }
}

/// L2-normalize a vector in place and return it (zero vector is left as-is).
fn normalize(v: &DVector<f64>) -> DVector<f64> {
    let norm = v.norm();
    if norm > 1e-12 {
        v / norm
    } else {
        v.clone()
    }
}

/// Cosine similarity between two non-zero vectors.
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

    #[test]
    fn closed_loop_reduces_reconstruction_error() {
        let loop_cfg = PerceptionLoop::default();
        let s = DVector::from_fn(loop_cfg.n, |i, _| 0.5 + (i as f64) * 0.1);
        let p = loop_cfg.perceive(&s);
        assert!(p.local_recurrence, "loop must close for default config");
        assert!(p.error_reduction > 0.05);
        assert!(p.corrected_error < p.feedforward_error);
    }

    #[test]
    fn construction_is_deterministic() {
        let a = PerceptionLoop::default();
        let b = PerceptionLoop::default();
        let s = DVector::from_fn(a.n, |i, _| 0.5 + (i as f64) * 0.1);
        assert_eq!(a.hamiltonian(), b.hamiltonian());
        assert_eq!(a.perceive(&s).representation, b.perceive(&s).representation);
    }

    #[test]
    fn feedforward_with_zero_feedback_never_closes() {
        let loop_cfg = PerceptionLoop {
            feedback_gain: 0.0,
            ..Default::default()
        };
        let s = DVector::from_fn(loop_cfg.n, |i, _| 0.5 + (i as f64) * 0.1);
        let p = loop_cfg.perceive(&s);
        assert!(!p.local_recurrence, "no feedback term => no error reduction");
    }
}
