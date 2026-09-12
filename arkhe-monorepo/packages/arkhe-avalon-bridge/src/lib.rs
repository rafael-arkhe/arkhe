//! # AVALON RF bridge (PyO3)
//!
//! Exposes the Arkhe OS AGC controller and Safe-Core evidence to Python.
//!
//! ```text
//!   Python (Avalon ensemble)  <──>  avalon_rf_bridge  <──>  arkhe-avalon-agc
//! ```
//!
//! ## Build requirements (wheel)
//!
//! The PyO3 feature is **off by default** so the monorepo compiles without a
//! CPython interpreter (CI-safe). To produce the wheel:
//!
//! ```text
//!   pip install maturin
//!   cd packages/arkhe-avalon-bridge
//!   maturin develop --release      # or: maturin build --release
//! ```
//!
//! > ⚠️ **Python 3.14 note:** PyO3 pin must support CPython 3.14. On this
//! > machine Python 3.14.2 (64-bit) is installed but `maturin` is absent; the
//! > [architecture] imports compile via `--no-default-features`.
//!
//! ## Safety
//!
//! The FFI surface is thin and `unsafe`-free: objects are constructed in the
//! core crate (`arkhe-avalon-agc`, `#![deny(unsafe_code)]`) and only *moved*
//! across the boundary through pyclass wrappers. Bounds checks and float
//! validation happen in the core crate's [`Untrusted`]-gate, never here.

#![deny(unsafe_code)]

#[cfg(feature = "pyo3")]
mod agc;
#[cfg(feature = "pyo3")]
mod evidence;

/// Pymodule entry point (`avalon_rf_bridge`).
///
/// Only compiled when the `pyo3` feature is on. `maturin` wires this into the
/// Python `avalon_rf_bridge` module.
#[cfg(feature = "pyo3")]
#[pyo3::pymodule]
fn avalon_rf_bridge(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    use pyo3::types::PyModuleMethods;
    agc::register(m)?;
    evidence::register(m)?;
    m.add("ATTACHED_TO_ENSEMBLE", true)?;
    Ok(())
}

/// Marked as attached to the Python ensemble so it is not treated as free
/// evidence by Safe-Core's provenance checks (Provenance-1).
pub const ATTACHED_TO_ENSEMBLE: bool = true;