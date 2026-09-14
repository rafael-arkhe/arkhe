//! The crate's invariants, with the paper's identifiers where the paper owns
//! them.
//!
//! Three of these are **not this crate's to name**. `T-E1`, `T-E2` and `T-E3` are
//! the canonical paper's identifiers (DOI 10.5281/zenodo.21383201, §4.2), and
//! they are reproduced here with the paper's numbers, the paper's statements and
//! an honest account of what can and cannot be checked from inside this crate.
//! The repository's governed canonical ID spaces (`I6xx`, `I-xx`) are
//! deliberately untouched: this crate does not mint identifiers in a namespace it
//! does not own.
//!
//! | ID | Owner | Name | Statement |
//! |---|---|---|---|
//! | `T-E1` | paper §4.2 | `no_direct_domain_imports` | Domain crates never import each other; all domain-to-domain communication flows through `arkhe-event-bus` as typed pub/sub. A direct import fails compilation and blocks CI. |
//! | `T-E2` | paper §4.2 | `four_identifiers_on_every_event` | Every event carries `event_id`, `trace_id`, `causation_id` and `correlation_id`, and they survive publishing, delivery, logging and a JSON round trip. |
//! | `T-E3` | paper §4.2 | `unknown_schemas_never_panic` | Unknown future event schemas never panic a consumer: unknown topics, unknown payload members, and future schema versions yield a value or a typed error. |
//! | `EB-01` | this crate | `malformed_events_are_unpublishable` | §5.2's *"malformed events are unpublishable"*: a refused publish leaves the log, its root and every subscriber untouched. |
//! | `EB-02` | this crate | `replay_is_exact_and_tamper_is_visible` | Sequence numbers are monotonic from 1; replay reproduces the logged order exactly; the Merkle root is stable for the same log, moves when any event's bytes move, and checkpoint fencing refuses already-fenced history. |
//!
//! # `T-E1` cannot be checked here, and this module says so
//!
//! `T-E1` is a property *between crates*: it is enforced by the compiler plus CI,
//! by the absence of an import edge. Nothing running inside `arkhe-event-bus` can
//! observe whether `arkhe-attestation` imports `arkhe-storage`. Representing it
//! in this table with `check: None` and a [`BusInvariant::verification`] string
//! that names the mechanism is the honest option; a fabricated "self-test" would
//! be evidence of nothing. See `README.md`, "Not verified / out of scope".

use crate::bus::{BusConfig, EventBus};
use crate::error::{BusError, BusResult};
use crate::event::Event;
use crate::log::{merkle_root, EventLog, SequenceNumber};
use crate::meta::{CorrelationId, EventId, EventMeta, SchemaVersion, TraceId};
use crate::topic::{Topic, ATTESTATION_COMPLETED, STORAGE_STORED};

/// Who owns an invariant's identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvariantSource {
    /// The canonical paper owns the identifier and the statement.
    Paper {
        /// Section of the paper that states it.
        section: &'static str,
    },
    /// This crate owns the identifier.
    CrateLocal,
}

/// A declared invariant.
#[derive(Debug, Clone, Copy)]
pub struct BusInvariant {
    /// Stable identifier.
    pub id: &'static str,
    /// Short machine-friendly name.
    pub name: &'static str,
    /// What the invariant asserts.
    pub description: &'static str,
    /// Who owns the identifier.
    pub source: InvariantSource,
    /// How it is verified, or why it cannot be verified here.
    pub verification: &'static str,
    /// An executable check, when one is genuinely possible.
    pub check: Option<fn() -> BusResult<()>>,
}

/// `T-E1` — no direct domain-to-domain imports (paper §4.2, Table 1).
pub const T_E1: BusInvariant = BusInvariant {
    id: "T-E1",
    name: "no_direct_domain_imports",
    description: "domain crates never import each other; all domain-to-domain communication flows \
                  through arkhe-event-bus as typed pub/sub, and a direct import fails the build",
    source: InvariantSource::Paper { section: "4.2" },
    verification: "compiler + CI (workspace dependency graph). A property BETWEEN crates: not \
                   checkable from inside arkhe-event-bus, and not asserted here",
    check: None,
};

