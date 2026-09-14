//! The crate's declared invariants, and the delivery semantics they depend on.
//!
//! This file is deliberately blunt about `T-E1`. The paper (DOI
//! 10.5281/zenodo.21383201, §4.2) states:
//!
//! > Domain crates never import each other. All domain-to-domain communication
//! > flows through `arkhe-event-bus` as typed publish/subscribe events. A direct
//! > import fails compilation and blocks CI (invariant **T-E1**).
//!
//! That is a property **between crates**. No test inside `arkhe-event-bus` can
//! observe whether `arkhe-attestation` imports `arkhe-storage`; claiming
//! otherwise would be fabricated evidence. What can be tested is that the crate
//! *declares* `T-E1` with the paper's identifier, does not invent a runtime
//! "check" for it, and provides the typed pub/sub that makes E1's architecture
//! expressible. That is what this file does.

use arkhe_event_bus::bus::{BusConfig, EventBus};
use arkhe_event_bus::error::BusError;
use arkhe_event_bus::event::Event;
use arkhe_event_bus::invariant::{self, InvariantSource};
use arkhe_event_bus::log::SequenceNumber;
use arkhe_event_bus::meta::{CorrelationId, EventId, EventMeta, SchemaVersion, TraceId};
use arkhe_event_bus::topic::{ATTESTATION_COMPLETED, STORAGE_STORED};

use serde_json::json;

fn meta(topic: arkhe_event_bus::topic::Topic, event_id: &str) -> EventMeta {
    EventMeta::root(
        topic,
        SchemaVersion::V1,
        EventId::new(event_id).expect("valid"),
        TraceId::new("trc_inv").expect("valid"),
        CorrelationId::new("cor_inv").expect("valid"),
        1,
    )
}

fn event(topic: arkhe_event_bus::topic::Topic, event_id: &str) -> Event {
    Event::new(meta(topic, event_id), json!({ "k": event_id })).expect("valid")
}

// ---------------------------------------------------------------------------
// The declared invariants
// ---------------------------------------------------------------------------

#[test]
fn every_declared_invariant_that_has_a_check_passes() {
    let results = invariant::check_all();
    assert_eq!(
        results.len(),
        4,
        "EB-01, EB-02, T-E2 and T-E3 have executable checks"
    );

    for (declared, result) in results {
        if let Err(error) = result {
            panic!("{} ({}) failed: {error}", declared.id, declared.name);
        }
    }
}

#[test]
fn t_e1_is_declared_with_the_papers_identifier_and_not_faked() {
    let t_e1 = invariant::by_id("T-E1").expect("T-E1 is declared");

    assert_eq!(t_e1.source, InvariantSource::Paper { section: "4.2" });
    assert_eq!(t_e1.name, "no_direct_domain_imports");
    assert!(
        t_e1.check.is_none(),
        "T-E1 is enforced between crates; a runtime self-check here would assert nothing"
    );
    assert!(
        t_e1.verification.contains("BETWEEN crates"),
        "the declaration must say why it is not checked here: {}",
        t_e1.verification
    );
    assert!(
        t_e1.description.contains("never import each other"),
        "the statement must be the paper's: {}",
        t_e1.description
    );
    assert!(
        !invariant::checkable()
            .iter()
            .any(|declared| declared.id == "T-E1"),
        "T-E1 must not appear among the invariants this crate claims to check"
    );

    // The other two paper-owned identifiers are present and *are* checkable.
    for id in ["T-E2", "T-E3"] {
        let declared = invariant::by_id(id).expect("declared");
        assert_eq!(declared.source, InvariantSource::Paper { section: "4.2" });
        assert!(declared.check.is_some());
    }
}

#[test]
fn crate_local_invariants_do_not_use_a_governed_namespace() {
    for declared in invariant::ALL {
        assert!(
            !declared.id.starts_with("I6") && !declared.id.starts_with("I-"),
            "{} reuses a canonical namespace this crate does not own",
            declared.id
        );
        assert!(!declared.description.is_empty());
        assert!(!declared.verification.is_empty());
    }

    // The crate-local ones are labelled as such.
    assert_eq!(
        invariant::by_id("EB-01").expect("declared").source,
        InvariantSource::CrateLocal
    );
    assert_eq!(
        invariant::by_id("EB-02").expect("declared").source,
        InvariantSource::CrateLocal
    );
    assert!(invariant::by_id("EB-03").is_none());
}

