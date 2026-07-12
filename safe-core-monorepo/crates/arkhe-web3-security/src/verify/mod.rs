//! Formal-verification harnesses. Only compiled under the `kani` feature +
//! `#[cfg(kani)]` (see `kani_harness.rs`) — never part of a normal build.

#[cfg(kani)]
mod kani_harness;
