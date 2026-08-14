#![deny(unsafe_code)]

//! ARKHE Neurogenesis (substrate 344-AGENT-NEUROGENESIS) — "Denáritas do Fogo".
//!
//! A deterministic, reproducible dendritic-growth simulator. A lattice of `n`
//! sites carries a real amplitude field `ψ` whose transport is governed by the
//! non-Hermitian evolution equation
//!
//! ```text
//! dψ/dt = H ψ,   H = C + D
//! ```
//!
//! where `C` is a real anti-symmetric coupling matrix (conserving transport)
//! and `D = diag(gain_loss)` is a diagonal gain/loss term. Whenever a site
//! amplitude `|ψ_i|²` exceeds the configured threshold, a dendrite branch
//! grows off that site. Everything is reproducible from the
//! [`SimulationConfig`] tuple `(n, dt, t_max, seed, threshold, growth_allowed,
//! parity_time_sym)`.
//!
//! The optional parity–time (PT) symmetry mirrors the gain/loss vector
//! (`gain_loss[i] = -gain_loss[n-1-i]`), balancing total gain against total
//! loss so the total amplitude stays bounded instead of running away.

pub mod hamiltonian;
pub mod simulator;
pub mod solver;
pub mod tree;

pub use hamiltonian::Hamiltonian;
pub use simulator::{SimulationConfig, SimulationResult};
pub use solver::Rk4Solver;
pub use tree::{GrowEvent, Neuron};

/// Run a simulation from a configuration (alias for [`SimulationConfig::run`]).
pub fn simulate(config: &SimulationConfig) -> SimulationResult {
    config.run()
}

pub const VERSION: &str = "0.1.0-ARKHE-NEUROGENESIS-2026-08-02";
