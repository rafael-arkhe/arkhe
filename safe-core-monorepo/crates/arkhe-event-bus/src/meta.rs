//! Identifiers, schema versioning and `EventMeta` — the second half of §5.2's
//! `EventBus` contract.
//!
//! §4.2 of the canonical paper (DOI 10.5281/zenodo.21383201) makes the causal
//! history a first-class artifact:
//!
//! > The payoff of routing everything through the bus is that the system's
//! > causal history is a first-class artifact: every event carries `event_id`,
//! > `trace_id`, `causation_id`, and `correlation_id` (**T-E2**); unknown future
//! > event schemas never panic a consumer (**T-E3**); and any flow can be
//! > replayed or audited from the event stream alone.
//!
//! §5.2 adds that `EventMeta` — *"with identifiers and schema version on every
//! event"* — is mandatory. [`EventMeta`] is therefore a required field of
//! [`crate::event::Event`]: an event without meta is not constructible, and an
//! event whose wire form lacks meta is rejected by
//! [`crate::event::Event::from_json`] rather than defaulted.
//!
//! ## What the four identifiers mean here
//!
//! | Field | Meaning | Cardinality |
//! |---|---|---|
//! | `event_id` | Identity of *this* event. Unique per bus instance in Phase 1. | one per event |
//! | `trace_id` | The causal trace this event belongs to. Constant along a chain. | one per trace |
//! | `causation_id` | The `event_id` of the event that caused this one. `None` marks a chain root. | one per edge |
//! | `correlation_id` | The logical operation this event participates in; may span several traces (a request, a settlement, a governance action). | one per operation |
//!
//! The paper requires every event to *carry* all four. `causation_id` is
//! modelled as `Option<EventId>` because a chain root has no cause: the field is
//! always present in the struct and on the wire, and `None` is an explicit
//! statement ("this is a root") rather than an omission. This is a modelling
//! decision, recorded here instead of hidden.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{BusError, BusResult};
use crate::topic::Topic;

/// Longest accepted identifier, in bytes.
pub const MAX_ID_LEN: usize = 128;

/// Microseconds since the Unix epoch.
///
/// [`EventMeta::timestamp_unix_us`] uses this unit. It matches
/// `arkhe-tee`'s `now_unix_us`, so timestamps from both crates are comparable.
pub const TIMESTAMP_UNIT: &str = "unix microseconds";

/// Microseconds since the Unix epoch, or `0` if the system clock is set before
/// 1970.
///
/// This is the crate's only source of wall-clock time. Nothing in the bus
/// *validates* a timestamp (Phase 1 has no clock policy); see the README,
/// "Not verified / out of scope".
pub fn now_unix_us() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

