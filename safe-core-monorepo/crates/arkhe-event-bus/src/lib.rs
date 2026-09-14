//! # `arkhe-event-bus` — Phase 1 typed publish/subscribe, in memory
//!
//! The `arkhe-event-bus` crate of the Arkhe Trust Infrastructure (DOI
//! 10.5281/zenodo.21383201, Table 1). It implements the paper's §5.2 contract
//! literally, and nothing beyond it:
//!
//! > `EventBus / Publisher / Subscriber` — **typed topics** (attestation
//! > completed/failed, storage stored, amendments proposed/applied, kill switch
//! > activated, credentials verified/rejected, alerts); **mandatory `EventMeta`
//! > with identifiers and schema version on every event**; **malformed events
//! > are unpublishable**.
//!
//! | Module | Purpose |
//! |---|---|
//! | [`topic`] | [`Topic`]: validated typed topics, with the nine §5.2 topics as constants and unknown topics accepted (`T-E3`). |
//! | [`meta`] | [`EventMeta`], the four `T-E2` identifiers, [`IdGenerator`], [`SchemaVersion`]. |
//! | [`event`] | [`Event`]: meta + JSON payload + optional signature, and the canonical bytes a signature covers. |
//! | [`signature`] | [`SignatureVerifier`] (pluggable, **no crypto backend here**), [`EventSignature`], `SignaturePolicy`. |
//! | [`log`] | [`EventLog`]: append-only, monotonic [`SequenceNumber`]s, Merkle root, [`Checkpoint`] fencing (`T-13`). |
//! | [`bus`] | [`EventBus`]: fail-closed `publish`, typed `subscribe`, delivery. |
//! | [`invariant`] | The invariants `T-E1`, `T-E2`, `T-E3` (paper-owned IDs) and `EB-01`, `EB-02` (crate-local), with executable checks where one exists. |
//! | [`error`] | [`BusError`], with [`BusError::is_rejection`] separating refusals from transport failures. |
//!
//! ## What this crate is, in one paragraph
//!
//! A domain crate publishes a typed event; the bus validates it, refuses it if
//! the metadata is missing or the identifier, topic or schema version is
//! malformed, verifies its signature if the bus is configured to require one,
//! appends it to a monotonic, Merkle-rooted log, and hands it to every
//! subscriber of its topic. No crate needs to know another. The log — not the
//! channels — is the authority, which is what makes §4.2's promise *"any flow can
//! be replayed or audited from the event stream alone"* a property of the code
//! rather than of the documentation.
//!
//! ## Threat-model mitigations this crate carries
//!
//! From §6.2 of the paper:
//!
//! * **T-13**, *"replay against the event log"* — mitigated here by monotonic
//!   sequence numbers ([`log::EventLog`]), Merkle roots
//!   ([`log::EventLog::current_root`]), signed events ([`signature`]) and
//!   checkpoint fencing ([`log::Checkpoint`], [`log::EventLog::admits_as_new`]).
//! * **T-12**, *"man-in-the-middle between crates"* — the *"per-event Ed25519
//!   signatures verified by the bus"* half is [`bus::EventBus`] plus
//!   [`signature::SignatureVerifier`]. The *"mTLS with 24-hour certificates"*
//!   half is transport and is **not** implemented here.
//!
//! ## What is *not* here
//!
//! Persistence and IPFS, a cryptographic signature backend, mTLS, consensus,
//! Merkle proofs, cross-process uniqueness of `event_id`, and back-pressure
//! beyond the bounded channel. `README.md` lists these explicitly, together
//! with everything this crate does not verify. [`invariant::T_E1`] — *"domain
//! crates never import each other"* — is a property **between** crates and is
//! deliberately not pretended to be checked here.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod bus;
pub mod error;
pub mod event;
pub mod invariant;
pub mod log;
pub mod meta;
pub mod signature;
pub mod topic;

pub use bus::{BusConfig, Delivery, EventBus, Subscription, DEFAULT_CHANNEL_CAPACITY};
pub use error::{BusError, BusResult};
pub use event::Event;
pub use invariant::{BusInvariant, InvariantSource};
pub use log::{merkle_root, Checkpoint, EventLog, LogEntry, MerkleRoot, SequenceNumber};
pub use meta::{
    now_unix_us, CorrelationId, EventId, EventMeta, IdGenerator, SchemaVersion, TraceId,
    MAX_ID_LEN, TIMESTAMP_UNIT,
};
pub use signature::{EventSignature, SignaturePolicy, SignatureVerifier, MAX_SIGNATURE_LEN};
pub use topic::{Topic, CANONICAL_TOPICS, MAX_TOPIC_LEN, MAX_TOPIC_SEGMENTS};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_documented_entry_points_are_reexported_at_the_root() {
        // A compile-time check that the crate root is usable without reaching
        // into modules: the re-exports above are part of the public contract.
        // Closures rather than bare paths because most constructors take
        // `impl Into<…>` / `impl AsRef<…>` and have no single monomorphic
        // signature to coerce to.
        let _: fn(BusConfig) -> EventBus = EventBus::new;
        let _: fn(&str) -> BusResult<Topic> = |value| Topic::parse(value);
        let _: fn(u32, u32) -> BusResult<SchemaVersion> = SchemaVersion::new;
        let _: fn(&str) -> BusResult<EventId> = |value| EventId::new(value);
        let _: fn(&str) -> BusResult<TraceId> = |value| TraceId::new(value);
        let _: fn(&str) -> BusResult<CorrelationId> = |value| CorrelationId::new(value);
        let _: fn() -> u64 = now_unix_us;
        let _: fn(&[[u8; 32]]) -> MerkleRoot = merkle_root;
        let _: fn(&Event) -> BusResult<()> = Event::validate;
        let _ = SignaturePolicy::Optional;
        let _ = EventSignature::from_bytes(Vec::new());

        assert_eq!(CANONICAL_TOPICS.len(), 9);
        assert_eq!(MAX_TOPIC_LEN, 128);
        assert_eq!(MAX_TOPIC_SEGMENTS, 8);
        assert_eq!(MAX_ID_LEN, 128);
        assert_eq!(MAX_SIGNATURE_LEN, 128);
        assert_eq!(TIMESTAMP_UNIT, "unix microseconds");
        assert_eq!(DEFAULT_CHANNEL_CAPACITY, 1024);
        assert_eq!(SequenceNumber::FIRST.get(), 1);
    }

    #[test]
    fn all_declared_invariants_run_and_pass() {
        for (invariant, result) in invariant::check_all() {
            if let Err(error) = result {
                panic!("{} failed: {error}", invariant.id);
            }
        }
    }
}
