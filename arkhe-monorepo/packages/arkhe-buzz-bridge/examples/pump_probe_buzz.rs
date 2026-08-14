//! End-to-end example: encode an OrchOR state with LT fountain codes, publish
//! the frames to a Buzz (Nostr) relay, simulate 30% loss, then recover.
//!
//! Usage:
//!   BUZZ_SECRET_KEY=nsec... BUZZ_RELAY_URL=wss://relay.example.com cargo run --example pump_probe_buzz
//!
//! The relay round-trip is skipped (with a notice) when the env vars are absent,
//! so the fountain encode/decode + firewall validation can run offline.

use arkhe_buzz_bridge::{
    BuzzBridge, CertificationStatus, DivergenceReport, EdgeType, EvidenceBundle,
    FountainDecoder, FountainEncoder, OrchORState, Zone, validate_hyperedge_firewall,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const BLOCK_SIZE: usize = 57;
const K_BLOCKS: usize = 64;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a random OrchOR state (CSPRNG-backed).
    let original = OrchORState::random();
    println!("original OrchOR state: {:?}", original);
    let data = original.to_bytes();

    // 2. Encode with LT fountain codes.
    let mut encoder = FountainEncoder::new(K_BLOCKS, BLOCK_SIZE, 0xCAFE_BABE);
    let mut frames: Vec<Vec<u8>> = Vec::with_capacity(192);
    for _ in 0..192 {
        frames.push(encoder.next_frame(&data)?);
    }
    println!("encoded {} frames (k={}, block={})", frames.len(), K_BLOCKS, BLOCK_SIZE);

    // 3. Simulate 30% loss.
    let mut rng = StdRng::seed_from_u64(7);
    let received: Vec<Vec<u8>> = frames
        .into_iter()
        .filter(|_| rng.gen::<f64>() > 0.30)
        .collect();
    println!("received {} frames after simulated 30% loss", received.len());

    // 4. Decode.
    let mut decoder = FountainDecoder::new(K_BLOCKS, BLOCK_SIZE);
    for frame in &received {
        decoder.ingest_frame(frame)?;
        if decoder.is_complete() {
            break;
        }
    }
    assert!(decoder.is_complete(), "fountain decode failed");
    let recovered = decoder.get_decoded().expect("decode incomplete");
    assert_eq!(recovered.as_slice(), &data[..]);
    println!("recovered OrchOR state matches original: {:?}", OrchORState::from_bytes(&recovered));

    // 5. Firewall validation (offline).
    let nodes = vec![
        ("z2".to_string(), Zone::Z2_Continuous),
        ("z3".to_string(), Zone::Z3_Discrete),
    ];
    validate_hyperedge_firewall(&nodes, EdgeType::TranslatesToPrimitive.as_str())?;
    assert!(validate_hyperedge_firewall(&nodes, EdgeType::DependsOn.as_str()).is_err());
    println!("firewall: Z2↔Z3 direct edge blocked, TRANSLATES_TO_PRIMITIVE allowed");

    // 6. Optional relay round-trip.
    match (std::env::var("BUZZ_SECRET_KEY"), std::env::var("BUZZ_RELAY_URL")) {
        (Ok(secret), Ok(relay)) => {
            let bridge = BuzzBridge::new(&secret, &relay)?;
            bridge.connect().await?;

            // Publish AFT frames. The Buzz relay enforces a per-minute message
            // quota (default 60/min for members), so pace publishes to stay
            // under the limit and prove a sustained live round-trip.
            let mut encoder = FountainEncoder::new(K_BLOCKS, BLOCK_SIZE, 0xCAFE_BABE);
            for i in 0..32 {
                let frame = encoder.next_frame(&data)?;
                let id = bridge.publish_aft_frame(&frame, "exp-001").await?;
                println!("published AFT frame {i}: {id}");
                if i < 31 {
                    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
                }
            }

            // Publish evidence bundle.
            let bundle = EvidenceBundle {
                id: "eb-001".into(),
                hypothesis: "Cohn-Elkies LP rate = sqrt(e)/2pi".into(),
                baseline_hash: "abc123".into(),
                pump_sequence: vec!["Mellin transform".into(), "Poisson interpolation".into()],
                probe: "Saddle asymptotics".into(),
                counterfactual: "Kabatianskii-Levenshtein".into(),
                observations_forward: vec!["Prop 3.1".into(), "Thm 4.1".into()],
                observations_reverse: vec!["Thm 3.8".into()],
                divergences: DivergenceReport {
                    structural: Some(0.6044),
                    observational: Some(std::f64::consts::FRAC_1_PI),
                    invariant_violations: vec![],
                    threshold: 0.5,
                    has_divergence: true,
                },
                witness: Some("thm-oai-1.1".into()),
                certification: CertificationStatus::Supported,
                timestamp: "2026-08-02T00:00:00Z".into(),
            };
            let id = bridge.publish_evidence_bundle(&bundle).await?;
            println!("published evidence bundle: {id}");

            // Fetch back.
            let frames = bridge.fetch_aft_frames("exp-001").await?;
            println!("fetched {} AFT frames from relay", frames.len());
            let bundles = bridge.fetch_evidence_bundles().await?;
            println!("fetched {} evidence bundles from relay", bundles.len());
        }
        _ => {
            println!(
                "skipping relay round-trip — set BUZZ_SECRET_KEY and BUZZ_RELAY_URL to run it"
            );
        }
    }

    println!("example complete");
    Ok(())
}
