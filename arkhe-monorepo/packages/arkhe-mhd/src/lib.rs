#![deny(unsafe_code)]

//! ARKHE Block 21 — Resistive MHD phase-field engine.
//!
//! The Ledger is modelled as a continuous, topological medium whose state is a
//! pseudo-magnetic vector field `Ω` (`B` in the MHD analogy). A **handover**
//! (topological rewriting of the account graph) occurs in the regions where
//! `|∇×Ω|` exceeds a critical level — i.e. magnetic reconnection in the
//! equivalent plasma.
//!
//! The crate provides:
//!
//! * [`mhd::EvoField`] — an explicit-Euler induction solver
//!   `∂Ω/∂t = ∇×(u×Ω) + ν∇²Ω`, i.e. the corrected `advance` that computes the
//!   cross product on the whole grid **before** taking the curl (fixes the
//!   original f64-vs-array typing bug).
//! * [`mhd::helicity`] — the true magnetic helicity `H = ∫ A·Ω dV`, where the
//!   vector potential `A` is recovered from `∇²A = −J` in the Coulomb gauge by
//!   Jacobi relaxation (the previous `Ω·J` scalar was *not* a helicity).
//! * [`retro::RetroChannel`] — a retrocausal echo channel with a
//!   dimensionally-consistent propagation delay.
//!
//! Everything downstream is pure Rust: `ndarray` without its BLAS/LAPACK
//! features, so the crate builds on Windows with no native toolchain.

pub mod error;
pub mod mhd;
pub mod retro;

pub use error::MhdError;
pub use mhd::{helicity, EvoField, PlasmaConfig};
pub use retro::{RetroChannel, RetroSignal};

pub const VERSION: &str = "0.1.0-ARKHE-MHD-2026-08-02";