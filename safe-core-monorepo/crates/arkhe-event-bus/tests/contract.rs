//! The §5.2 contract, asserted from **outside** the crate.
//!
//! Every test here uses only the public API, the way a domain crate would. The
//! properties under test are the paper's own:
//!
//! * **T-E2** (§4.2) — every event carries `event_id`, `trace_id`,
//!   `causation_id` and `correlation_id`, end to end.
//! * **T-E3** (§4.2) — unknown future event schemas never panic a consumer.
//! * §5.2 — *"mandatory `EventMeta` with identifiers and schema version on every
//!   event; malformed events are unpublishable"*.
//!
//! See `README.md` for what is deliberately **not** asserted anywhere.

use std::panic::{catch_unwind, AssertUnwindSafe};

use serde_json::json;

use arkhe_event_bus::bus::{BusConfig, EventBus};
use arkhe_event_bus::error::BusError;
use arkhe_event_bus::event::Event;
use arkhe_event_bus::log::{MerkleRoot, SequenceNumber};
use arkhe_event_bus::meta::{CorrelationId, EventId, EventMeta, SchemaVersion, TraceId};
use arkhe_event_bus::topic::{
    Topic, ATTESTATION_COMPLETED, ATTESTATION_FAILED, CREDENTIALS_REJECTED,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn meta(topic: Topic, event_id: &str, trace_id: &str, correlation_id: &str) -> EventMeta {
    EventMeta::root(
        topic,
        SchemaVersion::V1,
        EventId::new(event_id).expect("valid event id"),
        TraceId::new(trace_id).expect("valid trace id"),
        CorrelationId::new(correlation_id).expect("valid correlation id"),
        1_700_000_000_000_000,
    )
}

fn event(topic: Topic, event_id: &str, trace_id: &str, correlation_id: &str) -> Event {
    Event::new(
        meta(topic, event_id, trace_id, correlation_id),
        json!({ "event_id": event_id }),
    )
    .expect("valid event")
}

/// A valid envelope for `evt_ok`, used as the base for mutation tests.
fn valid_envelope() -> String {
    event(ATTESTATION_COMPLETED, "evt_ok", "trc_1", "cor_1")
        .to_json()
        .expect("serializes")
}

/// Rewrite one `meta` member of a valid envelope.
fn envelope_with_meta(key: &str, replacement: serde_json::Value) -> String {
    let mut value: serde_json::Value =
        serde_json::from_str(&valid_envelope()).expect("envelope is JSON");
    value["meta"]
        .as_object_mut()
        .expect("meta is an object")
        .insert(key.to_string(), replacement);
    value.to_string()
}

// ---------------------------------------------------------------------------
// T-E2 — the four identifiers, end to end, through a causal chain
// ---------------------------------------------------------------------------

#[test]
fn four_identifiers_survive_a_causal_chain_end_to_end() {
    let bus = EventBus::new(BusConfig::default());

    // A→B→C: one trace, one correlation, each event caused by the previous one.
    let root_meta = meta(ATTESTATION_COMPLETED, "evt_a", "trc_chain", "cor_settlement");
    let second_meta = root_meta.caused_by(EventId::new("evt_b").expect("valid"), 1_700_000_000_000_001);
    let third_meta = second_meta.caused_by(EventId::new("evt_c").expect("valid"), 1_700_000_000_000_002);

    // Subscribe *before* publishing: the bus delivers from subscription time
    // forward (see the bus module docs).
    let mut subscriber = bus.subscribe(ATTESTATION_COMPLETED);

    for (meta, stage) in [
        (root_meta.clone(), "requested"),
        (second_meta.clone(), "verified"),
        (third_meta.clone(), "settled"),
    ] {
        let sequence = bus
            .publish(Event::new(meta, json!({ "stage": stage })).expect("valid event"))
            .expect("accepted");
        assert!(sequence.get() >= 1);
    }

    // 1. Delivery: the identifiers arrive with the events.
    let delivered = subscriber.drain().expect("no lag");
    assert_eq!(delivered.len(), 3, "the chain must arrive whole");
    let delivered_ids: Vec<&str> = delivered
        .iter()
        .map(|delivery| delivery.event_id().as_str())
        .collect();
    assert_eq!(delivered_ids, ["evt_a", "evt_b", "evt_c"]);

    assert_eq!(delivered[0].sequence().get(), 1);
    assert_eq!(delivered[2].sequence().get(), 3);

    // 2. Causation: B points at A, C points at B, A is a root.
    assert!(
        delivered[0].event().causation_id().is_none(),
        "A is the chain root"
    );
    assert_eq!(
        delivered[1].event().causation_id().map(EventId::as_str),
        Some("evt_a")
    );
    assert_eq!(
        delivered[2].event().causation_id().map(EventId::as_str),
        Some("evt_b")
    );

    // 3. trace_id is constant; correlation_id is constant.
    for delivery in &delivered {
        assert_eq!(delivery.event().trace_id().as_str(), "trc_chain");
        assert_eq!(
            delivery.event().correlation_id().as_str(),
            "cor_settlement"
        );
    }

    // 4. The log holds the same four identifiers, in the same order.
    let log = bus.log();
    let logged = log.events();
    assert_eq!(logged.len(), 3);
    for (index, event) in logged.iter().enumerate() {
        assert_eq!(event.event_id().as_str(), delivered[index].event().event_id().as_str());
        assert_eq!(event.trace_id().as_str(), "trc_chain");
        assert_eq!(event.correlation_id().as_str(), "cor_settlement");
        assert_eq!(event.causation_id(), delivered[index].event().causation_id());
    }

    // 5. And a JSON round trip of every event preserves all four, plus the
    //    digest the log committed to.
    for (sequence, event) in logged.iter().enumerate() {
        let bytes = event.to_json().expect("serializes");
        let reparsed = Event::from_json(bytes.as_bytes()).expect("parses");
        assert_eq!(reparsed.meta(), event.meta(), "meta must survive JSON");
        assert_eq!(reparsed.digest(), event.digest(), "digest must survive JSON");
        assert_eq!(
            log.entries()[sequence].digest(),
            event.digest(),
            "the logged digest must be the event's digest"
        );
    }
}

#[tokio::test]
async fn four_identifiers_survive_async_delivery() {
    let bus = EventBus::new(BusConfig::default());
    let mut subscriber = bus.subscribe(ATTESTATION_FAILED);

    let root_meta = meta(ATTESTATION_FAILED, "evt_root", "trc_async", "cor_async");
    let child_meta = root_meta.caused_by(EventId::new("evt_child").expect("valid"), 2);

    bus.publish(Event::new(root_meta, json!({ "stage": "requested" })).expect("valid"))
        .expect("accepted");
    bus.publish(Event::new(child_meta, json!({ "stage": "failed" })).expect("valid"))
        .expect("accepted");

    let first = subscriber.recv().await.expect("delivered");
    let second = subscriber.recv().await.expect("delivered");

    assert_eq!(first.event_id().as_str(), "evt_root");
    assert_eq!(second.event_id().as_str(), "evt_child");
    assert_eq!(second.event().trace_id().as_str(), "trc_async");
    assert_eq!(
        second.event().causation_id().map(EventId::as_str),
        Some("evt_root")
    );
    assert_eq!(first.sequence().get() + 1, second.sequence().get());
}

// ---------------------------------------------------------------------------
// T-E3 — unknown topics, unknown schemas, unknown members: never a panic
// ---------------------------------------------------------------------------

#[test]
fn unknown_topics_do_not_panic() {
    let bus = EventBus::new(BusConfig::default());

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let future_topic = Topic::parse("arkhe_physics.experiment.published").expect("parses");
        let mut subscriber = bus.subscribe(future_topic.clone());
        let event = event(future_topic.clone(), "evt_future", "trc_future", "cor_future");

        let sequence = bus.publish(event).expect("an unknown topic is publishable");
        let delivery = subscriber.try_recv().expect("no lag").expect("delivered");
        (future_topic.to_string(), sequence.get(), delivery.topic().to_string())
    }));

    let (topic, sequence, delivered_topic) =
        outcome.expect("T-E3: an unknown topic must not panic");
    assert_eq!(topic, "arkhe_physics.experiment.published");
    assert_eq!(delivered_topic, topic);
    assert_eq!(sequence, 1);
    assert!(
        !Topic::parse("arkhe_physics.experiment.published")
            .expect("parses")
            .is_canonical(),
        "the probe topic is deliberately not one of the nine §5.2 topics"
    );
}

