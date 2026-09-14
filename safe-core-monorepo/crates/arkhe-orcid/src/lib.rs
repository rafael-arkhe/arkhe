//! ORCID iD support for Arkhe OS.
//!
//! Two independent halves, kept apart so that neither depends on the
//! other's failure modes:
//!
//! - [`OrcidId`] — pure, offline parsing and checksum validation of the
//!   `XXXX-XXXX-XXXX-XXXX` identifier (ISO 7064 mod 11-2 check character).
//!   No network, no I/O, cannot fail for environmental reasons.
//! - [`OrcidClient`] — verification of an [`OrcidId`] against the ORCID
//!   public API (`GET {base_url}/{orcid}/person`), returning the
//!   researcher's [`OrcidVerification`] (display name plus the raw name
//!   parts).
//!
//! The two are used together by `arkhe-agi`'s `AgiCoordinator`, which
//! caches a verified identity and uses it to attest every turn's
//! provenance record — see `AgiCoordinator::attest_with_orcid`.
//!
//! ```no_run
//! # async fn example() -> Result<(), arkhe_orcid::OrcidError> {
//! let id = arkhe_orcid::OrcidId::parse("0000-0002-1825-0097")?;
//! let client = arkhe_orcid::OrcidClient::new();
//! let verified = client.verify(&id).await?;
//! assert_eq!(verified.id, id);
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod client;
pub mod error;
pub mod id;

pub use client::{OrcidClient, OrcidVerification, DEFAULT_BASE_URL};
pub use error::OrcidError;
pub use id::OrcidId;