/// `T-E2` — the four identifiers on every event (paper §4.2).
pub const T_E2: BusInvariant = BusInvariant {
    id: "T-E2",
    name: "four_identifiers_on_every_event",
    description: "every event carries event_id, trace_id, causation_id and correlation_id, and \
                  they survive publishing, delivery, logging and a JSON round trip",
    source: InvariantSource::Paper { section: "4.2" },
    verification: "runtime check over a synthetic A -> B -> C chain (this module), plus the \
                   integration tests in tests/",
    check: Some(check_t_e2),
};

/// `T-E3` — unknown future schemas never panic a consumer (paper §4.2).
pub const T_E3: BusInvariant = BusInvariant {
    id: "T-E3",
    name: "unknown_schemas_never_panic",
    description: "unknown topics, unknown payload members and future schema versions never panic \
                  a consumer: each yields a value or a typed error",
    source: InvariantSource::Paper { section: "4.2" },
    verification: "runtime check over an unknown topic, an unknown payload member and a v2 event \
                   read by a v1 consumer (this module), plus the integration tests in tests/",
    check: Some(check_t_e3),
};

/// `EB-01` — malformed events are unpublishable (§5.2).
pub const EB_01: BusInvariant = BusInvariant {
    id: "EB-01",
    name: "malformed_events_are_unpublishable",
    description: "a refused publish leaves no trace: the log length, the Merkle root and every \
                  subscriber are exactly as they were",
    source: InvariantSource::CrateLocal,
    verification: "runtime check feeding malformed wire envelopes through Event::from_json and \
                   asserting the log and subscribers are untouched",
    check: Some(check_eb_01),
};

/// `EB-02` — replay is exact, tampering is visible.
pub const EB_02: BusInvariant = BusInvariant {
    id: "EB-02",
    name: "replay_is_exact_and_tamper_is_visible",
    description: "sequence numbers are monotonic from 1; replay reproduces the logged order \
                  exactly; the Merkle root is stable for the same log and moves when an event's \
                  bytes move; checkpoint fencing refuses already-fenced history",
    source: InvariantSource::CrateLocal,
    verification: "runtime check over a synthetic log, a recomputed root with one leaf replaced, \
                   and a checkpoint taken mid-log",
    check: Some(check_eb_02),
};

/// Every invariant this crate declares, in identifier order.
pub const ALL: [BusInvariant; 5] = [T_E1, T_E2, T_E3, EB_01, EB_02];

/// Look up an invariant by its identifier.
pub fn by_id(id: &str) -> Option<&'static BusInvariant> {
    ALL.iter().find(|invariant| invariant.id == id)
}

/// The invariants that have an executable check.
pub fn checkable() -> Vec<&'static BusInvariant> {
    ALL.iter()
        .filter(|invariant| invariant.check.is_some())
        .collect()
}

/// Run every check that exists. Invariants without one are omitted, not faked.
///
/// Failures are returned, not panicked on, so a caller can record them as
/// evidence and decide what to do.
pub fn check_all() -> Vec<(&'static BusInvariant, BusResult<()>)> {
    ALL.iter()
        .filter_map(|invariant| {
            invariant
                .check
                .map(|check| (invariant, check()))
        })
        .collect()
}

/// Build validated meta for the checks below.
fn sample_meta(
    topic: Topic,
    event_id: &str,
    trace_id: &str,
    correlation_id: &str,
) -> BusResult<EventMeta> {
    Ok(EventMeta::root(
        topic,
        SchemaVersion::V1,
        EventId::new(event_id)?,
        TraceId::new(trace_id)?,
        CorrelationId::new(correlation_id)?,
        1,
    ))
}

/// Fetch an event by position without indexing.
fn event_at(
    events: &[std::sync::Arc<Event>],
    index: usize,
    invariant: &str,
) -> BusResult<std::sync::Arc<Event>> {
    events.get(index).cloned().ok_or_else(|| {
        BusError::internal(format!(
            "{invariant}: replay returned {} events, expected at least {}",
            events.len(),
            index + 1
        ))
    })
}

