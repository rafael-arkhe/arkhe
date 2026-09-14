//! Benchmark driver: runs N emit→verify cycles and reports wall-clock timing
//! plus a comparison to the (UNVERIFIED) arXiv:2607.16100 reference latency.
//!
//! This measures the software TLM on a host CPU — it is NOT a silicon latency
//! and does NOT validate the paper. It exists so the RTL has numbers to match.

use arkhe_soc_tlm::aotb::{AotbEncoder, AotbVerifier};
use arkhe_soc_tlm::soc::ArkheSoc;
use arkhe_soc_tlm::{DOMAIN_NODES, REFERENCE_SOL_US};
use ed25519_dalek::SigningKey;
use serde::Serialize;
use std::time::Instant;

#[derive(Serialize)]
struct Report {
    iterations: usize,
    wall_time_us: f64,
    per_iter_us: f64,
    frames_emitted: u64,
    frames_verified: u64,
    reference_sol_us: f64,
    reference_verified: bool,
    per_iter_vs_reference_ratio: f64,
    note: String,
}

fn main() {
    let iterations: usize = std::env::args()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(10_000);

    let sk = SigningKey::from_bytes(&[7u8; 32]);
    let vk = sk.verifying_key();
    let mut soc = ArkheSoc::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], [9; 16]);
    let mut enc = AotbEncoder::new(sk, soc.session_id(), soc.proof_hash());
    let mut ver = AotbVerifier::new(vk, soc.session_id());
    let _ = DOMAIN_NODES;

    let start = Instant::now();
    for seq in 0..iterations as u64 {
        let _ = soc.qpl_hop();
        soc.expand(seq);
        let f = soc.emit(&mut enc);
        ver.verify(&f).expect("benchmark frame must verify");
        soc.counters.frames_verified += 1;
    }
    let wall = start.elapsed().as_secs_f64() * 1e6;
    let per_iter = wall / iterations as f64;

    let report = Report {
        iterations,
        wall_time_us: wall,
        per_iter_us: per_iter,
        frames_emitted: soc.counters.frames_emitted,
        frames_verified: soc.counters.frames_verified,
        reference_sol_us: REFERENCE_SOL_US,
        reference_verified: false,
        per_iter_vs_reference_ratio: per_iter / REFERENCE_SOL_US,
        note: "Host-CPU software TLM timing. Reference latency is UNVERIFIED; \
               this does not validate arXiv:2607.16100."
            .to_string(),
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
