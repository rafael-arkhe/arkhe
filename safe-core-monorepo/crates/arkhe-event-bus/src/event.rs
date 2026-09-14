//! The event message: mandatory [`EventMeta`], a JSON payload, optional
//! signature, and the canonical bytes that tie them together.
//!
//! # What the signature covers
//!
//! [`Event::signing_bytes`] is
//!
//! ```text
//! SIGNING_DOMAIN (27 bytes, ASCII)
//! ‖ u64 little-endian byte length of the canonical meta JSON
//! ‖ canonical meta JSON
//! ‖ canonical payload JSON
//! ```
//!
//! Canonical JSON here means: `serde_json::to_value`, followed by an explicit
//! recursive rebuild that puts every object's keys in alphabetical order,
//! followed by compact `serde_json::to_vec`. The ordering is made explicit
//! because it must not depend on `serde_json`'s `Map` implementation: a
//! `BTreeMap` by default, but an insertion-ordered `IndexMap` as soon as any
//! crate in the build enables `serde_json/preserve_order` — which feature
//! unification then applies to this crate too, silently leaking key insertion
//! order into the signing bytes. Three consequences, all deliberate:
//!
//! * **Struct field order does not matter.** Meta is canonicalised through a
//!   `Value`, so a future reordering of `EventMeta`'s fields cannot silently
//!   invalidate existing signatures.
//! * **Key insertion order does not matter, at any depth.** Objects nested
//!   inside objects are sorted recursively; arrays keep their element order
//!   (JSON arrays are ordered — sorting them would erase information). Two
//!   payloads that differ only in the order their objects were built sign
//!   identically.
//! * **A published event and its JSON round trip sign identically.**
//!   [`Event::to_json`] → [`Event::from_json`] preserves
//!   [`Event::signing_bytes`] byte-for-byte, which the tests assert by comparing
//!   digests. This is what makes offline verification of a stored event
//!   meaningful.
//!
//! The hash [`Event::digest`] is `BLAKE3(signing_bytes)` and is the Merkle leaf
//! for this event in [`crate::log`].
//!
//! # Formats this crate does *not* accept
//!
//! There is no constructor that takes pre-serialised bytes as the signing
//! message. A publisher that signs raw bytes it produced outside this crate is
//! out of contract: the bus canonicalises, so the signature must cover the
//! canonical form. Documented here rather than discovered in production.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Map, Value};

use crate::error::{BusError, BusResult};
use crate::meta::{CorrelationId, EventId, EventMeta, SchemaVersion, TraceId};
use crate::signature::EventSignature;
use crate::topic::Topic;

/// Domain separator prefixed to every signing message.
///
/// Prevents a signature collected for this bus from being replayed onto another
/// protocol, and vice versa.
pub const SIGNING_DOMAIN: &[u8] = b"ARKHE-EVENT-BUS/v1/event";

/// Domain separator for the Merkle leaf hash of an event digest.
///
/// Leaf and internal node hashes are domain-separated inside
/// [`crate::log::merkle_root`] so that a leaf can never be mistaken for an
/// internal node (the standard defence against second-preimage games in
/// Merkle trees).
pub const LEAF_DOMAIN: u8 = 0x00;

/// Domain separator for the Merkle internal node hash.
pub const INTERNAL_DOMAIN: u8 = 0x01;

/// An event: the unit the bus publishes, logs and delivers.
///
/// Fields are private and every constructor validates, so an `Event` that
/// exists is an event that passed §5.2's gates. `Debug` is implemented
/// manually to print the identifiers and the digest rather than a payload that
/// may be large or sensitive.
#[derive(Clone, PartialEq)]
pub struct Event {
    meta: EventMeta,
    payload: Value,
    canonical_payload: Arc<[u8]>,
    signing_bytes: Arc<[u8]>,
    digest: [u8; 32],
    signature: Option<EventSignature>,
}

