//! End-to-end integration for the PoTT substrate, driven through the public
//! crate API only — mimicking an operator issuing a Bitcoin payload from Earth,
//! relaying it through a Mars loop and settling against a Lightning HTLC.

use arkhe_pott::lightning::{cltv_extra_blocks, header_bps, rtt_minutes};
use arkhe_pott::verify::{
    arrived_before_expiry, transcript_commit, transcript_open, verify_chain, AuthorizedRelay,
    Domain, MTPReference, Profile, VerifyOptions,
};
use arkhe_pott::wire::APPENDIX_A_HEX;
use arkhe_pott::{receipt, Chain, NodeId, PayloadKind, Receipt, H32, Nonce16};

/// Reproduces the appendix A vector through the public encode/decode APIs.
#[test]
fn appendix_a_wire_vector_roundtrips() {
    let bytes = appendix_a_bytes();
    let parsed = Receipt::from_bytes(&bytes).expect("normative vector decodes");
    assert_eq!(parsed.h[0], 0x83);
    assert_eq!(parsed.tin, 0x65B9B8A0);
    assert_eq!(parsed.tout, 0x65B9BD40);
    assert_eq!(parsed.sig.len(), 64);
    // Re-encoding reproduces the normative hex exactly.
    assert_eq!(parsed.to_wire(), bytes);
}

/// Minutes → seconds.
fn min_secs(mins: u64) -> u64 {
    mins * 60
}

/// Decodes the appendix-A hex once (timestamps are contiguous 8-byte tokens).
fn appendix_a_bytes() -> Vec<u8> {
    let compact: String = APPENDIX_A_HEX.split_whitespace().collect();
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).expect("ascii"), 16).expect("hex"))
        .collect()
}

struct Relay {
    secret: [u8; 32],
    node: NodeId,
    domain: Domain,
    operator: u32,
}

impl Relay {
    fn earth(seed: u8, operator: u32) -> Self {
        let secret = Self::secret(seed);
        let node = receipt::xonly_pubkey(&receipt::signing_key_from_secret(&secret));
        Self {
            secret,
            node,
            domain: Domain::Earth,
            operator,
        }
    }

    fn mars(seed: u8, operator: u32) -> Self {
        let secret = Self::secret(seed);
        let node = receipt::xonly_pubkey(&receipt::signing_key_from_secret(&secret));
        Self {
            secret,
            node,
            domain: Domain::Mars,
            operator,
        }
    }

    fn secret(seed: u8) -> [u8; 32] {
        let mut s = [seed; 32];
        s[31] = seed.wrapping_add(0x5a);
        s
    }

    fn authorized(&self) -> AuthorizedRelay {
        AuthorizedRelay {
            node: self.node,
            domain: self.domain,
            operator_id: self.operator,
        }
    }
}

/// Simulates a header-first replication of one 80-byte block header from Earth
/// to Mars over a 3-hop path (Earth gateway → deep-space relay → Mars gateway).
#[test]
fn earth_to_mars_header_chain_passes_m2() {
    let earth1 = Relay::earth(1, 1);
    let dsl = Relay::mars(2, 2); // deep-space link (Mars-operated domain anchor)
    let mars_gw = Relay::mars(3, 3);

    // Payload: a synthetic 80-byte header; H(P) = double-SHA-256.
    let header: [u8; 80] = [0x12; 80];
    let h: H32 = receipt::payload_digest(PayloadKind::Header, &header);
    let nu: Nonce16 = [0xca; 16];

    // TAI now (~2026): reuse the reference-paper magnitude + a modern offset.
    let t0 = 2_160_000_000u64; // ≈ 2026-06 in TAI-from-1958 seconds.
    let mut chain = Chain::new(h, nu);
    chain
        .append_signed(&earth1.secret, earth1.node, t0, t0 + min_secs(5))
        .unwrap();
    chain
        .append_signed(&dsl.secret, dsl.node, t0 + min_secs(25), t0 + min_secs(50))
        .unwrap();
    chain
        .append_signed(&mars_gw.secret, mars_gw.node, t0 + min_secs(70), t0 + min_secs(90))
        .unwrap();

    let opts = VerifyOptions {
        allowlist: vec![earth1.authorized(), dsl.authorized(), mars_gw.authorized()],
        profile: Profile::M2,
        ..Default::default()
    };
    let verified = verify_chain(&chain, &opts).expect("M2 must accept a diverse path");
    assert!(verified.domains.contains(&Domain::Earth));
    assert!(verified.domains.contains(&Domain::Mars));

    // HTLC settlement: the header arrived long before the 144+Δ CLTV expiry.
    let mtp_utc = arkhe_pott::time::posix_from_tai_u64(verified.terminal_tai);
    let mtp = MTPReference {
        tip_height: 800_100,
        hexpiry: 800_260, // ~26h of 10-min blocks
        kappa: 6,
        mtp_utc_secs: mtp_utc + 3_000,
        delta_mtp_secs: 3_600,
    };
    assert!(arrived_before_expiry(verified.terminal_tai, &mtp, opts.safety_delta_secs()));

    // Latency-aware policy for this route (Earth–Mars: 22 min light time).
    assert_eq!(rtt_minutes(22), 44);
    assert_eq!(cltv_extra_blocks(44, 60), 11);
}

#[test]
fn tampered_interior_hop_fails_e2e() {
    let earth1 = Relay::earth(7, 1);
    let dsl = Relay::mars(8, 2);
    let mars_gw = Relay::mars(9, 3);

    let mut chain = Chain::new([0x42; 32], [0x24; 16]);
    chain
        .append_signed(&earth1.secret, earth1.node, 2_160_000_000, 2_160_000_060)
        .unwrap();
    chain
        .append_signed(&dsl.secret, dsl.node, 2_160_000_120, 2_160_000_300)
        .unwrap();
    chain
        .append_signed(&mars_gw.secret, mars_gw.node, 2_160_000_360, 2_160_000_420)
        .unwrap();

    // Attacker rewrites a hop timestamp; signed/payload bytes now disagree.
    let mut attacker = chain.clone();
    attacker.receipts[1].tout = 2_160_000_999;

    let opts = VerifyOptions {
        allowlist: vec![earth1.authorized(), dsl.authorized(), mars_gw.authorized()],
        profile: Profile::M2,
        ..Default::default()
    };
    assert!(verify_chain(&attacker, &opts).is_err());
    assert!(verify_chain(&chain, &opts).is_ok());
}

#[test]
fn privacy_commit_end_to_end() {
    let earth1 = Relay::earth(11, 1);
    let mars_gw = Relay::mars(12, 2);
    let mut chain = Chain::new([0x55; 32], [0x66; 16]);
    chain
        .append_signed(&earth1.secret, earth1.node, 2_160_000_000, 2_160_000_060)
        .unwrap();
    chain
        .append_signed(&mars_gw.secret, mars_gw.node, 2_160_000_200, 2_160_000_260)
        .unwrap();

    let commit = transcript_commit(&chain).expect("commit");
    assert_eq!(commit.hops, 2);
    assert_eq!(commit.min_tin, 2_160_000_000);
    assert_eq!(commit.max_tout, 2_160_000_260);
    assert!(transcript_open(&commit, &chain).is_ok());

    let mut forged = chain.clone();
    forged.receipts[0].nu = [0x00; 16];
    assert!(transcript_open(&commit, &forged).is_err());
}

#[test]
fn header_link_budget_matches_paper() {
    // 80 B × 52,560 blocks/yr ≈ 1.07 bps sustained.
    assert!((header_bps() - 1.07).abs() < 0.02);
}