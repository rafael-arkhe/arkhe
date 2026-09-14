//! Error type for `arkhe-event-bus`.
//!
//! Every fallible operation in this crate fails **closed**. There is no path in
//! which a malformed event, an unverifiable signature, a diverging replay or an
//! out-of-range sequence yields a successful `publish`, and no path in which a
//! consumer mistake (an unknown topic, a newer schema) panics the process.
//!
//! §4.2 of the canonical paper states the reason plainly: *"no claim in the
//! architecture is supposed to rest on a layer weaker than the claim
//! requires"*, and Table 2's invariants are annotated *"violation is never
//! silent"*. Rejections are therefore typed values, not log lines and not
//! panics.

use thiserror::Error;

/// Result alias used throughout the crate.
pub type BusResult<T> = Result<T, BusError>;

/// Everything the bus can refuse or fail to do.
///
/// The variants split into two families, see [`BusError::is_rejection`]:
/// *event refusals* (the event was never appended and never delivered) and
/// *transport/state failures* (delivery lag, a closed subscription, a log that
/// does not match a checkpoint).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BusError {
    /// An `EventMeta` field that §5.2 makes mandatory was absent from the wire
    /// form of an event. `field` is `"meta"` when the whole member was missing.
    #[error("event meta is missing required field `{field}`")]
    MissingMeta {
        /// Name of the absent field.
        field: &'static str,
    },

    /// The event envelope had no `payload` member. §5.2's `EventMeta` is
    /// mandatory, and an event without a payload is not a well-formed event.
    #[error("event envelope has no `payload` member")]
    MissingPayload,

    /// The envelope was structurally malformed: not UTF-8, not JSON, not a JSON
    /// object, or a member of the wrong JSON type.
    #[error("malformed event envelope: {field} — {reason}")]
    Malformed {
        /// Member that failed structural validation.
        field: &'static str,
        /// Why it failed.
        reason: String,
    },

    /// An identifier ([`crate::meta::EventId`], [`crate::meta::TraceId`],
    /// [`crate::meta::CorrelationId`]) was empty, over-long, or used characters
    /// outside the accepted set.
    #[error("invalid {kind} {value:?}: {reason}")]
    InvalidId {
        /// Which identifier type rejected the value (`"event_id"`, ...).
        kind: &'static str,
        /// The offending value, verbatim.
        value: String,
        /// Why it was rejected.
        reason: String,
    },

    /// A topic did not match the topic grammar (see [`crate::topic::Topic`]).
    #[error("invalid topic {topic:?}: {reason}")]
    InvalidTopic {
        /// The offending value, verbatim.
        topic: String,
        /// Why it was rejected.
        reason: String,
    },

    /// A schema version was structurally invalid — `major == 0` means "no
    /// schema declared", which §5.2 forbids, since `EventMeta` must carry a
    /// schema version on every event.
    #[error("invalid schema version {major}.{minor}: {reason}")]
    InvalidSchemaVersion {
        /// Major component as received.
        major: u32,
        /// Minor component as received.
        minor: u32,
        /// Why it was rejected.
        reason: String,
    },

    /// The event's *major* schema version is newer than the one the consumer
    /// understands. This is invariant `T-E3`'s typed outcome: an unknown future
    /// schema is reported, never panicked on, and never silently coerced.
    ///
    /// `sequence` locates the refused event in the log, so nothing is lost: a
    /// consumer that meets a schema it cannot read can replay that sequence and
    /// inspect the raw JSON itself. See
    /// [`crate::bus::Subscription::recv_understanding`].
    #[error(
        "event at sequence {sequence} has schema major {event_major}, newer than the \
         consumer's major {consumer_major}"
    )]
    UnsupportedSchemaVersion {
        /// Log position of the refused event.
        sequence: u64,
        /// Major version carried by the event.
        event_major: u32,
        /// Major version the consumer declared it understands.
        consumer_major: u32,
    },

    /// The same `event_id` was already appended to this bus. `EventId`
    /// uniqueness is enforced per bus instance in Phase 1 (in-memory); see the
    /// crate README, "Not verified / out of scope".
    #[error("event_id {event_id} was already published on this bus")]
    DuplicateEventId {
        /// The repeated identifier.
        event_id: String,
    },

    /// [`crate::signature::SignaturePolicy::Required`] is configured but no
    /// [`crate::signature::SignatureVerifier`] was supplied, so no event can be
    /// validated. This state is deliberately *representable* (so that it can be
    /// constructed and reported) and deliberately *unusable* (no event is ever
    /// accepted in it).
    #[error("the bus requires signed events but no SignatureVerifier is configured")]
    SignatureVerifierUnavailable,

    /// The bus requires signatures and the event carried none.
    #[error("the bus requires signed events but event {event_id} carries no signature")]
    MissingSignature {
        /// Identifier of the unsigned event.
        event_id: String,
    },

    /// The configured verifier rejected the signature. The bus carries no key
    /// material and no crypto backend: rejection means the plugged-in verifier
    /// returned `false` (see [`crate::signature`]).
    #[error("the signature on event {event_id} was rejected by the configured verifier")]
    InvalidSignature {
        /// Identifier of the event whose signature failed.
        event_id: String,
    },

    /// A sequence number was outside `0..=head + 1` for this log. Also returned
    /// by [`crate::log::EventLog::root_at`] when asked for a root beyond the
    /// head.
    #[error("sequence {from_seq} is out of range (log head is {head})")]
    ReplayOutOfRange {
        /// The requested sequence number.
        from_seq: u64,
        /// The log's current head (`0` for an empty log).
        head: u64,
    },

    /// The requested sequence is at or below an established checkpoint fence:
    /// already-fenced history must never be accepted as new. This is the
    /// `T-13` anti-replay gate.
    #[error("sequence {seq} is at or below checkpoint fence {fence}: refusing to accept it as new")]
    FencedSequence {
        /// The sequence number that was refused.
        seq: u64,
        /// Sequence up to and including which the checkpoint fences history.
        fence: u64,
    },

    /// A log presented as authoritative does not reproduce the Merkle root the
    /// checkpoint recorded for its prefix: the fenced history was rewritten,
    /// reordered, or replaced.
    #[error(
        "log diverges from checkpoint at sequence {checkpoint_seq}: \
         checkpoint root {expected}, observed root {observed}"
    )]
    CheckpointMismatch {
        /// Sequence the checkpoint covers.
        checkpoint_seq: u64,
        /// Root recorded by the checkpoint, hex-encoded.
        expected: String,
        /// Root computed from the presented log, hex-encoded.
        observed: String,
    },

    /// An internal log entry carried the wrong sequence number. Sequence numbers
    /// are assigned by the log itself, so this indicates memory corruption or a
    /// bug — it is surfaced rather than assumed impossible.
    #[error("log sequence discontinuity at {seq}: expected {expected}")]
    LogDiscontinuity {
        /// Sequence found.
        seq: u64,
        /// Sequence that was expected at that position.
        expected: u64,
    },

    /// An entry's cached digest disagreed with the digest recomputed from the
    /// event bytes.
    #[error("log integrity failure at sequence {seq}: {reason}")]
    LogIntegrity {
        /// Sequence of the offending entry.
        seq: u64,
        /// What disagreed.
        reason: String,
    },

    /// The subscriber was too slow and the bounded channel dropped events for
    /// it. Reported explicitly — never silently. The subscription remains
    /// usable; re-synchronise through [`crate::log::EventLog::replay`].
    #[error("subscriber lagged and missed {skipped} event(s)")]
    SubscriberLagged {
        /// Number of events missed.
        skipped: u64,
    },

    /// The bus (and therefore the channel's sender) was dropped; no further
    /// events will arrive on this subscription.
    #[error("the bus was dropped; no further events will arrive on this subscription")]
    SubscriptionClosed,

    /// An internal invariant of this crate was violated.
    #[error("internal error: {0}")]
    Internal(String),
}