impl Event {
    /// Wrap meta and a payload, computing the canonical bytes and the digest.
    ///
    /// Fails closed: the meta is re-validated ([`EventMeta::validate`]) and a
    /// payload that cannot be canonicalised to JSON is refused
    /// ([`BusError::Malformed`] with `field == "payload"`).
    pub fn new(meta: EventMeta, payload: Value) -> BusResult<Self> {
        meta.validate()?;
        let canonical_meta = canonical_json_bytes("meta", &meta)?;
        let canonical_payload = canonical_json_bytes("payload", &payload)?;
        let signing_bytes = assemble_signing_bytes(&canonical_meta, &canonical_payload);
        let digest = *blake3::hash(&signing_bytes).as_bytes();

        Ok(Self {
            meta,
            payload,
            canonical_payload: canonical_payload.into(),
            signing_bytes: signing_bytes.into(),
            digest,
            signature: None,
        })
    }

    /// Attach a signature. The bytes signed must be [`Event::signing_bytes`],
    /// which do not change when a signature is attached.
    pub fn with_signature(mut self, signature: EventSignature) -> Self {
        self.signature = Some(signature);
        self
    }

    /// Parse an event from its JSON envelope:
    /// `{"meta": {...}, "payload": ..., "signature": "<hex>" | null}`.
    ///
    /// # Failure modes, by design
    ///
    /// | Input | Outcome |
    /// |---|---|
    /// | not UTF-8, not JSON, not an object | [`BusError::Malformed`] |
    /// | `meta` absent | [`BusError::MissingMeta`] with `field == "meta"` |
    /// | any §5.2 meta member absent | [`BusError::MissingMeta`] naming it |
    /// | meta member of the wrong JSON type | [`BusError::Malformed`] naming it |
    /// | empty `event_id`, empty `trace_id`, … | [`BusError::InvalidId`] |
    /// | `schema_version.major == 0` | [`BusError::InvalidSchemaVersion`] |
    /// | unparseable topic | [`BusError::InvalidTopic`] |
    /// | `payload` absent | [`BusError::MissingPayload`] |
    /// | `signature` not hex / not a string / not null | [`BusError::Malformed`] |
    ///
    /// **Unknown members are ignored** — both top-level and inside `meta`, and
    /// unknown payload fields are preserved verbatim. That is `T-E3`: a future
    /// schema that adds fields must not make today's consumer fail, let alone
    /// panic. Unknown *major* schema versions are handled by the consumer side,
    /// see [`crate::bus::Subscription::recv_understanding`].
    pub fn from_json(bytes: &[u8]) -> BusResult<Self> {
        let text = std::str::from_utf8(bytes)
            .map_err(|e| BusError::malformed("envelope", format!("not UTF-8: {e}")))?;
        let value: Value = serde_json::from_str(text)
            .map_err(|e| BusError::malformed("envelope", format!("not JSON: {e}")))?;
        let object = match value.as_object() {
            Some(object) => object,
            None => return Err(BusError::malformed("envelope", "expected a JSON object")),
        };

        let meta = match object.get("meta") {
            Some(value) => match value.as_object() {
                Some(meta_object) => parse_meta(meta_object)?,
                None => return Err(BusError::malformed("meta", "expected a JSON object")),
            },
            None => return Err(BusError::MissingMeta { field: "meta" }),
        };

        let payload = match object.get("payload") {
            Some(value) => value.clone(),
            None => return Err(BusError::MissingPayload),
        };

        let signature = parse_signature(object.get("signature"))?;

        let event = Event::new(meta, payload)?;
        Ok(match signature {
            Some(signature) => event.with_signature(signature),
            None => event,
        })
    }

    /// Render the JSON envelope.
    ///
    /// `meta` is rendered through the same canonicalisation used for signing, so
    /// `from_json(&to_json(&e))` preserves `e.signing_bytes()` and
    /// `e.digest()`. Unknown members are never invented; `signature` is always
    /// present, as `null` when unsigned.
    pub fn to_json(&self) -> BusResult<String> {
        let meta = serde_json::to_value(&self.meta)
            .map_err(|e| BusError::internal(format!("meta is not serialisable: {e}")))?;

        let mut object = Map::new();
        object.insert(String::from("meta"), meta);
        object.insert(String::from("payload"), self.payload.clone());
        object.insert(
            String::from("signature"),
            match &self.signature {
                Some(signature) => Value::String(signature.to_hex()),
                None => Value::Null,
            },
        );

        serde_json::to_string(&Value::Object(object))
            .map_err(|e| BusError::internal(format!("envelope is not serialisable: {e}")))
    }

