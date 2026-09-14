//! # arkhe-soc-tlm
//!
//! Transaction-level golden model (TLM) for the Arkhe SoC. It is the reference
//! the RTL is checked against — not a claim about silicon. Modules:
//!
//! * [`smith`]   — Smith-chart AFE: (I,Q) → coupling = 1 - |Γ|², a bit-exact port
//!   of `arkhe-soc-rtl/model/cordic_ref.py`.
//! * [`payload`] — canonical 136-byte AOTB payload (integers LE, f64 BE).
//! * [`aotb`]    — Ed25519 encoder/verifier with **soft sync**.
//! * [`sram`]    — SRAM D/X double buffer + IFS expansion.
//! * [`qpl`]     — ring convolution `(left+center+right)/3` + perf counters.
//! * [`soc`]     — integrator wiring the pieces into one emit→verify cycle.

pub mod aotb;
pub mod payload;
pub mod qpl;
pub mod smith;
pub mod soc;
pub mod sram;

pub use payload::DOMAIN_NODES;

/// Reference latency from arXiv:2607.16100 (small AllReduce, 2.37 µs).
///
/// NOTE: this paper is cited by the project docs but has **not** been
/// independently verified here. It is a target to compare against, not a
/// validated result. Do not report the TLM as "validated against" it.
pub const REFERENCE_SOL_US: f64 = 2.37;