/// `T-E2` — the four identifiers across a causal chain.
pub fn check_t_e2() -> BusResult<()> {
    let bus = EventBus::new(BusConfig::default());

    let first_meta = sample_meta(ATTESTATION_COMPLETED, "evt_a", "trc_chain", "cor_chain")?;
    let second_meta = first_meta.caused_by(EventId::new("evt_b")?, 2);
    let third_meta = second_meta.caused_by(EventId::new("evt_c")?, 3);

    bus.publish(Event::new(
        first_meta,
        serde_json::json!({ "stage": "requested" }),
    )?)?;
    bus.publish(Event::new(
        second_meta,
        serde_json::json!({ "stage": "verified" }),
    )?)?;
    bus.publish(Event::new(
        third_meta,
        serde_json::json!({ "stage": "settled" }),
    )?)?;

    let events = bus.replay(SequenceNumber::FIRST)?;

    let ids: Vec<&str> = events
        .iter()
        .map(|event| event.event_id().as_str())
        .collect();
    if ids != ["evt_a", "evt_b", "evt_c"] {
        return Err(BusError::internal(format!(
            "T-E2: replay returned {ids:?}, expected the published order"
        )));
    }

    let first = event_at(&events, 0, "T-E2")?;
    let second = event_at(&events, 1, "T-E2")?;
    let third = event_at(&events, 2, "T-E2")?;

    if first.causation_id().is_some() {
        return Err(BusError::internal(
            "T-E2: the chain root must carry causation_id = None",
        ));
    }
    if second.causation_id().map(EventId::as_str) != Some("evt_a") {
        return Err(BusError::internal(
            "T-E2: the second event's causation_id must point at the first",
        ));
    }
    if third.causation_id().map(EventId::as_str) != Some("evt_b") {
        return Err(BusError::internal(
            "T-E2: the third event's causation_id must point at the second",
        ));
    }

    for event in &events {
        if event.trace_id().as_str() != "trc_chain" {
            return Err(BusError::internal(format!(
                "T-E2: trace_id drifted to {} mid-chain",
                event.trace_id()
            )));
        }
        if event.correlation_id().as_str() != "cor_chain" {
            return Err(BusError::internal(format!(
                "T-E2: correlation_id drifted to {} mid-chain",
                event.correlation_id()
            )));
        }
    }

    // And the identifiers survive the wire form the bus logs.
    for event in &events {
        let round_tripped = Event::from_json(event.to_json()?.as_bytes())?;
        if round_tripped.meta() != event.meta() {
            return Err(BusError::internal(
                "T-E2: meta did not survive a JSON round trip",
            ));
        }
        if round_tripped.digest() != event.digest() {
            return Err(BusError::internal(
                "T-E2: digest did not survive a JSON round trip",
            ));
        }
    }

    Ok(())
}

/// `T-E3` — unknown topics, unknown members, unknown future major versions.
pub fn check_t_e3() -> BusResult<()> {
    let bus = EventBus::new(BusConfig::default());

    // 1. An unknown topic: accepted by the parser, subscribable, publishable.
    let unknown_topic = Topic::parse("arkhe_physics.experiment.published")?;
    if unknown_topic.is_canonical() {
        return Err(BusError::internal(
            "T-E3: the probe topic must not be one of the canonical nine",
        ));
    }

    let mut subscriber = bus.subscribe(unknown_topic.clone());
    let meta = sample_meta(unknown_topic, "evt_unknown_topic", "trc_e3", "cor_e3")?;
    bus.publish(Event::new(
        meta,
        serde_json::json!({ "member_from_the_future": { "nested": [1, 2, 3] } }),
    )?)?;

    let delivery = match subscriber.try_recv()? {
        Some(delivery) => delivery,
        None => {
            return Err(BusError::internal(
                "T-E3: an event on an unknown topic was not delivered",
            ))
        }
    };
    if delivery.event().payload().get("member_from_the_future").is_none() {
        return Err(BusError::internal(
            "T-E3: an unknown payload member was dropped",
        ));
    }

    // 2. An unknown future major schema: readable? No — and refusal is typed,
    //    naming the sequence, so the caller can replay it instead of losing it.
    //    (Subscribe first: broadcast delivers from subscription time forward.)
    let future = SchemaVersion::new(2, 0)?;
    let future_meta = EventMeta {
        schema_version: future,
        ..sample_meta(ATTESTATION_COMPLETED, "evt_future_schema", "trc_e3", "cor_e3")?
    };

    let mut consumer = bus.subscribe(ATTESTATION_COMPLETED);
    let sequence = bus.publish(Event::new(
        future_meta,
        serde_json::json!({ "unknown": true }),
    )?)?;

    let delivery = match consumer.try_recv()? {
        Some(delivery) => delivery,
        None => return Err(BusError::internal("T-E3: the v2 event was not delivered")),
    };

    if delivery.is_readable_by(SchemaVersion::V1) {
        return Err(BusError::internal(
            "T-E3: a v2 event must not be readable by a v1 consumer",
        ));
    }

    // The same predicate `Subscription::recv_understanding` turns into this
    // error; here we only assert that the refusal is typed and locatable.
    let refusal = BusError::UnsupportedSchemaVersion {
        sequence: delivery.sequence().get(),
        event_major: delivery.schema_version().major(),
        consumer_major: SchemaVersion::V1.major(),
    };
    if refusal.to_string().is_empty() || delivery.sequence() != sequence {
        return Err(BusError::internal(
            "T-E3: the schema refusal must be typed and name the sequence",
        ));
    }

    // 3. Nothing above panicked, and the event is still fully auditable.
    let replayed = bus.replay(sequence)?;
    let replayed = match replayed.first() {
        Some(event) => event,
        None => {
            return Err(BusError::internal(
                "T-E3: a refused-by-the-consumer event must stay in the log",
            ))
        }
    };
    if replayed.schema_version() != future {
        return Err(BusError::internal(
            "T-E3: replay lost the future schema version",
        ));
    }

    Ok(())
}