#[test]
fn unknown_future_schema_versions_do_not_panic_and_yield_a_typed_error() {
    let bus = EventBus::new(BusConfig::default());

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // A publisher from the future, using major 2.
        let mut consumer = bus.subscribe(CREDENTIALS_REJECTED);
        let future_schema = SchemaVersion::new(2, 0).expect("valid");
        let future_meta = EventMeta {
            schema_version: future_schema,
            ..meta(CREDENTIALS_REJECTED, "evt_v2", "trc_v2", "cor_v2")
        };
        let sequence = bus
            .publish(Event::new(future_meta, json!({ "new_field": true })).expect("valid"))
            .expect("a newer schema is publishable");

        // A today-consumer, behind the bus's own delivery: the async API turns
        // the unreadable schema into a typed error rather than a panic.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        let refusal = runtime.block_on(consumer.recv_understanding(SchemaVersion::V1));
        (sequence.get(), refusal.err())
    }));

    let (sequence, refusal) = outcome.expect("T-E3: a v2 event must not panic a v1 consumer");
    match refusal {
        Some(BusError::UnsupportedSchemaVersion {
            sequence: refused_sequence,
            event_major,
            consumer_major,
        }) => {
            assert_eq!(refused_sequence, sequence, "the refusal names the event");
            assert_eq!(event_major, 2);
            assert_eq!(consumer_major, 1);
        }
        other => panic!("expected a typed UnsupportedSchemaVersion, got {other:?}"),
    }

    // Nothing was lost: the log still holds the event, and the consumer can read
    // it deliberately.
    let logged = bus.replay(SequenceNumber::new(sequence)).expect("in range");
    assert_eq!(logged.len(), 1);
    assert_eq!(logged[0].schema_version().major(), 2);
    assert_eq!(
        logged[0].payload().get("new_field"),
        Some(&json!(true)),
        "the unknown member survived"
    );

    // A higher *minor* within a known major is additive and accepted.
    let minor_meta = EventMeta {
        schema_version: SchemaVersion::new(1, 7).expect("valid"),
        ..meta(ATTESTATION_COMPLETED, "evt_v1_7", "trc_minor", "cor_minor")
    };
    let mut consumer = bus.subscribe(ATTESTATION_COMPLETED);
    bus.publish(Event::new(minor_meta, json!({ "added_minor_wise": 1 })).expect("valid"))
        .expect("accepted");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    let delivery = runtime
        .block_on(consumer.recv_understanding(SchemaVersion::V1))
        .expect("a v1.7 event is readable by a v1 consumer");
    assert_eq!(delivery.schema_version().minor(), 7);
}