impl BusError {
    /// Construct [`BusError::InvalidId`].
    pub fn invalid_id(
        kind: &'static str,
        value: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        BusError::InvalidId {
            kind,
            value: value.into(),
            reason: reason.into(),
        }
    }

    /// Construct [`BusError::InvalidTopic`].
    pub fn invalid_topic(topic: impl Into<String>, reason: impl Into<String>) -> Self {
        BusError::InvalidTopic {
            topic: topic.into(),
            reason: reason.into(),
        }
    }

    /// Construct [`BusError::Malformed`].
    pub fn malformed(field: &'static str, reason: impl Into<String>) -> Self {
        BusError::Malformed {
            field,
            reason: reason.into(),
        }
    }

    /// Construct [`BusError::Internal`].
    pub fn internal(reason: impl Into<String>) -> Self {
        BusError::Internal(reason.into())
    }

    /// True when this error means *"an event was refused"* — the event was not
    /// appended to the log and was not delivered to any subscriber.
    ///
    /// ```
    /// use arkhe_event_bus::error::BusError;
    ///
    /// assert!(BusError::SignatureVerifierUnavailable.is_rejection());
    /// assert!(!BusError::SubscriptionClosed.is_rejection());
    /// ```
    pub fn is_rejection(&self) -> bool {
        matches!(
            self,
            BusError::MissingMeta { .. }
                | BusError::MissingPayload
                | BusError::Malformed { .. }
                | BusError::InvalidId { .. }
                | BusError::InvalidTopic { .. }
                | BusError::InvalidSchemaVersion { .. }
                | BusError::DuplicateEventId { .. }
                | BusError::SignatureVerifierUnavailable
                | BusError::MissingSignature { .. }
                | BusError::InvalidSignature { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_displayable_and_thread_safe() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<BusError>();

        let e = BusError::FencedSequence { seq: 3, fence: 5 };
        assert_eq!(
            e.to_string(),
            "sequence 3 is at or below checkpoint fence 5: refusing to accept it as new"
        );
    }

    #[test]
    fn rejection_classification_covers_the_fail_closed_gates() {
        // Every publish-time gate that refuses an event is a rejection.
        assert!(BusError::MissingMeta { field: "meta" }.is_rejection());
        assert!(BusError::MissingPayload.is_rejection());
        assert!(BusError::malformed("payload", "not an object").is_rejection());
        assert!(BusError::invalid_id("event_id", "", "identifier is empty").is_rejection());
        assert!(BusError::invalid_topic("", "topic is empty").is_rejection());
        assert!(BusError::InvalidSchemaVersion {
            major: 0,
            minor: 0,
            reason: "x".into()
        }
        .is_rejection());
        assert!(BusError::DuplicateEventId {
            event_id: "e".into()
        }
        .is_rejection());
        assert!(BusError::MissingSignature {
            event_id: "e".into()
        }
        .is_rejection());
        assert!(BusError::InvalidSignature {
            event_id: "e".into()
        }
        .is_rejection());
        assert!(BusError::SignatureVerifierUnavailable.is_rejection());

        // State/transport failures are not event refusals.
        assert!(!BusError::SubscriberLagged { skipped: 1 }.is_rejection());
        assert!(!BusError::SubscriptionClosed.is_rejection());
        assert!(!BusError::ReplayOutOfRange {
            from_seq: 9,
            head: 1
        }
        .is_rejection());
        assert!(!BusError::FencedSequence { seq: 1, fence: 2 }.is_rejection());
        assert!(!BusError::UnsupportedSchemaVersion {
            sequence: 3,
            event_major: 2,
            consumer_major: 1
        }
        .is_rejection());
        assert!(!BusError::internal("boom").is_rejection());
    }

    #[test]
    fn constructor_helpers_keep_their_payloads() {
        match BusError::invalid_id("event_id", "", "identifier is empty") {
            BusError::InvalidId { kind, value, reason } => {
                assert_eq!(kind, "event_id");
                assert_eq!(value, "");
                assert_eq!(reason, "identifier is empty");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
        match BusError::malformed("meta", "expected object") {
            BusError::Malformed { field, reason } => {
                assert_eq!(field, "meta");
                assert_eq!(reason, "expected object");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
        assert_eq!(
            BusError::internal("x"),
            BusError::Internal(String::from("x"))
        );
    }
}
