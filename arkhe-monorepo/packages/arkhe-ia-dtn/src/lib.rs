//! ARKHE IA DTN — Delay/Disruption Tolerant Networking agent for deep space.
//!
//! The crate implements the full in-flight data path for a DTN agent:
//!
//! * [`bpv7`] — real BPv7 bundles + BPSec (RFC 9171/9173) via `hardy-bpv7`,
//!   Fase 1 (std only).
//! * [`discovery`] — BPv7 unidirectional beacon discovery and spectral scan (P1).
//! * [`cgr`] — Contact Graph Routing with `propagation_delay` (P4).
//! * [`q_learning`] — Q-Learning with a capacity-bounded (LRU) Q-table and
//!   exponential epsilon decay (P8, P10).
//! * [`processing`] — on-board light-model inference (MobileNetV3-INT8) with
//!   FEC and fragment sizing (P12).
//! * [`epistemic_audit`] — bridge to `audit_epistemic_v2.py` with a temporal
//!   evidence bank and adaptive fail-closed threshold (P14, P15).
//! * [`optical_convergence`] — optical links: RaptorQ FEC + QR Code (Fase 4,
//!   std only).
//! * [`orchestrator`] — main loop with fail-closed (SafeCgr), adaptive
//!   reconnect backoff and event-driven auditing (P16, P17, P18).

#![deny(unsafe_code)]
extern crate alloc;

pub mod cgr;
#[cfg(feature = "std")]
pub mod bpv7;
pub mod discovery;
pub mod epistemic_audit;
pub mod orchestrator;
#[cfg(feature = "std")]
pub mod optical_convergence;
pub mod processing;
pub mod q_learning;

pub const VERSION: &str = "0.1.0-ARKHE-IA-DTN-2026-08-03";
