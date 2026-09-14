//! Auditability: replay, Merkle roots and checkpoint fencing — **T-13** from
//! outside the crate.
//!
//! §6.2 of the canonical paper (DOI 10.5281/zenodo.21383201) anchors T-13 —
//! *"replay against the event log"* — on *"monotonic sequence numbers, Merkle
//! roots, signed events, checkpoint fencing"*, and §4.2 promises that *"any flow
//! can be replayed or audited from the event stream alone"*.
//!
//! This file tests the three that belong to the log. Signed events are covered
//! in `signature.rs`.
//!
//! # The claim under test
//!
//! Given nothing but the event stream — the JSON of each event, in order — an
//! auditor must be able to rebuild the log and arrive at the same Merkle root the
//! bus reported. And given a checkpoint, an auditor must be able to detect that
//! the history behind it was rewritten.

use std::sync::Arc;

use serde_json::json;

use arkhe_event_bus::bus::{BusConfig, EventBus};
use arkhe_event_bus::error::BusError;
use arkhe_event_bus::event::Event;
use arkhe_event_bus::log::{merkle_root, EventLog, MerkleRoot, SequenceNumber};
use arkhe_event_bus::meta::{CorrelationId, EventId, EventMeta, SchemaVersion, TraceId};
use arkhe_event_bus::topic::{Topic, STORAGE_STORED};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn event(event_id: &str, payload: serde_json::Value) -> Event {
    let meta = EventMeta::root(
        STORAGE_STORED,
        SchemaVersion::V1,
        EventId::new(event_id).expect("valid"),
        TraceId::new("trc_audit").expect("valid"),
        CorrelationId::new("cor_audit").expect("valid"),
        1,
    );
    Event::new(meta, payload).expect("valid event")
}

fn bus_with(count: u64) -> EventBus {
    let bus = EventBus::new(BusConfig::default());
    for n in 1..=count {
        bus.publish(event(&format!("evt_{n}"), json!({ "n": n })))
            .expect("accepted");
    }
    bus
}

// ---------------------------------------------------------------------------
// Monotonic sequence numbers
// ---------------------------------------------------------------------------

#[test]
fn sequence_numbers_are_monotonic_from_one_and_replay_is_exact() {
    let bus = bus_with(5);
    let log = bus.log();

    let sequences: Vec<u64> = log.entries().iter().map(|e| e.sequence().get()).collect();
    assert_eq!(sequences, [1, 2, 3, 4, 5], "strictly increasing from 1");
    assert_eq!(log.head(), Some(SequenceNumber::new(5)));
    assert_eq!(log.next_sequence().get(), 6);
    assert!(log.verify_integrity().is_ok());

    // Replay from an arbitrary point reproduces exactly the suffix, in order.
    let from_three = bus.replay(SequenceNumber::new(3)).expect("in range");
    let ids: Vec<&str> = from_three
        .iter()
        .map(|event| event.event_id().as_str())
        .collect();
    assert_eq!(ids, ["evt_3", "evt_4", "evt_5"], "inclusive from seq 3");

    // Genesis and FIRST are the same question.
    let all = bus.replay(SequenceNumber::FIRST).expect("in range");
    assert_eq!(all.len(), 5);
    assert_eq!(
        bus.replay(SequenceNumber::GENESIS).expect("in range").len(),
        all.len()
    );

    // Beyond the head is an error, not silence.
    match bus.replay(SequenceNumber::new(7)) {
        Err(BusError::ReplayOutOfRange { from_seq, head }) => assert_eq!((from_seq, head), (7, 5)),
        other => panic!("expected ReplayOutOfRange, got {other:?}"),
    }
}