    /// §5.2's mandatory metadata.
    pub fn meta(&self) -> &EventMeta {
        &self.meta
    }

    /// The payload, exactly as published.
    pub fn payload(&self) -> &Value {
        &self.payload
    }

    /// The payload's canonical JSON bytes (compact, keys sorted).
    pub fn canonical_payload(&self) -> &[u8] {
        &self.canonical_payload[..]
    }

    /// The bytes a [`crate::signature::SignatureVerifier`] must be given.
    pub fn signing_bytes(&self) -> &[u8] {
        &self.signing_bytes[..]
    }

    /// `BLAKE3(signing_bytes)` — this event's Merkle leaf.
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// The attached signature, if any.
    pub fn signature(&self) -> Option<&EventSignature> {
        self.signature.as_ref()
    }

    /// Whether a signature is attached. Not a statement about validity — only
    /// the bus's configured verifier can answer that.
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    /// Identity of this event (§4.2 `T-E2`).
    pub fn event_id(&self) -> &EventId {
        &self.meta.event_id
    }

    /// The causal trace (§4.2 `T-E2`).
    pub fn trace_id(&self) -> &TraceId {
        &self.meta.trace_id
    }

    /// The `event_id` of the causing event, or `None` for a chain root.
    pub fn causation_id(&self) -> Option<&EventId> {
        self.meta.causation_id.as_ref()
    }

    /// The logical operation (§4.2 `T-E2`).
    pub fn correlation_id(&self) -> &CorrelationId {
        &self.meta.correlation_id
    }

    /// The schema version of the payload (§5.2, mandatory).
    pub fn schema_version(&self) -> SchemaVersion {
        self.meta.schema_version
    }

    /// The topic this event belongs to.
    pub fn topic(&self) -> &Topic {
        &self.meta.topic
    }

    /// Re-validate the meta *and* the derived values.
    ///
    /// The second half is not decorative: it recomputes the canonical payload,
    /// the signing bytes and the digest from the fields and compares them with
    /// the cached copies. An `Event` whose cache disagrees with its fields is
    /// refused by [`crate::bus::EventBus::publish`] rather than entering the log
    /// and the Merkle tree under a digest that does not describe it.
    pub fn validate(&self) -> BusResult<()> {
        self.meta.validate()?;

        let canonical_meta = canonical_json_bytes("meta", &self.meta)?;
        let canonical_payload = canonical_json_bytes("payload", &self.payload)?;

        if self.canonical_payload() != canonical_payload.as_slice() {
            return Err(BusError::internal(
                "cached canonical payload disagrees with the payload",
            ));
        }

        let expected_signing_bytes = assemble_signing_bytes(&canonical_meta, &canonical_payload);
        if self.signing_bytes() != expected_signing_bytes.as_slice() {
            return Err(BusError::internal(
                "cached signing bytes disagree with meta and payload",
            ));
        }

        if *blake3::hash(&expected_signing_bytes).as_bytes() != self.digest {
            return Err(BusError::internal(
                "cached digest disagrees with the signing bytes",
            ));
        }

        Ok(())
    }
}

impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Event")
            .field("event_id", &self.meta.event_id)
            .field("trace_id", &self.meta.trace_id)
            .field("causation_id", &self.meta.causation_id)
            .field("correlation_id", &self.meta.correlation_id)
            .field("topic", &self.meta.topic)
            .field("schema_version", &self.meta.schema_version)
            .field("payload_bytes", &self.canonical_payload.len())
            .field("digest", &hex::encode(self.digest))
            .field("signed", &self.is_signed())
            .finish()
    }
}

