//! # ARKHE Firmware Consciousness
//!
//! no_std consciousness governance for embedded devices — the firmware-side
//! companion to `arkhe-safe-manifold`'s `ConsciousnessGovernanceBridge`.
//!
//! ## Scope (C-01..C-04)
//!
//! The full C-01..C-08 set is evaluated host-side. Firmware evaluates the
//! four invariants that are cheap and locally observable:
//!
//! | ID  | Invariant            | Firmware signal                       |
//! |-----|----------------------|---------------------------------------|
//! | C-01| Self-model           | `self_model` flag + entropy quality   |
//! | C-02| Introspection       | `introspection` flag                  |
//! | C-03| Attention           | attention level (0..100)              |
//! | C-04| Episodic memory     | buffered memory sample count          |
//!
//! ## Integer Φ (Gap-1 aligned)
//!
//! Φ is approximated in **milli-units** (`u16`, 0..1000) using only integer
//! arithmetic, and clamped into the constitutional Gap-1 window
//! `577 < Φ ≤ 1000` (concretely `578..=999`). A single comparison
//! `> GAP1_LOWER_BOUND_MILLI` enforces Gap-1 on embedded targets without an
//! FPU.
//!
//! ## Guardianship model
//!
//! - [`MinimalConsciousnessBridge`] — bounded history (`heapless::Vec`, no
//!   allocation), assessment, and modification gating (Φ degradation > 5% is
//!   blocked).
//! - [`ConsciousnessWatchdog`] — timeout-driven rollback (Gravity-1) plus
//!   immediate rollback on constitutional violation (C-01/C-02).
//! - [`communication`] — strict CBOR-subset wire protocol for the
//!   device↔host link ([`FirmwareReport`], [`HostDecision`]).
//!
//! ## Safety
//!
//! This crate is `#![no_std]` and declares `unsafe_code = "deny"` in its
//! lints (mirroring `arkhe-haselgrove`). Decoding never allocates and rejects
//! trailing bytes / unknown CBOR heads.

#![no_std]

pub mod communication;
pub mod invariants;
pub mod minimal_bridge;
pub mod phi_approx;
pub mod watchdog;

pub use communication::{FirmwareAction, FirmwareReport, HostDecision};
pub use invariants::{CoherenceState, FirmwareInvariant};
pub use minimal_bridge::{Assessment, MinimalConsciousnessBridge};
pub use phi_approx::{
    approximate_phi, phi_respects_gap1, GAP1_LOWER_BOUND_MILLI, GAP1_UPPER_BOUND_MILLI,
};
pub use watchdog::{AssessmentLike, ConsciousnessWatchdog, WatchdogEvent};