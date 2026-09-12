#![deny(unsafe_code)]

//! ARKHE XY Simulator (substrate proposal: Cathedral V8.6 — malha NxN de
//! skyrmions/Hopfions).
//!
//! Four phase-lattice extensions connecting the V8.6 computational model to
//! the experimental fronts F1-F5. All simulations are deterministic given a
//! seed (shared [`Rng`] = glibc-style LCG, no `thread_rng`).
//!
//! * **P1 — Kuramoto** ([`p1_kuramoto`]): natural frequency disorder
//!   (Gaussian / Lorentzian) + bidirectional PLVR pair (EEG <-> laser).
//! * **P2 — Langevin** ([`p2_langevin`]): finite temperature, Kosterlitz-
//!   Thouless regime, Binder cumulant estimator, energy-floor verification.
//! * **P3 — DMI / chiral** ([`p3_dmi`]): TWO realizations, per the 2026-08
//!   audit: (a) the driving dynamics from the original proposal (relaxes to a
//!   coherently-rotating state — documented falsification of a static pitch)
//!   and (b) gradient relaxation of the energy functional, which does produce
//!   chiral pitch / domain fragmentation.
//! * **P4 — Pinned defects** ([`p4_pinned`]): pin-mask sites with fixed phase
//!   (topological-charge centres acting as nucleation points).
//!
//! AVISOS CIENTIFICOS (heredados do [`hopfion_mesh_cathedral.py`]):
//!   1. Estes sao modelos de fase (XY), nao micromagnetismo (OOMMF/MUMAX3).
//!      Q_total = N^2*Q_max continua CONTAGEM; a coerencia multiplicativa
//!      (Hopfion fracionario) nao esta implementada.
//!   2. P2 fornece T_KT em unidades do Hamiltoniano H = -(J/2)*sum cos, com
//!      J = K_xy/2. Para K=0.6 (J=0.3), T_KT ~= 0.89*J ~= 0.267 — nao 0.53
//!      nem 0.45 como o documento original afirmava; ate 5 seeds o desvio
//!      padrao e reportado.
//!   3. P3 (dinamica) nao produz espiral estatica: sin(Delta + alpha) tem
//!      ponto fixo em rotacao uniforme (R -> 1), nao em pitch 2*pi/alpha.
//!      O pitch / fragmentacao em dominios pertence ao funcao de energia
//!      H = -(J/2) sum cos(Delta - alpha), resolvida por relaxacao.
//!   4. Veto de Anubis / decoerencia do EVO e narrativa; aqui so existem
//!      observaveis de controle (diff de fase, lock time, par. de ordem).

pub mod diagnostics;
pub mod lattice;
pub mod p1_kuramoto;
pub mod p2_langevin;
pub mod p3_dmi;
pub mod p4_pinned;
pub mod rng;

pub use lattice::Lattice;
pub use p1_kuramoto::{FreqDist, KuramotoConfig, KuramotoResult, KuramotoSim, PlvrPair, PlvrResult};
pub use p2_langevin::{
    LangevinConfig, LangevinResult, LangevinSim, T_KT_FACTOR,
};
pub use p3_dmi::{
    ChiralConfig, DmiDriveSim, DmiResult, relax_chain_pitch, relax_square_domains,
};
pub use p4_pinned::{PinnedConfig, PinnedSim};
pub use rng::Rng;

pub const VERSION: &str = "0.1.0-ARKHE-XY-SIMULATOR-2026-08-16";

/// Wrap an angle into `[-pi, pi)`.
///
/// Branch-based (two subtractions) instead of `rem_euclid` — safe because a
/// single RK4 step never moves a phase by more than the wrap window; keeps the
/// hot loop out of the slow IEEE remainder path.
#[inline]
pub fn wrap_pi(a: f64) -> f64 {
    let two_pi = std::f64::consts::TAU;
    let mut x = a;
    while x >= std::f64::consts::PI {
        x -= two_pi;
    }
    while x < -std::f64::consts::PI {
        x += two_pi;
    }
    x
}