/// Assemble the signing message. One place, so the framing cannot drift between
/// construction and [`Event::validate`].
fn assemble_signing_bytes(canonical_meta: &[u8], canonical_payload: &[u8]) -> Vec<u8> {
    let meta_len = canonical_meta.len() as u64;

    let mut bytes = Vec::with_capacity(
        SIGNING_DOMAIN.len() + std::mem::size_of::<u64>() + canonical_meta.len() + canonical_payload.len(),
    );
    bytes.extend_from_slice(SIGNING_DOMAIN);
    bytes.extend_from_slice(&meta_len.to_le_bytes());
    bytes.extend_from_slice(canonical_meta);
    bytes.extend_from_slice(canonical_payload);
    bytes
}

/// Canonical JSON: `to_value`, an explicit recursive alphabetical key sort
/// ([`sort_object_keys`]) so the result does not depend on `serde_json`'s
/// `Map` implementation, then compact `to_vec`. See the module docs.
fn canonical_json_bytes<T: serde::Serialize>(
    field: &'static str,
    value: &T,
) -> BusResult<Vec<u8>> {
    let as_value = serde_json::to_value(value)
        .map_err(|e| BusError::malformed(field, format!("cannot canonicalise as JSON: {e}")))?;
    serde_json::to_vec(&sort_object_keys(&as_value))
        .map_err(|e| BusError::malformed(field, format!("cannot serialise canonical JSON: {e}")))
}

/// Rebuild `value` with the keys of every object, at any depth, in
/// alphabetical order: collected into a `BTreeMap` and reinserted in that
/// order, so the outcome is the same whether `serde_json`'s `Map` is its
/// default `BTreeMap` or an insertion-ordered `IndexMap` (which is what
/// `serde_json/preserve_order` — enabled by some dependencies and unified
/// across the build — swaps in). Under the default `Map` this is a no-op:
/// iteration is already sorted, so no byte of a canonical form changes.
/// Arrays keep their element order (JSON arrays are ordered); scalars pass
/// through unchanged.
fn sort_object_keys(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, member)| (key.clone(), sort_object_keys(member)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(sort_object_keys).collect()),
        scalar => scalar.clone(),
    }
}

/// A required string member of `meta`.
fn required_str<'a>(map: &'a Map<String, Value>, field: &'static str) -> BusResult<&'a str> {
    match map.get(field) {
        Some(Value::String(value)) => Ok(value.as_str()),
        None | Some(Value::Null) => Err(BusError::MissingMeta { field }),
        Some(_) => Err(BusError::malformed(field, "expected a JSON string")),
    }
}

/// Parse `meta` member by member so that each failure names its field.
///
/// Unknown members are ignored — `T-E3`.
fn parse_meta(map: &Map<String, Value>) -> BusResult<EventMeta> {
    let event_id = EventId::new(required_str(map, "event_id")?)?;
    let trace_id = TraceId::new(required_str(map, "trace_id")?)?;

    let causation_id = match map.get("causation_id") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(EventId::new(value.as_str())?),
        Some(_) => {
            return Err(BusError::malformed(
                "causation_id",
                "expected a JSON string or null",
            ))
        }
    };

    let correlation_id = CorrelationId::new(required_str(map, "correlation_id")?)?;
    let schema_version = parse_schema_version(map)?;
    let topic = Topic::parse(required_str(map, "topic")?)?;

    let timestamp_unix_us = match map.get("timestamp_unix_us") {
        None | Some(Value::Null) => {
            return Err(BusError::MissingMeta {
                field: "timestamp_unix_us",
            })
        }
        Some(value) => value.as_u64().ok_or_else(|| {
            BusError::malformed(
                "timestamp_unix_us",
                "expected a non-negative integer (negative, fractional and non-numeric all land \
                 here)",
            )
        })?,
    };

    Ok(EventMeta {
        event_id,
        trace_id,
        causation_id,
        correlation_id,
        schema_version,
        topic,
        timestamp_unix_us,
    })
}