#[test]
fn sequence_numbers_stay_unique_and_monotonic_under_concurrent_publishers() {
    const WORKERS: u32 = 8;
    const PER_WORKER: u32 = 25;

    let bus = Arc::new(EventBus::new(BusConfig::default()));
    let mut handles = Vec::new();

    for worker in 0..WORKERS {
        let bus = Arc::clone(&bus);
        handles.push(std::thread::spawn(move || {
            let mut sequences = Vec::with_capacity(PER_WORKER as usize);
            for n in 0..PER_WORKER {
                let meta = EventMeta::root(
                    STORAGE_STORED,
                    SchemaVersion::V1,
                    EventId::new(format!("evt_{worker}_{n}")).expect("valid"),
                    TraceId::new("trc_race").expect("valid"),
                    CorrelationId::new("cor_race").expect("valid"),
                    1,
                );
                let event = Event::new(meta, json!({ "worker": worker, "n": n }))
                    .expect("valid event");
                sequences.push(bus.publish(event).expect("accepted").get());
            }
            sequences
        }));
    }

    let mut assigned = Vec::new();
    for handle in handles {
        let mut sequences = handle.join().expect("no worker panicked");
        assigned.append(&mut sequences);
    }

    // Every sequence from 1..=N was handed out exactly once.
    assigned.sort_unstable();
    let expected: Vec<u64> = (1..=u64::from(WORKERS * PER_WORKER)).collect();
    assert_eq!(assigned, expected, "sequences must be unique and complete");

    // And the log itself is contiguous and monotonic.
    let log = bus.log();
    let logged: Vec<u64> = log.entries().iter().map(|e| e.sequence().get()).collect();
    assert_eq!(logged, expected, "the log must be strictly ordered");
    assert_eq!(log.len(), (WORKERS * PER_WORKER) as usize);
    assert!(log.verify_integrity().is_ok());
    assert_eq!(log.events().len(), logged.len());
}

// ---------------------------------------------------------------------------
// The Merkle root: stable for the same log, moved by tampering
// ---------------------------------------------------------------------------

#[test]
fn the_merkle_root_is_stable_for_the_same_log() {
    let first = bus_with(6).log().current_root();
    let second = bus_with(6).log().current_root();
    assert_eq!(
        first, second,
        "the same sequence of events must produce the same root"
    );

    // Rebuilding from the events alone — the auditor's path — agrees.
    let bus = bus_with(6);
    let log = bus.log();
    let rebuilt = EventLog::restore(log.events()).expect("restore");
    assert_eq!(rebuilt.current_root(), log.current_root());
    assert_eq!(rebuilt.entries(), log.entries());
}

#[test]
fn the_merkle_root_moves_when_any_event_changes() {
    let original = bus_with(4).log();
    let leaves: Vec<[u8; 32]> = original.entries().iter().map(|e| e.digest()).collect();
    assert_eq!(merkle_root(&leaves), original.current_root());

    // One byte of one payload is enough.
    let tampered = Arc::new(event("evt_2", json!({ "n": 999 })));
    let rebuilt = EventLog::restore(vec![
        original.entries()[0].event().clone(),
        tampered,
        original.entries()[2].event().clone(),
        original.entries()[3].event().clone(),
    ])
    .expect("restore");

    assert_ne!(
        rebuilt.current_root(),
        original.current_root(),
        "changing an event must move the root"
    );
    assert_ne!(rebuilt.entries()[1].digest(), original.entries()[1].digest());
}

#[test]
fn the_merkle_root_moves_when_events_are_reordered_or_truncated() {
    let original = bus_with(4).log();

    // Reordered: the same four events, two of them swapped. Positions matter.
    let entries = original.entries();
    let reordered = EventLog::restore(vec![
        entries[1].event().clone(),
        entries[0].event().clone(),
        entries[2].event().clone(),
        entries[3].event().clone(),
    ])
    .expect("restore");
    assert_ne!(reordered.current_root(), original.current_root());

    // Truncated.
    let truncated = EventLog::restore(vec![
        entries[0].event().clone(),
        entries[1].event().clone(),
        entries[2].event().clone(),
    ])
    .expect("restore");
    assert_ne!(truncated.current_root(), original.current_root());

    // And every prefix has its own root, so a prefix cannot masquerade as the
    // whole log.
    let mut roots = Vec::new();
    for n in 0..=4u64 {
        roots.push(original.root_at(SequenceNumber::new(n)).expect("in range"));
    }
    for (i, root) in roots.iter().enumerate() {
        for (j, other) in roots.iter().enumerate() {
            if i != j {
                assert_ne!(root, other, "prefix roots {i} and {j} collided");
            }
        }
    }
    assert_eq!(roots[0], MerkleRoot::empty());
}