// ---------------------------------------------------------------------------
// Delivery semantics (documented in `bus.rs`, asserted here)
// ---------------------------------------------------------------------------

#[test]
fn late_subscribers_get_no_history_and_the_log_is_how_they_catch_up() {
    let bus = EventBus::new(BusConfig::default());
    for n in 1..=3u64 {
        bus.publish(event(STORAGE_STORED, &format!("evt_{n}")))
            .expect("accepted");
    }

    // Subscribing after the fact receives nothing: `broadcast` has no history.
    let mut late = bus.subscribe(STORAGE_STORED);
    assert!(
        late.try_recv().expect("no lag").is_none(),
        "a late subscriber is not sent the past"
    );

    // The log is the answer, and the caller dedupes on `event_id`.
    let replayed = bus.replay(SequenceNumber::FIRST).expect("in range");
    assert_eq!(replayed.len(), 3);
    let ids: Vec<&str> = replayed
        .iter()
        .map(|event| event.event_id().as_str())
        .collect();
    assert_eq!(ids, ["evt_1", "evt_2", "evt_3"]);

    // From now on the subscriber is live.
    bus.publish(event(STORAGE_STORED, "evt_4")).expect("accepted");
    let live = late.try_recv().expect("no lag").expect("delivered");
    assert_eq!(live.event_id().as_str(), "evt_4");
    assert_eq!(live.sequence().get(), 4);
}

#[test]
fn a_subscription_to_a_topic_nobody_publishes_stays_quiet() {
    let bus = EventBus::new(BusConfig::default());
    let mut quiet = bus.subscribe(ATTESTATION_COMPLETED);

    for n in 1..=3u64 {
        bus.publish(event(STORAGE_STORED, &format!("other_{n}")))
            .expect("accepted");
    }

    assert!(quiet.try_recv().expect("no lag").is_none());
    assert_eq!(bus.subscriber_count(&ATTESTATION_COMPLETED), 1);
    assert_eq!(bus.subscriber_count(&STORAGE_STORED), 0);
    assert_eq!(bus.log_len(), 3);

    // Dropping the subscription unsubscribes.
    drop(quiet);
    assert_eq!(bus.subscriber_count(&ATTESTATION_COMPLETED), 0);
}

#[test]
fn dropping_the_bus_closes_its_subscriptions() {
    let bus = EventBus::new(BusConfig::default());
    let mut subscriber = bus.subscribe(STORAGE_STORED);

    bus.publish(event(STORAGE_STORED, "evt_before")).expect("accepted");
    assert!(subscriber.try_recv().expect("no lag").is_some());

    drop(bus);

    assert!(subscriber.is_closed(), "the bus is gone");
    assert_eq!(
        subscriber.try_recv(),
        Err(BusError::SubscriptionClosed),
        "absence of the bus is reported, not mistaken for absence of events"
    );
}

#[test]
fn delivery_carries_the_sequence_number_so_consumers_can_establish_order() {
    let bus = EventBus::new(BusConfig::default());
    let mut all = bus.subscribe_all();

    bus.publish(event(STORAGE_STORED, "evt_1")).expect("accepted");
    bus.publish(event(ATTESTATION_COMPLETED, "evt_2")).expect("accepted");
    bus.publish(event(STORAGE_STORED, "evt_3")).expect("accepted");

    let delivered = all.drain().expect("no lag");
    let sequences: Vec<u64> = delivered.iter().map(|d| d.sequence().get()).collect();
    assert_eq!(sequences, [1, 2, 3]);

    // The sequence on a delivery is the sequence in the log: a consumer can cite
    // it, checkpoint it, and ask for the rest.
    let checkpoint = bus.checkpoint_at(SequenceNumber::new(2)).expect("in range");
    assert_eq!(checkpoint.sequence().get(), 2);
    assert!(bus.verify_against(&checkpoint).is_ok());
    assert_eq!(
        bus.log().new_events_since(&checkpoint).expect("in range").len(),
        1
    );
}
