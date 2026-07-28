#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod error;
pub mod types;
pub mod derivation;

#[cfg(feature = "std")]
pub mod mnemonic;

#[cfg(feature = "std")]
pub mod did;

#[cfg(feature = "flock")]
pub mod flock_bridge;

#[cfg(feature = "qdrant-resolver")]
pub mod did_resolver_qdrant;

#[cfg(feature = "std")]
pub use mnemonic::ArkheMnemonic;

pub use derivation::{derive_master_identity, pqc};

#[cfg(feature = "std")]
pub use did::{build_did, generate_did_document, resolve_did};

#[cfg(feature = "std")]
pub use types::*;

pub const VERSION: &str = "0.1.0-ARKHE-2026-07-28";