#[test]
fn the_whole_flow_is_reconstructible_from_the_event_stream_alone() {
    // §4.2: "any flow can be replayed or audited from the event stream alone".
    // Everything below uses only the bytes of the stream — no log object, no bus.
    let bus = bus_with(4);
    let reported_root = bus.current_root();

    let stream: Vec<String> = bus
        .log()
        .events()
        .iter()
        .map(|event| event.to_json().expect("serializes"))
        .collect();

    // An auditor parses the stream and recomputes the root.
    let mut parsed = Vec::new();
    for (index, json) in stream.iter().enumerate() {
        let event = Event::from_json(json.as_bytes()).expect("parses");
        assert_eq!(
            event.event_id().as_str(),
            format!("evt_{}", index + 1),
            "the stream preserves order"
        );
        parsed.push(Arc::new(event));
    }

    let rebuilt = EventLog::restore(parsed).expect("restore");
    assert_eq!(
        rebuilt.current_root(),
        reported_root,
        "an auditor with the stream alone must reach the same root"
    );
    assert_eq!(rebuilt.len(), 4);
}

// ---------------------------------------------------------------------------
// Checkpoint fencing
// ---------------------------------------------------------------------------

#[test]
fn a_checkpoint_makes_a_rewritten_prefix_detectable() {
    let bus = bus_with(5);
    let checkpoint = bus
        .checkpoint_at(SequenceNumber::new(3))
        .expect("in range");
    assert_eq!(checkpoint.sequence().get(), 3);

    // The honest log agrees.
    assert!(bus.verify_against(&checkpoint).is_ok());

    // A stream that rewrote one fenced event is refused, and the error says
    // which sequence and which roots disagree.
    let log = bus.log();
    let rewritten = EventLog::restore(vec![
        log.entries()[0].event().clone(),
        log.entries()[1].event().clone(),
        Arc::new(event("evt_3", json!({ "n": "rewritten" }))),
        log.entries()[3].event().clone(),
        log.entries()[4].event().clone(),
    ])
    .expect("restore");

    match rewritten.verify_against(&checkpoint) {
        Err(BusError::CheckpointMismatch {
            checkpoint_seq,
            expected,
            observed,
        }) => {
            assert_eq!(checkpoint_seq, 3);
            assert_eq!(expected, checkpoint.root().to_hex());
            assert_ne!(expected, observed);
        }
        other => panic!("expected CheckpointMismatch, got {other:?}"),
    }

    // A stream that does not even reach the fence is refused too: it says
    // nothing about the fenced range.
    let short = EventLog::restore(vec![log.entries()[0].event().clone()]).expect("restore");
    match short.verify_against(&checkpoint) {
        Err(BusError::ReplayOutOfRange { from_seq, head }) => assert_eq!((from_seq, head), (3, 1)),
        other => panic!("expected ReplayOutOfRange, got {other:?}"),
    }
}