/// Parse `meta.schema_version`.
fn parse_schema_version(map: &Map<String, Value>) -> BusResult<SchemaVersion> {
    let value = map
        .get("schema_version")
        .ok_or(BusError::MissingMeta {
            field: "schema_version",
        })?;

    let object = value.as_object().ok_or_else(|| {
        BusError::malformed(
            "schema_version",
            "expected a JSON object with `major` and `minor`",
        )
    })?;

    let component = |name: &'static str| -> BusResult<u32> {
        let raw = object
            .get(name)
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                BusError::malformed("schema_version", format!("`{name}` must be a non-negative integer"))
            })?;
        u32::try_from(raw).map_err(|_| {
            BusError::malformed("schema_version", format!("`{name}` does not fit in u32"))
        })
    };

    SchemaVersion::new(component("major")?, component("minor")?)
}

/// Parse the `signature` member: absent or `null` means unsigned.
fn parse_signature(value: Option<&Value>) -> BusResult<Option<EventSignature>> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(hex)) => Ok(Some(EventSignature::from_hex(hex)?)),
        Some(_) => Err(BusError::malformed(
            "signature",
            "expected a hex string or null",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topic::ATTESTATION_COMPLETED;
    use serde_json::json;

    fn meta(event_id: &str) -> EventMeta {
        EventMeta::root(
            ATTESTATION_COMPLETED,
            SchemaVersion::V1,
            EventId::new(event_id).expect("valid"),
            TraceId::new("trc_1").expect("valid"),
            CorrelationId::new("cor_1").expect("valid"),
            42,
        )
    }

    fn event() -> Event {
        Event::new(meta("evt_a"), json!({"ok": true, "n": 3})).expect("valid event")
    }

    #[test]
    fn signing_bytes_are_domain_separated_and_length_framed() {
        let event = event();
        let bytes = event.signing_bytes();
        assert!(bytes.starts_with(SIGNING_DOMAIN));
        let meta_len = u64::from_le_bytes(
            bytes[SIGNING_DOMAIN.len()..SIGNING_DOMAIN.len() + 8]
                .try_into()
                .expect("8 bytes"),
        ) as usize;
        assert!(meta_len > 0);
        assert!(bytes.len() > SIGNING_DOMAIN.len() + 8 + meta_len);
    }

    #[test]
    fn digest_is_blake3_of_the_signing_bytes() {
        let event = event();
        assert_eq!(
            event.digest(),
            *blake3::hash(event.signing_bytes()).as_bytes()
        );
        assert_eq!(LEAF_DOMAIN, 0x00);
        assert_eq!(INTERNAL_DOMAIN, 0x01);
    }

    #[test]
    fn meta_field_order_does_not_affect_the_signing_bytes() {
        // Canonicalisation goes through `serde_json::Value`, so a different
        // *serialisation* order of the same meta yields the same bytes.
        let a = meta("evt_same");
        let b = EventMeta {
            timestamp_unix_us: a.timestamp_unix_us,
            topic: a.topic.clone(),
            schema_version: a.schema_version,
            correlation_id: a.correlation_id.clone(),
            causation_id: a.causation_id.clone(),
            trace_id: a.trace_id.clone(),
            event_id: a.event_id.clone(),
        };
        let event_a = Event::new(a, json!({"k": 1})).expect("valid");
        let event_b = Event::new(b, json!({"k": 1})).expect("valid");
        assert_eq!(event_a.signing_bytes(), event_b.signing_bytes());
        assert_eq!(event_a.digest(), event_b.digest());
    }

    #[test]
    fn payload_key_order_does_not_affect_the_signing_bytes() {
        let one = Event::new(meta("evt_1"), json!({"a": 1, "b": 2})).expect("valid");
        let other = Event::new(meta("evt_1"), json!({"b": 2, "a": 1})).expect("valid");
        assert_eq!(one.digest(), other.digest());
    }

    #[test]
    fn nested_payload_key_order_does_not_affect_the_signing_bytes() {
        // The recursive case: the key sort must reach objects nested inside
        // objects (the flat case above only exercises the top level). This is
        // what breaks when `serde_json/preserve_order` makes `Map` an
        // insertion-ordered `IndexMap` — the explicit sort is what keeps the
        // signing bytes independent of the `Map` implementation.
        let one = Event::new(
            meta("evt_n"),
            json!({"outer": {"b": 2, "a": {"y": 1, "x": 2}}, "z": 1}),
        )
        .expect("valid");
        let other = Event::new(
            meta("evt_n"),
            json!({"z": 1, "outer": {"a": {"x": 2, "y": 1}, "b": 2}}),
        )
        .expect("valid");
        assert_eq!(one.canonical_payload(), other.canonical_payload());
        assert_eq!(one.signing_bytes(), other.signing_bytes());
        assert_eq!(one.digest(), other.digest());
    }

    #[test]
    fn different_payloads_produce_different_digests() {
        let one = Event::new(meta("evt_1"), json!({"a": 1})).expect("valid");
        let other = Event::new(meta("evt_1"), json!({"a": 2})).expect("valid");
        assert_ne!(one.digest(), other.digest());
    }

    #[test]
    fn accessors_expose_the_four_identifiers() {
        let event = event();
        assert_eq!(event.event_id().as_str(), "evt_a");
        assert_eq!(event.trace_id().as_str(), "trc_1");
        assert_eq!(event.causation_id(), None);
        assert_eq!(event.correlation_id().as_str(), "cor_1");
        assert_eq!(event.schema_version(), SchemaVersion::V1);
        assert_eq!(event.topic().as_str(), "attestation.completed");
        assert_eq!(event.meta().timestamp_unix_us, 42);
    }

    #[test]
    fn json_round_trip_preserves_the_signing_bytes_and_digest() {
        let original = event();
        let json = original.to_json().expect("serializes");
        let parsed = Event::from_json(json.as_bytes()).expect("parses");
        assert_eq!(parsed.signing_bytes(), original.signing_bytes());
        assert_eq!(parsed.digest(), original.digest());
        assert_eq!(parsed, original);
    }

    #[test]
    fn json_round_trip_preserves_a_signature() {
        let signed = event().with_signature(EventSignature::from_bytes(vec![9u8; 64]).expect("valid"));
        let json = signed.to_json().expect("serializes");
        assert!(json.contains("\"signature\":\"0909"));
        let parsed = Event::from_json(json.as_bytes()).expect("parses");
        assert!(parsed.is_signed());
        assert_eq!(parsed.signature(), signed.signature());
        assert_eq!(parsed.digest(), signed.digest());
    }

    #[test]
    fn unsigned_events_render_a_null_signature() {
        let json = event().to_json().expect("serializes");
        assert!(json.contains("\"signature\":null"), "{json}");
    }

    #[test]
    fn from_json_names_the_missing_field() {
        match Event::from_json(br#"{"payload":{}}"#) {
            Err(BusError::MissingMeta { field }) => assert_eq!(field, "meta"),
            other => panic!("expected MissingMeta, got {other:?}"),
        }

        let json = event().to_json().expect("serializes");
        let mut value: Value = serde_json::from_str(&json).expect("json");
        value["meta"]
            .as_object_mut()
            .expect("meta object")
            .remove("trace_id");
        match Event::from_json(value.to_string().as_bytes()) {
            Err(BusError::MissingMeta { field }) => assert_eq!(field, "trace_id"),
            other => panic!("expected MissingMeta(trace_id), got {other:?}"),
        }
    }

    #[test]
    fn from_json_rejects_each_malformed_member_with_a_typed_error() {
        let base = event().to_json().expect("serializes");
        let mutate = |key: &str, replacement: &Value| {
            let mut value: Value = serde_json::from_str(&base).expect("json");
            value["meta"]
                .as_object_mut()
                .expect("meta object")
                .insert(key.to_string(), replacement.clone());
            value.to_string()
        };

        for (key, replacement, expected) in [
            ("event_id", json!(""), "event_id"),
            ("trace_id", json!(""), "trace_id"),
            ("correlation_id", json!(""), "correlation_id"),
            ("event_id", json!(7), "event_id"),
            ("topic", json!("Not A Topic"), "topic"),
            ("schema_version", json!({"major": 0, "minor": 0}), "schema_version"),
            ("timestamp_unix_us", json!(-1), "timestamp_unix_us"),
            ("timestamp_unix_us", json!("now"), "timestamp_unix_us"),
        ] {
            let json = mutate(key, &replacement);
            let err = Event::from_json(json.as_bytes())
                .expect_err(&format!("{key} should be rejected"));
            let named = match &err {
                BusError::InvalidId { kind, .. } => *kind,
                BusError::InvalidTopic { .. } => "topic",
                BusError::InvalidSchemaVersion { .. } => "schema_version",
                BusError::MissingMeta { field } => field,
                BusError::Malformed { field, .. } => field,
                other => panic!("unexpected error for {key}: {other:?}"),
            };
            assert_eq!(named, expected, "for {key} with {replacement}");
        }
    }

    #[test]
    fn from_json_rejects_structural_malformations() {
        assert!(matches!(
            Event::from_json(b"not json"),
            Err(BusError::Malformed { field: "envelope", .. })
        ));
        assert!(matches!(
            Event::from_json(b"[]"),
            Err(BusError::Malformed { field: "envelope", .. })
        ));
        assert!(matches!(
            Event::from_json(&[0xff, 0xfe]),
            Err(BusError::Malformed { field: "envelope", .. })
        ));
        assert!(matches!(
            Event::from_json(br#"{"meta": 3, "payload": null}"#),
            Err(BusError::Malformed { field: "meta", .. })
        ));
        // Mandatory meta members are reported before the payload is considered.
        assert!(matches!(
            Event::from_json(br#"{"meta": {"event_id": "e"}}"#),
            Err(BusError::MissingMeta { field: "trace_id" })
        ));
        // `payload` absent, with otherwise valid meta.
        let json = event().to_json().expect("serializes");
        let mut value: Value = serde_json::from_str(&json).expect("json");
        value.as_object_mut().expect("object").remove("payload");
        assert_eq!(
            Event::from_json(value.to_string().as_bytes()),
            Err(BusError::MissingPayload)
        );
        // `signature` present but not a string and not null.
        let mut value: Value = serde_json::from_str(&json).expect("json");
        value["signature"] = json!(12);
        assert!(matches!(
            Event::from_json(value.to_string().as_bytes()),
            Err(BusError::Malformed { field: "signature", .. })
        ));
        // `signature` present but not hex.
        value["signature"] = json!("zzz");
        assert!(matches!(
            Event::from_json(value.to_string().as_bytes()),
            Err(BusError::Malformed { field: "signature", .. })
        ));
    }

    #[test]
    fn unknown_members_are_ignored_not_fatal() {
        let json = event().to_json().expect("serializes");
        let mut value: Value = serde_json::from_str(&json).expect("json");
        value["future_member"] = json!({"added": "later"});
        value["meta"]["future_meta_member"] = json!([1, 2, 3]);
        value["payload"]["future_payload_field"] = json!({"deep": true});

        let parsed = Event::from_json(value.to_string().as_bytes()).expect("T-E3: no panic, no error");
        assert_eq!(parsed.event_id().as_str(), "evt_a");
        assert_eq!(
            parsed.payload().get("future_payload_field"),
            Some(&json!({"deep": true}))
        );
    }

    #[test]
    fn validate_accepts_a_well_formed_event() {
        assert!(event().validate().is_ok());
    }

    #[test]
    fn debug_prints_identifiers_rather_than_the_payload() {
        let rendered = format!("{:?}", event());
        assert!(rendered.contains("evt_a"), "{rendered}");
        assert!(rendered.contains("attestation.completed"), "{rendered}");
        assert!(!rendered.contains("true"), "payload contents must not leak: {rendered}");
    }

    #[test]
    fn non_object_payloads_are_legal_payloads() {
        for payload in [json!(null), json!(7), json!("text"), json!([1, 2, 3])] {
            let event = Event::new(meta("evt_p"), payload.clone()).expect("valid");
            assert_eq!(event.payload(), &payload);
        }
    }
}