/// The identifier grammar, in one place.
///
/// Non-empty, at most [`MAX_ID_LEN`] bytes, ASCII letters/digits and `-`, `_`,
/// `.`, `:`, `/`. Whitespace of any kind is rejected, as is every non-ASCII
/// byte: an identifier reaches the wire verbatim, so "almost empty" values like
/// `"  "` must not be able to masquerade as an identity.
fn validate_id(kind: &'static str, value: &str) -> BusResult<()> {
    if value.is_empty() {
        return Err(BusError::invalid_id(kind, value, "identifier is empty"));
    }
    if value.len() > MAX_ID_LEN {
        return Err(BusError::invalid_id(
            kind,
            value,
            format!("identifier is {} bytes, limit is {MAX_ID_LEN}", value.len()),
        ));
    }
    for (offset, byte) in value.bytes().enumerate() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b':' | b'/' => {}
            _ => {
                return Err(BusError::invalid_id(
                    kind,
                    value,
                    format!(
                        "byte 0x{byte:02x} at offset {offset} is not allowed \
                         (ASCII letters, digits, `-_.:/` only)"
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Define a validated identifier newtype over `String`.
///
/// The tuple field is private and there is exactly one public constructor, so
/// an invalid identifier cannot exist — deserialization goes through the same
/// check, which is how a malformed wire value is rejected instead of accepted.
macro_rules! validated_id {
    ($name:ident, $kind:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// The label reported in [`BusError::InvalidId`].
            pub const KIND: &'static str = $kind;

            /// Validate and wrap a value.
            ///
            /// Empty, whitespace-only, over-long and non-ASCII values are
            /// rejected; see the `validate_id` function in this module.
            pub fn new(value: impl Into<String>) -> BusResult<Self> {
                let value = value.into();
                validate_id(Self::KIND, &value)?;
                Ok(Self(value))
            }

            /// The identifier as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Re-validate the stored value (second gate; see
            /// [`EventMeta::validate`]).
            pub fn is_valid(&self) -> BusResult<()> {
                validate_id(Self::KIND, &self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = BusError;

            fn try_from(value: String) -> BusResult<Self> {
                $name::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = BusError;

            fn try_from(value: &str) -> BusResult<Self> {
                $name::new(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> String {
                value.0
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                $name::new(value).map_err(<D::Error as serde::de::Error>::custom)
            }
        }
    };
}

validated_id!(
    EventId,
    "event_id",
    "Identity of a single event. §4.2 `T-E2`.\n\nThe bus enforces uniqueness per instance: publishing an `event_id` it has already\naccepted is [`BusError::DuplicateEventId`]. Global uniqueness across processes\nneeds the identity/storage crates and is out of Phase 1 scope."
);
validated_id!(
    TraceId,
    "trace_id",
    "The causal trace an event belongs to. §4.2 `T-E2`.\n\nConstant along a causal chain: [`EventMeta::caused_by`] inherits it."
);
validated_id!(
    CorrelationId,
    "correlation_id",
    "The logical operation an event participates in. §4.2 `T-E2`.\n\nWider than a trace: one correlated operation (a request, a settlement, a\ngovernance action) may contain several traces."
);

/// Mints identifiers that satisfy the identifier grammar (`validate_id`).
///
/// # Not a cryptographic identity provider
///
/// The generator combines a per-process salt (process id + first-use timestamp)
/// with a group-separated BLAKE3 hash over a monotonic counter. Two calls in the
/// same process never collide, and two processes on one host collide only if
/// they share a process id *and* start in the same microsecond. That is enough
/// to keep a per-bus uniqueness check meaningful; it is **not** an unforgeable
/// identity and **not** a substitute for the `arkhe-identity` crate the Phase 1
/// roadmap pairs with this bus. If you need unforgeable identifiers, mint them
/// from a key you hold and pass them in — this type is a convenience, not a
/// trust boundary.
#[derive(Debug)]
pub struct IdGenerator {
    salt: [u8; 32],
    counter: AtomicU64,
}

impl IdGenerator {
    /// A generator with a salt derived from the process id and the current time.
    pub fn new() -> Self {
        Self::with_salt(process_salt())
    }

    /// A generator with an explicit salt. Deterministic, and therefore testable.
    pub fn with_salt(salt: [u8; 32]) -> Self {
        Self {
            salt,
            counter: AtomicU64::new(0),
        }
    }

    /// Mint an [`EventId`].
    pub fn next_event_id(&self, timestamp_unix_us: u64) -> BusResult<EventId> {
        EventId::new(self.next("evt", timestamp_unix_us))
    }

    /// Mint a [`TraceId`].
    pub fn next_trace_id(&self, timestamp_unix_us: u64) -> BusResult<TraceId> {
        TraceId::new(self.next("trc", timestamp_unix_us))
    }

    /// Mint a [`CorrelationId`].
    pub fn next_correlation_id(&self, timestamp_unix_us: u64) -> BusResult<CorrelationId> {
        CorrelationId::new(self.next("cor", timestamp_unix_us))
    }

    /// The shared derivation. Output is `"{prefix}_{32 hex chars}"`, which the
    /// identifier grammar accepts; the typed constructors above still re-check
    /// it, so a future change to this function cannot silently produce an
    /// invalid identifier.
    fn next(&self, prefix: &str, timestamp_unix_us: u64) -> String {
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);

        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ARKHE-EVENT-BUS/v1/id-generator");
        hasher.update(&self.salt);
        hasher.update(prefix.as_bytes());
        hasher.update(&timestamp_unix_us.to_le_bytes());
        hasher.update(&counter.to_le_bytes());

        let digest = hasher.finalize().to_hex();
        let short: String = digest.as_str().chars().take(32).collect();
        format!("{prefix}_{short}")
    }
}

impl Default for IdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-process salt for [`IdGenerator::new`].
fn process_salt() -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"ARKHE-EVENT-BUS/v1/id-generator/salt");
    hasher.update(&std::process::id().to_le_bytes());
    hasher.update(&now_unix_us().to_le_bytes());
    *hasher.finalize().as_bytes()
}

/// The schema version of an event, as §5.2 requires in every `EventMeta`.
///
/// # Compatibility rule (`T-E3`)
///
/// Older consumers reading newer events is the case §4.2 names: *"unknown future
/// event schemas never panic a consumer"*. This crate's policy is the usual one,
/// stated so it can be argued with:
///
/// * `event.major <= consumer.major` ⇒ readable. A higher *minor* within a known
///   major is additive and accepted ([`SchemaVersion::is_readable_by`]).
/// * `event.major > consumer.major` ⇒ **not** readable, and the consumer gets
///   [`BusError::UnsupportedSchemaVersion`] from
///   [`crate::bus::Subscription::recv_understanding`] — a typed error, never a
///   panic and never a silent coercion.
///
/// `major == 0` does not exist: §5.2 makes a schema version mandatory on every
/// event, so `0.x` means "no schema declared" and is rejected at construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SchemaVersion {
    major: u32,
    minor: u32,
}

impl SchemaVersion {
    /// The version this crate publishes and understands: `1.0`.
    pub const V1: Self = Self { major: 1, minor: 0 };

    /// Lowest legal major version. `0` is reserved for "no schema declared".
    pub const MIN_MAJOR: u32 = 1;

    /// Validate and construct.
    pub fn new(major: u32, minor: u32) -> BusResult<Self> {
        let version = Self { major, minor };
        version.validate()?;
        Ok(version)
    }

    /// Major component. A change here is breaking.
    pub fn major(&self) -> u32 {
        self.major
    }

    /// Minor component. A change here is additive.
    pub fn minor(&self) -> u32 {
        self.minor
    }

    /// Re-check the raw components.
    pub fn validate(&self) -> BusResult<()> {
        if self.major < Self::MIN_MAJOR {
            return Err(BusError::InvalidSchemaVersion {
                major: self.major,
                minor: self.minor,
                reason: format!(
                    "major {} is below the minimum {} (0.x means \"no schema declared\", \
                     which §5.2 forbids)",
                    self.major,
                    Self::MIN_MAJOR
                ),
            });
        }
        Ok(())
    }

    /// True when a consumer at `consumer` can read an event at `self`.
    pub fn is_readable_by(&self, consumer: Self) -> bool {
        self.major <= consumer.major
    }

    /// True when the two versions share a major component.
    pub fn is_same_major(&self, other: Self) -> bool {
        self.major == other.major
    }
}

impl Default for SchemaVersion {
    /// `1.0`. Hand-written rather than derived: a derived default would be
    /// `0.0`, which this type exists to reject.
    fn default() -> Self {
        Self::V1
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl serde::Serialize for SchemaVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("SchemaVersion", 2)?;
        state.serialize_field("major", &self.major)?;
        state.serialize_field("minor", &self.minor)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for SchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Wire {
            major: u32,
            minor: u32,
        }

        let wire = Wire::deserialize(deserializer)?;
        SchemaVersion::new(wire.major, wire.minor).map_err(<D::Error as serde::de::Error>::custom)
    }
}

/// The mandatory per-event metadata of §5.2.
///
/// Every field holds a validated type, so `EventMeta` cannot hold an empty
/// identifier, an unknown-grammar topic or a `0.x` schema version: the
/// validation happens in the constructors, including on the deserialization
/// path. [`EventMeta::validate`] re-checks the stored values and is the second
/// gate applied by [`crate::bus::EventBus::publish`].
///
/// # Causal chains
///
/// [`EventMeta::root`] starts a chain, [`EventMeta::caused_by`] extends one:
///
/// ```
/// use arkhe_event_bus::meta::{CorrelationId, EventId, EventMeta, SchemaVersion, TraceId};
/// use arkhe_event_bus::topic::ATTESTATION_COMPLETED;
///
/// let trace = TraceId::new("trc_1").expect("valid");
/// let correlation = CorrelationId::new("cor_1").expect("valid");
///
/// let a = EventMeta::root(
///     ATTESTATION_COMPLETED,
///     SchemaVersion::V1,
///     EventId::new("evt_a").expect("valid"),
///     trace.clone(),
///     correlation.clone(),
///     1,
/// );
/// assert!(a.causation_id.is_none(), "a root has no cause");
///
/// let b = a.caused_by(EventId::new("evt_b").expect("valid"), 2);
/// assert_eq!(b.causation_id.as_ref().map(EventId::as_str), Some("evt_a"));
/// assert_eq!(b.trace_id, trace, "the trace is inherited");
/// assert_eq!(b.correlation_id, correlation, "the correlation is inherited");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EventMeta {
    /// Identity of this event. §4.2 `T-E2`.
    pub event_id: EventId,
    /// The causal trace this event belongs to. §4.2 `T-E2`.
    pub trace_id: TraceId,
    /// The `event_id` of the event that caused this one, or `None` for a chain
    /// root. §4.2 `T-E2`.
    pub causation_id: Option<EventId>,
    /// The logical operation this event participates in. §4.2 `T-E2`.
    pub correlation_id: CorrelationId,
    /// Schema version of the payload. §5.2 (mandatory).
    pub schema_version: SchemaVersion,
    /// Topic this event is published on. §5.2 "typed topics".
    pub topic: Topic,
    /// Wall-clock time, in [`TIMESTAMP_UNIT`].
    pub timestamp_unix_us: u64,
}

impl EventMeta {
    /// Meta for the root of a causal chain (`causation_id == None`).
    pub fn root(
        topic: Topic,
        schema_version: SchemaVersion,
        event_id: EventId,
        trace_id: TraceId,
        correlation_id: CorrelationId,
        timestamp_unix_us: u64,
    ) -> Self {
        Self {
            event_id,
            trace_id,
            causation_id: None,
            correlation_id,
            schema_version,
            topic,
            timestamp_unix_us,
        }
    }

    /// Meta for an event caused by `self`.
    ///
    /// Inherits `trace_id`, `correlation_id`, `schema_version` and `topic`, and
    /// points `causation_id` at this event. Callers that need a different topic
    /// or schema version set those fields afterwards — the struct has public
    /// fields and every one of them is a validated type.
    pub fn caused_by(&self, event_id: EventId, timestamp_unix_us: u64) -> Self {
        Self {
            event_id,
            trace_id: self.trace_id.clone(),
            causation_id: Some(self.event_id.clone()),
            correlation_id: self.correlation_id.clone(),
            schema_version: self.schema_version,
            topic: self.topic.clone(),
            timestamp_unix_us,
        }
    }

    /// Re-validate every stored value.
    ///
    /// This is a real re-check of the raw characters and numbers, not a no-op;
    /// it is what makes "an event built by any path is validated before it
    /// enters the log" true rather than assumed. In the current crate the
    /// constructors make a violation unreachable — which is exactly why the
    /// tests prove the *constructor* gates reject, and why this gate exists
    /// anyway (see the README, "Defence in depth").
    pub fn validate(&self) -> BusResult<()> {
        self.event_id.is_valid()?;
        self.trace_id.is_valid()?;
        if let Some(causation_id) = &self.causation_id {
            causation_id.is_valid()?;
        }
        self.correlation_id.is_valid()?;
        self.schema_version.validate()?;
        self.topic.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topic::STORAGE_STORED;

    fn meta(event_id: &str) -> EventMeta {
        EventMeta::root(
            STORAGE_STORED,
            SchemaVersion::V1,
            EventId::new(event_id).expect("valid event id"),
            TraceId::new("trc_test").expect("valid trace id"),
            CorrelationId::new("cor_test").expect("valid correlation id"),
            1_700_000_000_000_000,
        )
    }

    #[test]
    fn identifiers_reject_empty_whitespace_and_foreign_bytes() {
        for bad in ["", " ", "  ", "\t", "a b", "café", "id!", "id\n"] {
            match EventId::new(bad) {
                Err(BusError::InvalidId { kind, value, .. }) => {
                    assert_eq!(kind, "event_id");
                    assert_eq!(value, bad);
                }
                other => panic!("expected InvalidId for {bad:?}, got {other:?}"),
            }
            assert!(TraceId::new(bad).is_err());
            assert!(CorrelationId::new(bad).is_err());
        }
    }

    #[test]
    fn identifiers_accept_the_grammar_and_enforce_the_length_cap() {
        for good in [
            "evt_01H",
            "did:arkhe:agent-7",
            "550e8400-e29b-41d4-a716-446655440000",
            "a/b.c_d-e",
            "0f9a2c",
        ] {
            assert!(EventId::new(good).is_ok(), "{good} should be accepted");
        }

        assert!(EventId::new("e".repeat(MAX_ID_LEN)).is_ok());
        assert!(EventId::new("e".repeat(MAX_ID_LEN + 1)).is_err());
    }

    #[test]
    fn identifier_kinds_are_distinct_labels() {
        assert_eq!(EventId::KIND, "event_id");
        assert_eq!(TraceId::KIND, "trace_id");
        assert_eq!(CorrelationId::KIND, "correlation_id");

        match TraceId::new("") {
            Err(BusError::InvalidId { kind, .. }) => assert_eq!(kind, "trace_id"),
            other => panic!("expected InvalidId, got {other:?}"),
        }
    }

    #[test]
    fn identifiers_round_trip_through_json_and_reject_bad_wire_values() {
        let id = EventId::new("evt_round").expect("valid");
        let json = serde_json::to_string(&id).expect("serializes");
        assert_eq!(json, "\"evt_round\"");
        assert_eq!(
            serde_json::from_str::<EventId>(&json).expect("deserializes"),
            id
        );
        assert!(serde_json::from_str::<EventId>("\"\"").is_err());
        assert!(serde_json::from_str::<EventId>("42").is_err());
    }

    #[test]
    fn id_generator_is_monotonic_and_grammar_valid() {
        let generator = IdGenerator::with_salt([7u8; 32]);
        let mut seen = std::collections::HashSet::new();
        for n in 0..64u64 {
            let id = generator.next_event_id(1000 + n).expect("generated id is valid");
            assert!(id.as_str().starts_with("evt_"), "{id}");
            assert_eq!(id.as_str().len(), 4 + 32);
            assert!(seen.insert(id.to_string()), "generated ids must not repeat");
        }

        // Same salt, same inputs, same sequence ⇒ same ids (deterministic salt
        // makes the generator testable).
        let a = IdGenerator::with_salt([1u8; 32]);
        let b = IdGenerator::with_salt([1u8; 32]);
        assert_eq!(
            a.next_trace_id(5).expect("valid"),
            b.next_trace_id(5).expect("valid")
        );

        // Different salts ⇒ different ids.
        let c = IdGenerator::with_salt([2u8; 32]);
        assert_ne!(
            a.next_correlation_id(5).expect("valid"),
            c.next_correlation_id(5).expect("valid")
        );

        // The default constructor is usable and produces valid ids.
        assert!(IdGenerator::new().next_event_id(0).is_ok());
    }

    #[test]
    fn schema_version_rejects_zero_major_and_defaults_to_v1() {
        match SchemaVersion::new(0, 0) {
            Err(BusError::InvalidSchemaVersion { major, minor, reason }) => {
                assert_eq!((major, minor), (0, 0));
                assert!(reason.contains("no schema declared"), "{reason}");
            }
            other => panic!("expected InvalidSchemaVersion, got {other:?}"),
        }
        assert!(SchemaVersion::new(0, 7).is_err());
        assert!(SchemaVersion::new(1, 0).is_ok());
        assert_eq!(SchemaVersion::default(), SchemaVersion::V1);
        assert_eq!(SchemaVersion::V1.to_string(), "1.0");
    }

    #[test]
    fn schema_readability_is_by_major() {
        let v1_0 = SchemaVersion::new(1, 0).expect("valid");
        let v1_7 = SchemaVersion::new(1, 7).expect("valid");
        let v2_0 = SchemaVersion::new(2, 0).expect("valid");

        assert!(v1_0.is_readable_by(v1_0));
        assert!(v1_7.is_readable_by(v1_0), "higher minor is additive");
        assert!(!v2_0.is_readable_by(v1_0), "higher major is not readable");
        assert!(v1_0.is_readable_by(v2_0));
        assert!(v1_0.is_same_major(v1_7));
        assert!(!v1_0.is_same_major(v2_0));
    }

    #[test]
    fn schema_version_round_trips_through_json() {
        let v = SchemaVersion::new(2, 3).expect("valid");
        let json = serde_json::to_string(&v).expect("serializes");
        assert_eq!(json, r#"{"major":2,"minor":3}"#);
        assert_eq!(
            serde_json::from_str::<SchemaVersion>(&json).expect("deserializes"),
            v
        );
        assert!(serde_json::from_str::<SchemaVersion>(r#"{"major":0,"minor":1}"#).is_err());
        assert!(serde_json::from_str::<SchemaVersion>(r#""1.0""#).is_err());
    }

    #[test]
    fn root_and_caused_by_build_a_consistent_chain() {
        let a = meta("evt_a");
        assert!(a.causation_id.is_none());
        assert_eq!(a.schema_version, SchemaVersion::V1);
        assert_eq!(a.topic, STORAGE_STORED);

        let b = a.caused_by(EventId::new("evt_b").expect("valid"), 2);
        assert_eq!(b.causation_id.as_ref().map(EventId::as_str), Some("evt_a"));
        assert_eq!(b.trace_id, a.trace_id);
        assert_eq!(b.correlation_id, a.correlation_id);
        assert_eq!(b.topic, a.topic);
        assert_ne!(b.event_id, a.event_id);
    }

    #[test]
    fn meta_validate_accepts_well_formed_values() {
        assert!(meta("evt_ok").validate().is_ok());
    }

    #[test]
    fn meta_round_trips_through_json_with_all_four_identifiers() {
        let m = meta("evt_json");
        let json = serde_json::to_string(&m).expect("serializes");
        let back: EventMeta = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back, m);
        assert!(json.contains("\"causation_id\":null"));
        assert!(json.contains("\"event_id\":\"evt_json\""));
        assert!(json.contains("\"trace_id\":\"trc_test\""));
        assert!(json.contains("\"correlation_id\":\"cor_test\""));
    }

    #[test]
    fn meta_deserialization_rejects_a_bad_wire_field() {
        let mut value: serde_json::Value =
            serde_json::to_value(meta("evt_wire")).expect("to_value");
        value["event_id"] = serde_json::Value::String(String::new());
        assert!(
            serde_json::from_value::<EventMeta>(value).is_err(),
            "an empty event_id must not deserialize into EventMeta"
        );
    }

    #[test]
    fn now_unix_us_looks_like_a_current_timestamp() {
        // 2020-01-01T00:00:00Z, in microseconds.
        assert!(now_unix_us() > 1_577_836_800_000_000);
        assert_eq!(TIMESTAMP_UNIT, "unix microseconds");
    }
}
