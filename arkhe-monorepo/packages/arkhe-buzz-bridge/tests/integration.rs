//! Integration tests: LT fountain round-trips, firewall, and event translation.

use arkhe_buzz_bridge::{
    BuzzBridge, CertificationStatus, DivergenceReport, EdgeType, EvidenceBundle,
    FountainDecoder, FountainEncoder, KIND_EVIDENCE_BUNDLE, OrchORState, Zone,
    validate_event_firewall, validate_hyperedge_firewall,
};
use nostr_sdk::prelude::{EventBuilder, Keys, Kind};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

#[test]
fn orchid_state_survives_30pct_loss() {
    let original = OrchORState::random();
    let data = original.to_bytes();

    let mut encoder = FountainEncoder::new(64, 57, 0xCAFE_BABE);
    let frames: Vec<Vec<u8>> = (0..192).map(|_| encoder.next_frame(&data).unwrap()).collect();

    let mut rng = StdRng::seed_from_u64(99);
    let received: Vec<Vec<u8>> = frames.into_iter().filter(|_| rng.gen::<f64>() > 0.30).collect();

    let mut decoder = FountainDecoder::new(64, 57);
    for frame in &received {
        decoder.ingest_frame(frame).unwrap();
        if decoder.is_complete() {
            break;
        }
    }
    assert!(decoder.is_complete(), "decode incomplete after {} frames", received.len());
    let recovered = decoder.get_decoded().unwrap();
    assert_eq!(recovered, data);
    assert_eq!(OrchORState::from_bytes(&recovered), Some(original));
}

#[test]
fn firewall_rejects_direct_z2_z3() {
    let nodes = vec![
        ("z2".to_string(), Zone::Z2_Continuous),
        ("z3".to_string(), Zone::Z3_Discrete),
    ];
    assert!(validate_hyperedge_firewall(&nodes, EdgeType::TranslatesToPrimitive.as_str()).is_ok());
    assert!(validate_hyperedge_firewall(&nodes, EdgeType::DependsOn.as_str()).is_err());
}

#[test]
fn event_firewall_binds_translation_digest() {
    let keys = Keys::generate();
    let bundle = EvidenceBundle {
        id: "eb-fw".into(),
        hypothesis: "fw".into(),
        baseline_hash: "b".into(),
        pump_sequence: vec![],
        probe: "p".into(),
        counterfactual: "c".into(),
        observations_forward: vec![],
        observations_reverse: vec![],
        divergences: DivergenceReport::none(0.5),
        witness: None,
        certification: CertificationStatus::Pending,
        timestamp: "2026-08-02T00:00:00Z".into(),
    };

    // Valid event: includes the correct translation_digest and valid signature.
    let builder = BuzzBridge::evidence_bundle_to_event(&bundle);
    let event = builder.to_event(&keys).unwrap();
    assert!(
        validate_event_firewall(
            &event,
            Zone::Z1_Tools,
            Kind::Custom(KIND_EVIDENCE_BUNDLE)
        )
        .is_ok(),
        "valid bundle should pass firewall"
    );

    // Tampered event: digest no longer matches content.
    let mut tags = vec![
        nostr_sdk::prelude::Tag::custom(
            nostr_sdk::prelude::TagKind::from("target_zone"),
            ["Z3_Discrete"],
        ),
        nostr_sdk::prelude::Tag::custom(
            nostr_sdk::prelude::TagKind::from("edge_type"),
            ["TRANSLATES_TO_PRIMITIVE"],
        ),
        nostr_sdk::prelude::Tag::custom(
            nostr_sdk::prelude::TagKind::from("translation_digest"),
            ["deadbeef"],
        ),
    ];
    let mut content = serde_json::to_string(&bundle).unwrap();
    content.push(' '); // mutate content after digest was computed
    let builder = EventBuilder::new(
        Kind::Custom(KIND_EVIDENCE_BUNDLE),
        content,
        std::mem::take(&mut tags),
    );
    let event = builder.to_event(&keys).unwrap();
    assert!(
        validate_event_firewall(
            &event,
            Zone::Z1_Tools,
            Kind::Custom(KIND_EVIDENCE_BUNDLE)
        )
        .is_err(),
        "tampered bundle must fail firewall"
    );
}
