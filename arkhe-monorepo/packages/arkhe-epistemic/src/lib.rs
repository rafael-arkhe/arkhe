//! # arkhe-epistemic
//!
//! Módulo epistémico do ARKHE OS.
//!
//! Fornece:
//! - Verificação aritmética de `a(p) ≠ p³ − p²` (sequência A051193)
//! - Estados epistémicos (`VerifiedStatus`, `UnverifiedStatus`)
//! - Ponte para o Lean 4 (`ArkheEpistemic.lean`)
//!
//! ## Invariantes constitucionais impactados
//!
//! - **Ghost-3 / Correlation-1**: toda proposição verificada aqui é âncora
//!   de referência cruzada verificável.
//! - **Simplicity-1 / Simplicity-2**: dependências mínimas (serde, thiserror,
//!   num-bigint), sem async, sem unsafe.

pub mod arithmetic;
pub mod lean_bridge;
pub mod status;

pub use arithmetic::{a, verify_a_neq_p3_minus_p2, A051193};
pub use lean_bridge::{LeanResult, LeanVerifier};
pub use status::{EpistemicStatus, UnverifiedStatus, VerifiedStatus};

/// Versão semântica do módulo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
