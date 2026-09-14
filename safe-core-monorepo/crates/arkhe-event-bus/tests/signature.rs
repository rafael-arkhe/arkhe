//! Per-event signatures verified by the bus — **T-12** and the *"signed events"*
//! term of **T-13** — asserted from outside the crate.
//!
//! §6.2 of the canonical paper (DOI 10.5281/zenodo.21383201):
//!
//! > **T-12**, man-in-the-middle between crates (mTLS with 24-hour certificates
//! > plus per-event Ed25519 signatures verified by the bus).
//!
//! # What the stand-in verifier does and does not prove
//!
//! `arkhe-event-bus` ships **no** cryptographic backend and holds **no** key
//! material (see the crate's `README.md`). The verifier used here is
//! [`DigestEqualityVerifier`]: it accepts a signature that equals the BLAKE3
//! digest of the message. That is **not** a signature scheme and is forgeable by
//! anyone who can read the message.
//!
//! It is nevertheless the right instrument for these tests, because what is under
//! test is the *bus*, not the cryptography:
//!
//! * that the bus refuses to accept anything when it cannot verify;
//! * that it hands the verifier exactly the canonical bytes a publisher signed;
//! * that those bytes cover both the meta and the payload, and are stable across
//!   a JSON round trip.
//!
//! Ed25519 verification itself is out of scope here and is listed as such in the
//! README.

use std::sync::{Arc, Mutex};

use serde_json::json;

use arkhe_event_bus::bus::{BusConfig, EventBus};
use arkhe_event_bus::error::BusError;
use arkhe_event_bus::event::Event;
use arkhe_event_bus::meta::{CorrelationId, EventId, EventMeta, SchemaVersion, TraceId};
use arkhe_event_bus::signature::{EventSignature, SignaturePolicy, SignatureVerifier};
use arkhe_event_bus::topic::STORAGE_STORED;

// ---------------------------------------------------------------------------
// The stand-in verifier (plumbing, not cryptography)
// ---------------------------------------------------------------------------

/// Accepts a signature equal to `BLAKE3(message)`. **Not a signature scheme.**
struct DigestEqualityVerifier;

impl SignatureVerifier for DigestEqualityVerifier {
    fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        signature == blake3::hash(message).as_bytes().as_slice()
    }
}

/// Records every message it is asked about, and accepts everything.
#[derive(Default)]
struct RecordingVerifier {
    seen: Mutex<Vec<Vec<u8>>>,
}

impl RecordingVerifier {
    fn messages(&self) -> Vec<Vec<u8>> {
        self.seen.lock().expect("not poisoned").clone()
    }
}