#[test]
fn fencing_refuses_history_at_or_below_the_checkpoint() {
    let bus = bus_with(5);
    let log = bus.log();
    let checkpoint = log
        .checkpoint_at(SequenceNumber::new(3))
        .expect("in range");

    // Already-fenced sequence numbers are never new.
    for fenced in 1..=3u64 {
        match log.admits_as_new(SequenceNumber::new(fenced), &checkpoint) {
            Err(BusError::FencedSequence { seq, fence }) => {
                assert_eq!(seq, fenced);
                assert_eq!(fence, 3);
            }
            other => panic!("sequence {fenced} must be fenced, got {other:?}"),
        }
    }

    // What lies beyond the fence is new, and only that.
    let genuinely_new = log.new_events_since(&checkpoint).expect("in range");
    let ids: Vec<&str> = genuinely_new
        .iter()
        .map(|event| event.event_id().as_str())
        .collect();
    assert_eq!(ids, ["evt_4", "evt_5"]);
    for sequence in 4..=5u64 {
        assert!(log
            .admits_as_new(SequenceNumber::new(sequence), &checkpoint)
            .is_ok());
    }

    // A checkpoint at the head fences everything recorded so far.
    let at_head = log.checkpoint();
    assert!(log.new_events_since(&at_head).expect("in range").is_empty());
    assert!(log
        .admits_as_new(SequenceNumber::new(5), &at_head)
        .is_err());
}

#[test]
fn a_checkpoint_is_self_contained_and_reusable() {
    // A checkpoint is two values — a sequence and a root — so it can be
    // published, stored and compared by a party that has no access to the bus.
    let bus = bus_with(3);
    let checkpoint = bus.checkpoint();
    let sequence = checkpoint.sequence().get();
    let root_hex = checkpoint.root().to_hex();
    assert_eq!(sequence, 3);
    assert_eq!(root_hex.len(), 64);

    // Reconstructed from the raw values, it still verifies a log.
    let reconstructed = arkhe_event_bus::log::Checkpoint::new(
        SequenceNumber::new(sequence),
        MerkleRoot::from_hex(&root_hex).expect("valid hex"),
    );
    assert!(bus.verify_against(&reconstructed).is_ok());
    assert_eq!(reconstructed, checkpoint);

    // And a malformed root is refused rather than accepted.
    assert!(MerkleRoot::from_hex("not-hex").is_err());
    assert!(MerkleRoot::from_hex("00").is_err());
}

#[test]
fn a_genesis_checkpoint_only_matches_an_empty_log() {
    let empty = EventBus::new(BusConfig::default());
    let genesis = empty.checkpoint();
    assert!(genesis.is_genesis());
    assert!(empty.verify_against(&genesis).is_ok());

    let non_empty = bus_with(1);
    match non_empty.verify_against(&genesis) {
        Err(BusError::CheckpointMismatch { checkpoint_seq, .. }) => assert_eq!(checkpoint_seq, 0),
        other => panic!("expected CheckpointMismatch, got {other:?}"),
    }
}

#[test]
fn topic_types_are_usable_as_log_keys() {
    // The log is topic-agnostic; this checks that a consumer can filter a
    // replayed stream by topic without the bus having to know the topic set.
    let bus = EventBus::new(BusConfig::default());
    let other = Topic::parse("arkhe_physics.experiment.published").expect("parses");

    bus.publish(event("evt_storage_1", json!({}))).expect("accepted");
    let future_meta = EventMeta::root(
        other.clone(),
        SchemaVersion::V1,
        EventId::new("evt_future_1").expect("valid"),
        TraceId::new("trc_audit").expect("valid"),
        CorrelationId::new("cor_audit").expect("valid"),
        1,
    );
    bus.publish(Event::new(future_meta, json!({})).expect("valid"))
        .expect("accepted");

    let replayed = bus.replay(SequenceNumber::FIRST).expect("in range");
    let storage: Vec<&Event> = replayed
        .iter()
        .map(|event| event.as_ref())
        .filter(|event| event.topic() == &STORAGE_STORED)
        .collect();
    let physics: Vec<&Event> = replayed
        .iter()
        .map(|event| event.as_ref())
        .filter(|event| event.topic() == &other)
        .collect();

    assert_eq!(storage.len(), 1);
    assert_eq!(physics.len(), 1);
    assert_eq!(storage[0].event_id().as_str(), "evt_storage_1");
    assert_eq!(physics[0].event_id().as_str(), "evt_future_1");
}
