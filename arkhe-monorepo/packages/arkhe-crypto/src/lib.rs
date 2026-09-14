//! arkhe-crypto — DKG failure detection, hash security, BIP-322 verification.
//!
//! Statutory note:
//!   - `dkg` and `hash` are `[DEDUTIVO]` — pure logic, no external deps.
//!   - `bip322` stub path is `[VERIFICADO]` by execution (10/10 tests).
//!   - `bip322` feature path is `[VERIFICADO]` by execution (9/9 tests,
//!     `cargo test --features bip322`; 2026-09-12) and `[EXECUTADO-CRUZADO]`
//!     against an independent implementation (`bip322` 0.0.12).

pub mod bip322;
pub mod dkg;
pub mod error;
pub mod hash;

pub use dkg::is_dkg_failed;
pub use error::{CryptoError, Result};
pub use hash::HashSecurity;