/// `EB-01` — malformed events are unpublishable, and leave no trace.
pub fn check_eb_01() -> BusResult<()> {
    let bus = EventBus::new(BusConfig::default());
    let mut subscriber = bus.subscribe(STORAGE_STORED);

    let malformed: [&[u8]; 3] = [
        // No `meta` member at all — §5.2 makes it mandatory.
        br#"{"payload":{}}"#,
        // Empty event_id.
        br#"{"meta":{"event_id":"","trace_id":"trc","correlation_id":"cor","schema_version":{"major":1,"minor":0},"topic":"alerts","timestamp_unix_us":1},"payload":{}}"#,
        // `major == 0` means "no schema declared", which §5.2 forbids.
        br#"{"meta":{"event_id":"evt","trace_id":"trc","correlation_id":"cor","schema_version":{"major":0,"minor":0},"topic":"alerts","timestamp_unix_us":1},"payload":{}}"#,
    ];

    for bytes in malformed {
        let error = match Event::from_json(bytes) {
            Err(error) => error,
            // Unreachable today (the three inputs fail above), handled anyway so
            // that the check cannot be defeated by a constructor change.
            Ok(event) => match bus.publish(event) {
                Err(error) => error,
                Ok(sequence) => {
                    return Err(BusError::internal(format!(
                        "EB-01: malformed input was published at sequence {sequence}"
                    )))
                }
            },
        };

        if !error.is_rejection() {
            return Err(BusError::internal(format!(
                "EB-01: malformed input must be a typed rejection, got {error}"
            )));
        }
    }

    if bus.log_len() != 0 {
        return Err(BusError::internal(format!(
            "EB-01: {} event(s) entered the log from malformed input",
            bus.log_len()
        )));
    }

    if bus.current_root() != crate::log::MerkleRoot::empty() {
        return Err(BusError::internal(
            "EB-01: the Merkle root moved without an accepted event",
        ));
    }

    if subscriber.try_recv()?.is_some() {
        return Err(BusError::internal(
            "EB-01: a malformed event was delivered to a subscriber",
        ));
    }

    Ok(())
}