impl SignatureVerifier for RecordingVerifier {
    fn verify(&self, message: &[u8], _signature: &[u8]) -> bool {
        self.seen.lock().expect("not poisoned").push(message.to_vec());
        true
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn meta(event_id: &str, timestamp_unix_us: u64) -> EventMeta {
    EventMeta::root(
        STORAGE_STORED,
        SchemaVersion::V1,
        EventId::new(event_id).expect("valid"),
        TraceId::new("trc_sig").expect("valid"),
        CorrelationId::new("cor_sig").expect("valid"),
        timestamp_unix_us,
    )
}

fn event(event_id: &str, payload: serde_json::Value) -> Event {
    Event::new(meta(event_id, 1), payload).expect("valid event")
}

/// Sign the way [`DigestEqualityVerifier`] expects. Not cryptography.
fn sign(event: &Event) -> EventSignature {
    EventSignature::from_bytes(blake3::hash(event.signing_bytes()).as_bytes().to_vec())
        .expect("32-byte signature")
}

fn required_bus(verifier: Arc<dyn SignatureVerifier>) -> EventBus {
    EventBus::with_verifier(
        BusConfig::default().with_signature_policy(SignaturePolicy::Required),
        verifier,
    )
}

// ---------------------------------------------------------------------------
// Fail-closed: no verifier, no acceptance
// ---------------------------------------------------------------------------

#[test]
fn required_without_a_verifier_refuses_every_publish() {
    let bus = EventBus::new(
        BusConfig::default().with_signature_policy(SignaturePolicy::Required),
    );
    let mut subscriber = bus.subscribe(STORAGE_STORED);

    assert!(!bus.has_signature_verifier());
    assert_eq!(bus.signature_policy(), SignaturePolicy::Required);

    // Unsigned…
    let unsigned = event("evt_unsigned", json!({ "k": 1 }));
    assert_eq!(
        bus.publish(unsigned),
        Err(BusError::SignatureVerifierUnavailable)
    );

    // …or signed: the bus cannot check it, so it does not accept it.
    let signed = event("evt_signed", json!({ "k": 1 })).with_signature(
        EventSignature::from_bytes(vec![7u8; 64]).expect("valid signature"),
    );
    assert_eq!(
        bus.publish(signed),
        Err(BusError::SignatureVerifierUnavailable)
    );

    assert!(BusError::SignatureVerifierUnavailable.is_rejection());
    assert_eq!(bus.log_len(), 0, "nothing entered the log");
    assert_eq!(bus.current_root(), arkhe_event_bus::log::MerkleRoot::empty());
    assert!(subscriber.try_recv().expect("no lag").is_none());
}

#[test]
fn optional_without_a_verifier_accepts_unsigned_but_refuses_the_unverifiable() {
    let bus = EventBus::new(BusConfig::default());
    assert_eq!(bus.signature_policy(), SignaturePolicy::Optional);

    // Plain, unsigned traffic is the Phase 1 happy path.
    bus.publish(event("evt_plain", json!({ "k": 1 })))
        .expect("unsigned events are accepted under Optional");

    // A signature that cannot be verified is never taken on faith.
    let unverifiable = event("evt_unverifiable", json!({ "k": 1 }))
        .with_signature(EventSignature::from_bytes(vec![1u8; 64]).expect("valid"));
    assert_eq!(
        bus.publish(unverifiable),
        Err(BusError::SignatureVerifierUnavailable)
    );

    assert_eq!(bus.log_len(), 1);
}

#[test]
fn required_refuses_missing_signatures() {
    let bus = required_bus(Arc::new(DigestEqualityVerifier));

    assert_eq!(
        bus.publish(event("evt_missing", json!({ "k": 1 }))),
        Err(BusError::MissingSignature {
            event_id: String::from("evt_missing"),
        })
    );
    assert_eq!(bus.log_len(), 0);
}

// ---------------------------------------------------------------------------
// Fail-closed: a signature that does not match is refused
// ---------------------------------------------------------------------------

#[test]
fn a_valid_signature_is_accepted_and_an_invalid_one_is_not() {
    let bus = required_bus(Arc::new(DigestEqualityVerifier));
    let mut subscriber = bus.subscribe(STORAGE_STORED);

    let legitimate = event("evt_legit", json!({ "amount": 10 }));
    let signature = sign(&legitimate);
    let sequence = bus
        .publish(legitimate.clone().with_signature(signature.clone()))
        .expect("a signature over the exact bytes is accepted");
    assert_eq!(sequence.get(), 1);

    let delivery = subscriber.try_recv().expect("no lag").expect("delivered");
    assert!(delivery.event().is_signed());

    // The same signature over a *different* payload is refused — and refused as
    // a signature failure, not as anything else.
    let tampered_payload = Event::new(meta("evt_tampered", 1), json!({ "amount": 1_000_000 }))
        .expect("valid")
        .with_signature(signature.clone());
    assert_eq!(
        bus.publish(tampered_payload),
        Err(BusError::InvalidSignature {
            event_id: String::from("evt_tampered"),
        })
    );

    // The same signature over different *meta* is refused too: the signature
    // covers the metadata, not only the payload. `event_id` is unchanged here,
    // which also shows the signature gate runs before the uniqueness gate.
    let tampered_meta = Event::new(meta("evt_legit", 999), json!({ "amount": 10 }))
        .expect("valid")
        .with_signature(signature.clone());
    assert_eq!(
        bus.publish(tampered_meta),
        Err(BusError::InvalidSignature {
            event_id: String::from("evt_legit"),
        })
    );

    // A malformed-length signature cannot even be constructed.
    assert!(EventSignature::from_bytes(Vec::new()).is_err());

    // Two events were refused; one is in the log, and it is the one that arrived.
    assert_eq!(bus.log_len(), 1);
    assert_eq!(subscriber.drain().expect("no lag").len(), 0);
    let logged = bus.log();
    assert_eq!(logged.entries()[0].event_id().as_str(), "evt_legit");
}

#[test]
fn a_signature_over_the_wrong_bytes_is_refused() {
    let bus = required_bus(Arc::new(DigestEqualityVerifier));

    // A signature over some other message (a plausible mistake: signing the
    // payload alone, or the JSON envelope, rather than the canonical bytes).
    let payload_only = blake3::hash(b"{ \"amount\": 10 }");
    let signature =
        EventSignature::from_bytes(payload_only.as_bytes().to_vec()).expect("valid signature");

    let event = event("evt_payload_only", json!({ "amount": 10 })).with_signature(signature);
    assert_eq!(
        bus.publish(event),
        Err(BusError::InvalidSignature {
            event_id: String::from("evt_payload_only"),
        })
    );
    assert_eq!(bus.log_len(), 0);
}

// ---------------------------------------------------------------------------
// The message the verifier is given
// ---------------------------------------------------------------------------

#[test]
fn the_verifier_is_given_exactly_the_canonical_signing_bytes() {
    let verifier = Arc::new(RecordingVerifier::default());
    let bus = EventBus::with_verifier(BusConfig::default(), verifier.clone());

    let event = event("evt_canonical", json!({ "b": 2, "a": 1 }));
    let expected = event.signing_bytes().to_vec();
    assert!(expected.starts_with(b"ARKHE-EVENT-BUS/v1/event"));

    bus.publish(event.clone().with_signature(sign(&event)))
        .expect("accepted");

    let seen = verifier.messages();
    assert_eq!(seen.len(), 1, "the verifier is consulted exactly once");
    assert_eq!(seen[0], expected);

    // A JSON round trip does not change the bytes a signature must cover: an
    // auditor can verify an event it received as JSON.
    let json = event.to_json().expect("serializes");
    let reparsed = Event::from_json(json.as_bytes()).expect("parses");
    assert_eq!(
        reparsed.signing_bytes(),
        event.signing_bytes(),
        "canonicalisation must be stable across the wire form"
    );

    let signature = sign(&reparsed);
    let signed_again = reparsed.with_signature(signature);
    // A second bus, because the first already holds this `event_id` (uniqueness
    // is per bus instance).
    let auditor_bus = EventBus::with_verifier(BusConfig::default(), verifier.clone());
    auditor_bus
        .publish(signed_again)
        .expect("accepted after round trip");

    let seen = verifier.messages();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        seen[0], seen[1],
        "the same event must present the same bytes before and after a round trip"
    );
}

#[test]
fn key_order_in_the_payload_does_not_change_the_signing_bytes() {
    let verifier = Arc::new(RecordingVerifier::default());
    let bus = EventBus::with_verifier(BusConfig::default(), verifier.clone());

    let one = Event::new(meta("evt_order", 1), json!({ "a": 1, "b": 2, "c": 3 })).expect("valid");
    let other = Event::new(meta("evt_order", 1), json!({ "c": 3, "a": 1, "b": 2 })).expect("valid");

    // Only the first can be published (unique event_id), so compare the bytes
    // directly and then through the bus.
    assert_eq!(one.signing_bytes(), other.signing_bytes());
    assert_eq!(one.digest(), other.digest());

    bus.publish(one.clone().with_signature(sign(&one)))
        .expect("accepted");
    assert_eq!(verifier.messages().len(), 1);
}

#[test]
fn a_verifier_is_not_consulted_for_an_unsigned_event_under_optional() {
    let verifier = Arc::new(RecordingVerifier::default());
    let bus = EventBus::with_verifier(BusConfig::default(), verifier.clone());

    bus.publish(event("evt_no_signature", json!({ "k": 1 })))
        .expect("accepted");
    assert!(
        verifier.messages().is_empty(),
        "there was no signature to verify"
    );
}

#[test]
fn a_verifier_that_reports_failure_is_believed() {
    // A verifier that rejects everything. The bus must not second-guess it.
    struct RejectAll;
    impl SignatureVerifier for RejectAll {
        fn verify(&self, _message: &[u8], _signature: &[u8]) -> bool {
            false
        }
    }

    let bus = required_bus(Arc::new(RejectAll));
    let event = event("evt_rejected", json!({ "k": 1 })).with_signature(
        EventSignature::from_bytes(vec![0u8; 64]).expect("valid signature"),
    );
    assert_eq!(
        bus.publish(event),
        Err(BusError::InvalidSignature {
            event_id: String::from("evt_rejected"),
        })
    );
    assert_eq!(bus.log_len(), 0);
}

#[test]
fn an_optional_bus_verifies_a_signature_when_one_is_present() {
    let bus = EventBus::with_verifier(BusConfig::default(), Arc::new(DigestEqualityVerifier));

    // Unsigned: accepted.
    bus.publish(event("evt_a", json!({ "k": 1 }))).expect("accepted");

    // Signed correctly: accepted.
    let correct = event("evt_b", json!({ "k": 2 }));
    bus.publish(correct.clone().with_signature(sign(&correct)))
        .expect("accepted");

    // Signed incorrectly: refused, even though the policy is only Optional.
    let wrong = event("evt_c", json!({ "k": 3 })).with_signature(
        EventSignature::from_bytes(vec![4u8; 32]).expect("valid signature"),
    );
    assert_eq!(
        bus.publish(wrong),
        Err(BusError::InvalidSignature {
            event_id: String::from("evt_c"),
        })
    );

    assert_eq!(bus.log_len(), 2);
    let ids: Vec<String> = bus
        .log()
        .events()
        .iter()
        .map(|event| event.event_id().to_string())
        .collect();
    assert_eq!(ids, ["evt_a", "evt_b"]);
}
