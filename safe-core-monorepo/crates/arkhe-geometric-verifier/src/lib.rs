//! FI-120–FI-125 — geometric verification of agent turns, and the typed
//! graphs built from them.
//!
//! Three things live here, deliberately kept separable:
//!
//! - [`verifier`] — [`GeometricVerifier`]: canonicalises a record into
//!   deterministic bytes, hashes those bytes into a
//!   [`GeometricCoordinate`](verifier::GeometricCoordinate), and runs every
//!   registered [`VerificationRule`](verifier::VerificationRule) against it.
//!   Synchronous by design: `arkhe-agi`'s `AgiCoordinator::process` calls it
//!   between two `await`s on the turn's critical path, where there is no I/O
//!   to hide behind an async state machine.
//! - [`memory_graph`] — FI-124: a [`TypedGraph`] of stored memory entries and
//!   the working/episodic links between them.
//! - [`provenance_graph`] — FI-122: a [`TypedGraph`] over an
//!   [`EvidenceChain`](arkhe_evidence::EvidenceChain), one node per record,
//!   validating that the hash links still hold.
//!
//! The types in [`graph`] are the shared substrate; they carry no policy.
//!
//! ```
//! use arkhe_geometric_verifier::{GeometricVerifier, VerifiableRecord};
//!
//! struct Turn {
//!     response: String,
//! }
//!
//! impl VerifiableRecord for Turn {
//!     fn user_input(&self) -> &str {
//!         ""
//!     }
//!     fn response(&self) -> &str {
//!         &self.response
//!     }
//! }
//!
//! struct NonEmpty;
//!
//! impl arkhe_geometric_verifier::VerificationRule for NonEmpty {
//!     fn check(&self, record: &dyn VerifiableRecord) -> Result<(), String> {
//!         if record.response().is_empty() {
//!             Err("response is empty".to_string())
//!         } else {
//!             Ok(())
//!         }
//!     }
//! }
//!
//! let mut verifier = GeometricVerifier::new();
//! verifier.register(NonEmpty);
//!
//! let artifact = verifier.verify(&Turn { response: "ok".to_string() }).unwrap();
//! assert!(!artifact.canonical_bytes.is_empty());
//!
//! let rejected = verifier.verify(&Turn { response: String::new() }).unwrap_err();
//! assert!(rejected.to_string().contains("response is empty"));
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod graph;
pub mod memory_graph;
pub mod provenance_graph;
pub mod verifier;

pub use graph::TypedGraph;
pub use memory_graph::{MemoryEdge, MemoryGraphError, MemoryNode};
pub use provenance_graph::{ProvenanceEdge, ProvenanceError, ProvenanceNode};
pub use verifier::{
    GeometricCoordinate, GeometricError, GeometricVerifier, VerifiableRecord, VerificationRule,
    VerifiedArtifact,
};