#[test]
fn unknown_payload_members_do_not_panic() {
    let bus = EventBus::new(BusConfig::default());
    let mut subscriber = bus.subscribe(ATTESTATION_COMPLETED);

    let payload = json!({
        "known": 1,
        "member_from_a_future_schema": {
            "nested": [1, 2, {"deeper": null}],
            "unicode": "confiança verificável"
        }
    });
    let event = Event::new(
        meta(ATTESTATION_COMPLETED, "evt_future_payload", "trc_p", "cor_p"),
        payload.clone(),
    )
    .expect("valid");

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        bus.publish(event).expect("accepted");
        let delivery = subscriber.try_recv().expect("no lag").expect("delivered");
        delivery.event().payload().clone()
    }));

    let seen = outcome.expect("T-E3: unknown payload members must not panic");
    assert_eq!(seen, payload, "unknown members are preserved verbatim");
}

// ---------------------------------------------------------------------------
// §5.2 — malformed events are unpublishable, and leave no trace
// ---------------------------------------------------------------------------

#[test]
fn malformed_wire_envelopes_are_unpublishable_and_leave_no_trace() {
    let bus = EventBus::new(BusConfig::default());
    let mut subscriber = bus.subscribe(ATTESTATION_COMPLETED);

    // A well-formed event first, so that "no trace" means "no *new* trace".
    bus.publish(event(ATTESTATION_COMPLETED, "evt_accepted", "trc_ok", "cor_ok"))
        .expect("accepted");
    let events_after_accept = bus.log_len();
    let root_after_accept = bus.current_root();
    let delivered_before = subscriber.drain().expect("no lag").len();

    let malformed: Vec<(&str, String)> = vec![
        ("no meta member", String::from(r#"{"payload": {}}"#)),
        (
            "empty event_id",
            envelope_with_meta("event_id", json!("")),
        ),
        (
            "whitespace-only event_id",
            envelope_with_meta("event_id", json!("   ")),
        ),
        (
            "empty trace_id",
            envelope_with_meta("trace_id", json!("")),
        ),
        (
            "empty correlation_id",
            envelope_with_meta("correlation_id", json!("")),
        ),
        (
            "schema_version 0.0",
            envelope_with_meta("schema_version", json!({"major": 0, "minor": 0})),
        ),
        (
            "schema_version missing",
            envelope_with_meta("schema_version", json!(null)),
        ),
        ("unparseable topic", envelope_with_meta("topic", json!("Not A Topic"))),
        ("topic of the wrong type", envelope_with_meta("topic", json!(7))),
        ("negative timestamp", envelope_with_meta("timestamp_unix_us", json!(-1))),
        ("not JSON", String::from("{oops")),
        ("not an object", String::from("[1,2,3]")),
    ];

    for (description, bytes) in &malformed {
        let error = match Event::from_json(bytes.as_bytes()) {
            Err(error) => error,
            Ok(event) => bus
                .publish(event)
                .expect_err(&format!("{description} must not be publishable")),
        };

        assert!(
            error.is_rejection(),
            "{description}: expected a refusal, got {error}"
        );

        // The refusal left nothing behind.
        assert_eq!(
            bus.log_len(),
            events_after_accept,
            "{description}: the log grew"
        );
        assert_eq!(
            bus.current_root(),
            root_after_accept,
            "{description}: the Merkle root moved"
        );
    }

    // The subscriber received exactly what was legitimately published, and
    // nothing from the twelve refusals.
    let delivered_after = subscriber.drain().expect("no lag");
    assert_eq!(
        delivered_before + delivered_after.len(),
        events_after_accept,
        "refused events must never be delivered"
    );
    assert_eq!(delivered_after.len(), 0);
}

#[test]
fn the_typed_constructors_reject_before_an_event_can_exist() {
    // §5.2's mandatory EventMeta and identifiers are enforced by construction:
    // these gates are the first line, and `publish`'s re-check is the second.
    assert!(matches!(
        EventId::new(""),
        Err(BusError::InvalidId { kind: "event_id", .. })
    ));
    assert!(matches!(
        EventId::new("  "),
        Err(BusError::InvalidId { kind: "event_id", .. })
    ));
    assert!(matches!(
        TraceId::new(""),
        Err(BusError::InvalidId { kind: "trace_id", .. })
    ));
    assert!(matches!(
        CorrelationId::new(""),
        Err(BusError::InvalidId { kind: "correlation_id", .. })
    ));
    assert!(matches!(
        SchemaVersion::new(0, 3),
        Err(BusError::InvalidSchemaVersion { major: 0, .. })
    ));
    assert!(matches!(
        Topic::parse(""),
        Err(BusError::InvalidTopic { .. })
    ));
    assert!(matches!(
        Topic::parse("Storage Stored"),
        Err(BusError::InvalidTopic { .. })
    ));
}

#[test]
fn duplicate_event_ids_are_refused_and_leave_no_trace() {
    let bus = EventBus::new(BusConfig::default());
    let mut subscriber = bus.subscribe(ATTESTATION_COMPLETED);

    bus.publish(event(ATTESTATION_COMPLETED, "evt_once", "trc_d", "cor_d"))
        .expect("accepted");
    let root_before = bus.current_root();
    let delivered_before = subscriber.drain().expect("no lag").len();

    // Same event_id, different payload: still the same identity.
    let conflicting = Event::new(
        meta(ATTESTATION_COMPLETED, "evt_once", "trc_other", "cor_other"),
        json!({ "different": true }),
    )
    .expect("valid");

    match bus.publish(conflicting) {
        Err(BusError::DuplicateEventId { event_id }) => assert_eq!(event_id, "evt_once"),
        other => panic!("expected DuplicateEventId, got {other:?}"),
    }

    assert_eq!(bus.log_len(), 1);
    assert_eq!(bus.current_root(), root_before, "the root must not move");
    assert_eq!(delivered_before, 1, "the accepted event was delivered once");
    assert!(
        subscriber.drain().expect("no lag").is_empty(),
        "the refused duplicate must never be delivered"
    );
}

// ---------------------------------------------------------------------------
// The canonical topics of §5.2
// ---------------------------------------------------------------------------

#[test]
fn every_canonical_topic_is_publishable_and_subscribable() {
    let bus = EventBus::new(BusConfig::default());
    let topics = [
        arkhe_event_bus::topic::ATTESTATION_COMPLETED,
        arkhe_event_bus::topic::ATTESTATION_FAILED,
        arkhe_event_bus::topic::STORAGE_STORED,
        arkhe_event_bus::topic::AMENDMENTS_PROPOSED,
        arkhe_event_bus::topic::AMENDMENTS_APPLIED,
        arkhe_event_bus::topic::KILL_SWITCH_ACTIVATED,
        arkhe_event_bus::topic::CREDENTIALS_VERIFIED,
        arkhe_event_bus::topic::CREDENTIALS_REJECTED,
        arkhe_event_bus::topic::ALERTS,
    ];
    assert_eq!(topics.len(), 9);
    assert_eq!(arkhe_event_bus::topic::CANONICAL_TOPICS.len(), 9);

    let mut all = bus.subscribe_all();

    for (index, topic) in topics.iter().enumerate() {
        assert!(topic.is_canonical(), "{topic} must be canonical");
        let mut subscriber = bus.subscribe(topic.clone());
        let event_id = format!("evt_topic_{index}");
        bus.publish(event(topic.clone(), &event_id, "trc_topics", "cor_topics"))
            .expect("accepted");

        let direct = subscriber.try_recv().expect("no lag").expect("delivered");
        assert_eq!(direct.topic(), topic);
        assert_eq!(direct.event_id().as_str(), event_id);

        let broadcast = all.try_recv().expect("no lag").expect("delivered");
        assert_eq!(broadcast.topic(), topic);
    }

    assert_eq!(bus.log_len(), 9);
    let seen: Vec<String> = bus
        .log()
        .events()
        .iter()
        .map(|event| event.topic().to_string())
        .collect();
    assert_eq!(
        seen,
        arkhe_event_bus::topic::CANONICAL_TOPICS
            .iter()
            .map(|topic| topic.to_string())
            .collect::<Vec<String>>()
    );
}

#[test]
fn the_empty_log_root_is_a_constant() {
    let bus = EventBus::new(BusConfig::default());
    assert_eq!(bus.current_root(), MerkleRoot::empty());
    assert_eq!(
        bus.current_root().to_hex(),
        MerkleRoot::empty().to_hex(),
        "BLAKE3(EMPTY_ROOT_DOMAIN) is deterministic"
    );
    assert_eq!(bus.log_len(), 0);
    assert!(bus.sequence_head().is_none());
    assert!(bus.checkpoint().is_genesis());

    // Publishing moves it, permanently.
    bus.publish(event(ATTESTATION_COMPLETED, "evt_move", "trc_r", "cor_r"))
        .expect("accepted");
    assert_ne!(bus.current_root(), MerkleRoot::empty());
    assert_eq!(bus.sequence_head(), Some(SequenceNumber::FIRST));
    assert!(!bus.checkpoint().is_genesis());
}