/// `EB-02` — monotonic sequences, exact replay, tamper-visible root, fencing.
pub fn check_eb_02() -> BusResult<()> {
    let bus = EventBus::new(BusConfig::default());

    for n in 1..=3u64 {
        let meta = sample_meta(
            STORAGE_STORED,
            &format!("evt_{n}"),
            "trc_replay",
            "cor_replay",
        )?;
        let sequence = bus.publish(Event::new(meta, serde_json::json!({ "n": n }))?)?;
        if sequence.get() != n {
            return Err(BusError::internal(format!(
                "EB-02: publish {n} returned sequence {}",
                sequence.get()
            )));
        }
    }

    let log = bus.log();
    let sequences: Vec<u64> = log.entries().iter().map(|e| e.sequence().get()).collect();
    if sequences != [1, 2, 3] {
        return Err(BusError::internal(format!(
            "EB-02: sequence numbers are {sequences:?}, expected [1, 2, 3]"
        )));
    }

    let replayed = bus.replay(SequenceNumber::FIRST)?;
    let replayed_ids: Vec<&str> = replayed
        .iter()
        .map(|event| event.event_id().as_str())
        .collect();
    if replayed_ids != ["evt_1", "evt_2", "evt_3"] {
        return Err(BusError::internal(format!(
            "EB-02: replay returned {replayed_ids:?}"
        )));
    }

    if log.verify_integrity().is_err() {
        return Err(BusError::internal("EB-02: log integrity did not verify"));
    }

    let rebuilt = EventLog::restore(replayed.clone())?;
    if rebuilt.current_root() != log.current_root() {
        return Err(BusError::internal(format!(
            "EB-02: the same log produced two roots: {} and {}",
            rebuilt.current_root(),
            log.current_root()
        )));
    }

    let mut leaves: Vec<[u8; 32]> = log.entries().iter().map(|e| e.digest()).collect();
    if leaves.len() != 3 {
        return Err(BusError::internal("EB-02: expected three leaves"));
    }
    if let Some(first) = leaves.first_mut() {
        *first = [0u8; 32];
    }
    if merkle_root(&leaves) == log.current_root() {
        return Err(BusError::internal(
            "EB-02: the Merkle root did not move when a leaf changed",
        ));
    }

    let checkpoint = bus.checkpoint_at(SequenceNumber::new(2))?;
    bus.verify_against(&checkpoint)?;

    match log.admits_as_new(SequenceNumber::new(1), &checkpoint) {
        Ok(()) => {
            return Err(BusError::internal(
                "EB-02: already-fenced history was accepted as new",
            ))
        }
        Err(BusError::FencedSequence { seq, fence }) => {
            if (seq, fence) != (1, 2) {
                return Err(BusError::internal(format!(
                    "EB-02: fence refused {seq} against {fence}, expected (1, 2)"
                )));
            }
        }
        Err(other) => {
            return Err(BusError::internal(format!(
                "EB-02: unexpected fencing error {other}"
            )))
        }
    }

    if log.admits_as_new(SequenceNumber::new(3), &checkpoint).is_err() {
        return Err(BusError::internal(
            "EB-02: sequence 3 is beyond the fence and must be admitted",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_the_papers_and_this_crates_own() {
        assert_eq!(ALL.len(), 5);

        // The three paper-owned identifiers, verbatim.
        assert_eq!(T_E2.id, "T-E2");
        assert_eq!(T_E3.id, "T-E3");
        assert_eq!(T_E1.id, "T-E1");
        assert_eq!(T_E1.source, InvariantSource::Paper { section: "4.2" });
        assert_eq!(T_E2.source, InvariantSource::Paper { section: "4.2" });
        assert_eq!(T_E3.source, InvariantSource::Paper { section: "4.2" });

        // Crate-local identifiers use this crate's own prefix, never a governed
        // namespace.
        assert_eq!(EB_01.source, InvariantSource::CrateLocal);
        assert_eq!(EB_02.source, InvariantSource::CrateLocal);

        for invariant in ALL {
            assert!(!invariant.description.is_empty());
            assert!(!invariant.name.is_empty());
            assert!(!invariant.verification.is_empty());
            assert!(
                !invariant.id.starts_with("I6") && !invariant.id.starts_with("I-"),
                "{} reuses a governed canonical identifier namespace",
                invariant.id
            );
        }

        assert_eq!(by_id("EB-01").map(|i| i.name), Some("malformed_events_are_unpublishable"));
        assert!(by_id("EB-99").is_none());
    }

    #[test]
    fn t_e1_is_declared_as_not_runtime_checkable() {
        assert!(
            T_E1.check.is_none(),
            "T-E1 is a property between crates: a runtime self-check would be fabricated evidence"
        );
        assert!(T_E1.verification.contains("BETWEEN crates"));
        assert!(T_E1.description.contains("never import each other"));
        assert!(!checkable().iter().any(|invariant| invariant.id == "T-E1"));
    }

    #[test]
    fn every_check_passes() {
        let results = check_all();
        assert_eq!(results.len(), 4, "four of the five invariants have a check");

        for (invariant, result) in results {
            if let Err(error) = result {
                panic!("{} ({}) failed: {error}", invariant.id, invariant.name);
            }
        }
    }
}
