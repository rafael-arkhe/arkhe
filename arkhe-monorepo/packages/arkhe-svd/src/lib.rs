#![deny(unsafe_code)]

//! ARKHE Block 20 — SVD Compression Protocol.
//!
//! Truncated singular-value-decomposition compression for EVO/state matrices
//! and a coherence-based frame selector for the Fountain transport layer.
//!
//! The SVD runs on `nalgebra`'s pure-Rust backend, so this crate builds on any
//! platform without a BLAS/LAPACK toolchain. The binary layout is stable and
//! self-contained, so a compressed matrix can be serialized into bytes,
//! transported over the Fountain protocol, and reconstructed peer-side.

pub mod error;
pub mod svd;

pub use error::SvdError;
pub use svd::{compress_evo, svd_frame_selector, SvdCompressed};

pub const VERSION: &str = "0.1.0-ARKHE-SVD-2026-08-02";