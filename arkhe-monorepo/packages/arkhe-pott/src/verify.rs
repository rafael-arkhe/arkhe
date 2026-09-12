//! Verification of PoTT chains of custody.
//!
//! A chain parsed from the wire is **untrusted** until
//! [`verify_chain`] accepts it under an authorized relay allowlist
//! (529-RUST-VALIDATE-KERNEL-API convention). Verification enforces:
//!
//! 1. size limits (≤ [`Chain::DEFAULT_MAX_HOPS`] hops, ≤ 8 kB);
//! 2. receipts all bound the **same** payload digest `h` and nonce `ν`, and
//!    every relay identity is on the allowlist;
//! 3. every BIP-340 signature verifies over its key-0–5 payload;
//! 4. timestamps are monotonic (`tin ≤ tout < tnext.tin`);
//! 5. the anti-splice hash chain holds: `previ = H(R(i−1) \ s(i−1))`;
//! 6. per-hop dwell is within the OWLT + jitter envelope.
//!
//! The [`Profile::M2`] ("PoTT-M2") assurance profile adds administrative
//! diversity requirements: ≥ 3 hops, ≥ 2 distinct operator domains, and at
//! least one time anchor in each of two planetary domains.
//!
//! Timing evidence is anchored to Bitcoin MedianTimePast (BIP-113) through
//! [`arrived_before_expiry`]; transcript privacy is supported via
//! [`transcript_commit`] / [`transcript_open`].

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::fmt;

use crate::receipt::{prev_hash_of, Chain, PREV_ZERO};
use crate::time::posix_from_tai_u64;
use crate::wire::{H32, NodeId, TAI};

/// Planetary (or organizational) anchor domain for a relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Domain {
    /// Terra — GNSS/DSN-anchored time beacons.
    Earth,
    /// Mars — local optical two-way time transfer.
    Mars,
    /// Third-party / other domain.
    Other,
}

/// One authorized relay on the custody path.
#[derive(Debug, Clone, Copy)]
pub struct AuthorizedRelay {
    /// BIP-340 x-only public key (32 bytes).
    pub node: NodeId,
    /// Administrative domain providing time-beacon anchors.
    pub domain: Domain,
    /// Operator identity (collusion-resistance: diversity counted by operator).
    pub operator_id: u32,
}

/// Verification profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Structural + cryptographic checks only.
    Base,
    /// PoTT-M2: adds hop-count, operator-diversity and dual-domain anchoring.
    M2,
}

/// Options controlling chain verification.
#[derive(Debug, Clone)]
pub struct VerifyOptions {
    /// Authorized relay identities; chains referencing anything else fail.
    pub allowlist: Vec<AuthorizedRelay>,
    /// Verification profile (default [`Profile::Base`]).
    pub profile: Profile,
    /// Minimum hop count for [`Profile::M2`].
    pub min_hops_m2: usize,
    /// Maximum hop count (paper: 32).
    pub max_hops: usize,
    /// Maximum serialized chain size (paper: 8 kB).
    pub max_chain_bytes: usize,
    /// Jitter/contact allowance `J` (seconds) — used for dwell envelopes.
    pub jitter_allowance_secs: u64,
    /// Clock uncertainty bound `σt` (seconds).
    pub sigma_t_secs: u64,
    /// Maximum one-way light time (OWLT) for the route (seconds).
    pub max_owlt_secs: u64,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        Self {
            allowlist: Vec::new(),
            profile: Profile::Base,
            min_hops_m2: 3,
            max_hops: Chain::DEFAULT_MAX_HOPS,
            max_chain_bytes: Chain::DEFAULT_MAX_CHAIN_BYTES,
            jitter_allowance_secs: 60 * 60, // 60 min
            sigma_t_secs: 60,               // ≤ 1 min
            max_owlt_secs: 22 * 60,         // Earth–Mars worst case
        }
    }
}

impl VerifyOptions {
    /// Round-trip envelope for per-hop dwell: `RTT_max + J`.
    #[must_use]
    pub fn max_hop_dwell_secs(&self) -> u64 {
        2 * self.max_owlt_secs + self.jitter_allowance_secs
    }

    /// Safety allowance `δ = J + 2σt` (paper §5.4).
    #[must_use]
    pub fn safety_delta_secs(&self) -> u64 {
        self.jitter_allowance_secs + 2 * self.sigma_t_secs
    }

    /// Looks up an authorized relay by x-only public key.
    pub fn find_relay(&self, node: &NodeId) -> Option<&AuthorizedRelay> {
        self.allowlist.iter().find(|r| &r.node == node)
    }
}

/// Why a chain failed verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// No receipts.
    EmptyChain,
    /// More receipts than [`VerifyOptions::max_hops`].
    TooManyHops,
    /// Serialized receipts exceed [`VerifyOptions::max_chain_bytes`].
    Oversize,
    /// A receipt references a relay outside the allowlist.
    UnknownRelay(NodeId),
    /// A receipt's BIP-340 signature does not verify.
    BadSignature(usize),
    /// Receipts disagree on the payload digest.
    PayloadMismatch,
    /// Receipts disagree on the per-message nonce.
    NonceMismatch,
    /// `tin > tout` on a single hop.
    BackwardsTime(usize),
    /// `tout(i) ≥ tin(i+1)` — hop times overlap.
    NonMonotonic(usize),
    /// `previ` does not hash to the previous receipt (splice detected).
    HashChainBroken(usize),
    /// First-hop `prev` is not `0^256`.
    RootNotZero,
    /// A hop's dwell time exceeds the OWLT + jitter envelope.
    DwellExceeded(usize),
    /// PoTT-M2 diversity requirements not met.
    DiversityInsufficient,
    /// PoTT-M2 hop-count requirement not met.
    InsufficientHops,
    /// Privacy-mode transcript does not match its commitment.
    TranscriptMismatch,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyChain => write!(f, "chain has no receipts"),
            Self::TooManyHops => write!(f, "chain exceeds max hop count"),
            Self::Oversize => write!(f, "chain exceeds max serialized size"),
            Self::UnknownRelay(n) => write!(f, "relay not on allowlist: {}", hex(n)),
            Self::BadSignature(i) => write!(f, "receipt {i}: bad signature"),
            Self::PayloadMismatch => write!(f, "receipts disagree on payload digest"),
            Self::NonceMismatch => write!(f, "receipts disagree on nonce"),
            Self::BackwardsTime(i) => write!(f, "receipt {i}: tout < tin"),
            Self::NonMonotonic(i) => write!(f, "receipt {i}: hop times not monotonic"),
            Self::HashChainBroken(i) => write!(f, "receipt {i}: anti-splice hash chain broken"),
            Self::RootNotZero => write!(f, "first hop prev is not 0^256"),
            Self::DwellExceeded(i) => write!(f, "receipt {i}: dwell exceeds OWLT+jitter envelope"),
            Self::DiversityInsufficient => write!(f, "PoTT-M2 diversity requirements not met"),
            Self::InsufficientHops => write!(f, "PoTT-M2 minimum hop count not met"),
            Self::TranscriptMismatch => write!(f, "transcript does not match its commitment"),
        }
    }
}

fn hex(b: &NodeId) -> alloc::string::String {
    let mut s = alloc::string::String::with_capacity(64);
    for byte in b {
        use core::fmt::Write as _;
        let _ = write!(s, "{byte:02x}");
    }
    s
}

/// Outcome of a successful verification.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedChain<'a> {
    /// The accepted chain.
    pub chain: &'a Chain,
    /// Terminal egress time (TAI) — `t⋆` for MTP anchoring.
    pub terminal_tai: TAI,
    /// Earliest ingress time (TAI).
    pub min_tin: TAI,
    /// Latest egress time (TAI).
    pub max_tout: TAI,
    /// Operator domains seen on the path.
    pub domains: BTreeSet<Domain>,
    /// Distinct operator identities seen on the path.
    pub operators: BTreeSet<u32>,
}

/// Verifies a PoTT chain under `options`.
///
/// Returns an [`VerifiedChain`] whose content is now trusted evidence.
pub fn verify_chain<'a>(
    chain: &'a Chain,
    options: &VerifyOptions,
) -> Result<VerifiedChain<'a>, VerifyError> {
    if chain.receipts.is_empty() {
        return Err(VerifyError::EmptyChain);
    }
    if chain.receipts.len() > options.max_hops {
        return Err(VerifyError::TooManyHops);
    }
    if chain.wire_bytes() > options.max_chain_bytes {
        return Err(VerifyError::Oversize);
    }

    let mut domains = BTreeSet::new();
    let mut operators = BTreeSet::new();

    for (i, r) in chain.receipts.iter().enumerate() {
        if r.h != chain.h {
            return Err(VerifyError::PayloadMismatch);
        }
        if r.nu != chain.nu {
            return Err(VerifyError::NonceMismatch);
        }
        let relay = options.find_relay(&r.node).ok_or(VerifyError::UnknownRelay(r.node))?;
        domains.insert(relay.domain);
        operators.insert(relay.operator_id);

        if !r.verify_schnorr(&r.node) {
            return Err(VerifyError::BadSignature(i));
        }
        if r.tout < r.tin {
            return Err(VerifyError::BackwardsTime(i));
        }
        let dwell = r.tout - r.tin;
        if dwell > options.max_hop_dwell_secs() {
            return Err(VerifyError::DwellExceeded(i));
        }
        if let Some(prev) = chain.receipts.get(i.wrapping_sub(1)) {
            if r.prev != prev_hash_of(prev) {
                return Err(VerifyError::HashChainBroken(i));
            }
            if r.tin <= prev.tout {
                return Err(VerifyError::NonMonotonic(i));
            }
        } else if r.prev != PREV_ZERO {
            return Err(VerifyError::RootNotZero);
        }
    }

    if options.profile == Profile::M2 {
        if chain.receipts.len() < options.min_hops_m2 {
            return Err(VerifyError::InsufficientHops);
        }
        if operators.len() < 2 || domains.len() < 2 {
            return Err(VerifyError::DiversityInsufficient);
        }
    }

    let first = &chain.receipts[0];
    let last = chain.receipts.last().expect("non-empty");
    Ok(VerifiedChain {
        chain,
        terminal_tai: last.tout,
        min_tin: first.tin,
        max_tout: last.tout,
        domains,
        operators,
    })
}

/// Reference to Bitcoin's best chain for the "arrived-before-expiry" test.
#[derive(Debug, Clone, Copy)]
pub struct MTPReference {
    /// Best-chain height `h⋆` at the moment of the check.
    pub tip_height: u64,
    /// HTLC on-chain expiry height (BOLT #2/#3).
    pub hexpiry: u64,
    /// Optional reorg margin `κ` (0–6 typical).
    pub kappa: u64,
    /// Median-Time-Past of the best-chain tip, in Unix/UTC seconds (BIP-113).
    pub mtp_utc_secs: i64,
    /// Conservative policy bound `ΔMTP` on MTP–UTC skew (default: 1 h).
    pub delta_mtp_secs: i64,
}

impl Default for MTPReference {
    fn default() -> Self {
        Self {
            tip_height: 0,
            hexpiry: 0,
            kappa: 0,
            mtp_utc_secs: 0,
            delta_mtp_secs: 3_600,
        }
    }
}

/// Anchors a PoTT chain's terminal timestamp against Bitcoin time.
///
/// Per the paper, a message "arrived before expiry" when
///
/// ```text
/// t⋆_UTC + δ ≤ TMTP + ΔMTP   AND   h⋆ ≤ hexpiry − κ
/// ```
///
/// with `t⋆_UTC` the terminal egress TAI converted to UTC/Unix seconds and
/// `δ` the safety allowance (`J + 2σt`).
#[must_use]
pub fn arrived_before_expiry(
    terminal_tai: TAI,
    mtp: &MTPReference,
    delta_secs: u64,
) -> bool {
    let t_star_utc = posix_from_tai_u64(terminal_tai);
    let timed_ok = t_star_utc
        .saturating_add(i64::try_from(delta_secs).unwrap_or(i64::MAX))
        <= mtp.mtp_utc_secs.saturating_add(mtp.delta_mtp_secs);
    let height_ok = mtp.tip_height.saturating_add(mtp.kappa) <= mtp.hexpiry;
    timed_ok && height_ok
}

/// Privacy-mode commitment: `Htxpt = SHA-256(R0 ‖ … ‖ RN)` plus the minimum
/// disclosure tuple `(Htxpt, min(tin), max(tout), hopcount)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptCommit {
    /// `Htxpt = SHA-256(R0 ‖ … ‖ RN)`.
    pub digest: H32,
    /// `min(tin)` over the path.
    pub min_tin: TAI,
    /// `max(tout)` over the path.
    pub max_tout: TAI,
    /// Hop count.
    pub hops: usize,
}

/// Computes the privacy-mode commitment for a chain (None if empty).
#[must_use]
pub fn transcript_commit(chain: &Chain) -> Option<TranscriptCommit> {
    let first = chain.receipts.first()?;
    let last = chain.receipts.last()?;
    let mut buf = Vec::new();
    for r in &chain.receipts {
        buf.extend_from_slice(&r.to_wire());
    }
    Some(TranscriptCommit {
        digest: crate::receipt::sha256(&buf),
        min_tin: first.tin,
        max_tout: last.tout,
        hops: chain.receipts.len(),
    })
}

/// Opens a transcript commitment: re-hashes the chain and verifies the
/// disclosure triple `(min(tin), max(tout), hopcount)` is honest.
pub fn transcript_open(commit: &TranscriptCommit, chain: &Chain) -> Result<(), VerifyError> {
    let recomputed = transcript_commit(chain).ok_or(VerifyError::EmptyChain)?;
    if recomputed.digest != commit.digest {
        return Err(VerifyError::HashChainBroken(0));
    }
    if recomputed.min_tin != commit.min_tin
        || recomputed.max_tout != commit.max_tout
        || recomputed.hops != commit.hops
    {
        return Err(VerifyError::TranscriptMismatch);
    }
    Ok(())
}

/// Convenience: run [`transcript_commit`] and immediately open it (sanity).
#[must_use]
pub fn transcript_roundtrip(chain: &Chain) -> bool {
    match transcript_commit(chain) {
        Some(c) => transcript_open(&c, chain).is_ok(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipt::Chain;

    struct Fixture {
        secret: [u8; 32],
        node: NodeId,
    }

    fn fixture(seed: u8) -> Fixture {
        let mut secret = [seed; 32];
        secret[31] = seed.wrapping_add(1);
        let sk = crate::receipt::signing_key_from_secret(&secret);
        let node = crate::receipt::xonly_pubkey(&sk);
        Fixture { secret, node }
    }

    fn three_hop_chain() -> (Chain, Vec<AuthorizedRelay>) {
        let f0 = fixture(1);
        let f1 = fixture(2);
        let f2 = fixture(3);
        let mut chain = Chain::new([0x11; 32], [0x22; 16]);
        chain
            .append_signed(&f0.secret, f0.node, 1_700_000_000, 1_700_000_030)
            .unwrap();
        chain
            .append_signed(&f1.secret, f1.node, 1_700_000_100, 1_700_000_130)
            .unwrap();
        chain
            .append_signed(&f2.secret, f2.node, 1_700_000_200, 1_700_000_230)
            .unwrap();
        let allowlist = vec![
            AuthorizedRelay {
                node: f0.node,
                domain: Domain::Earth,
                operator_id: 1,
            },
            AuthorizedRelay {
                node: f1.node,
                domain: Domain::Earth,
                operator_id: 2,
            },
            AuthorizedRelay {
                node: f2.node,
                domain: Domain::Mars,
                operator_id: 3,
            },
        ];
        (chain, allowlist)
    }

    #[test]
    fn base_profile_passes_single_hop() {
        let f0 = fixture(1);
        let mut chain = Chain::new([0x11; 32], [0x22; 16]);
        chain
            .append_signed(&f0.secret, f0.node, 1_700_000_000, 1_700_000_030)
            .unwrap();
        let opts = VerifyOptions {
            allowlist: vec![AuthorizedRelay {
                node: f0.node,
                domain: Domain::Earth,
                operator_id: 1,
            }],
            ..Default::default()
        };
        let v = verify_chain(&chain, &opts).expect("base profile accepts");
        assert_eq!(v.terminal_tai, 1_700_000_030);
        assert_eq!(chain.hop_count(), 1);
    }

    #[test]
    fn m2_profile_accepts_diverse_three_hop_chain() {
        let (chain, allowlist) = three_hop_chain();
        let opts = VerifyOptions {
            allowlist,
            profile: Profile::M2,
            ..Default::default()
        };
        let v = verify_chain(&chain, &opts).expect("M2 accepts");
        assert!(v.domains.contains(&Domain::Earth));
        assert!(v.domains.contains(&Domain::Mars));
        assert_eq!(v.operators.len(), 3);
    }

    #[test]
    fn m2_rejects_single_domain() {
        let f0 = fixture(1);
        let f1 = fixture(2);
        let f2 = fixture(3);
        let mut chain = Chain::new([0x11; 32], [0x22; 16]);
        chain
            .append_signed(&f0.secret, f0.node, 1_700_000_000, 1_700_000_030)
            .unwrap();
        chain
            .append_signed(&f1.secret, f1.node, 1_700_000_100, 1_700_000_130)
            .unwrap();
        chain
            .append_signed(&f2.secret, f2.node, 1_700_000_200, 1_700_000_230)
            .unwrap();
        // Everything is from Earth → fails dual-domain anchoring.
        let allowlist = vec![
            AuthorizedRelay {
                node: f0.node,
                domain: Domain::Earth,
                operator_id: 1,
            },
            AuthorizedRelay {
                node: f1.node,
                domain: Domain::Earth,
                operator_id: 2,
            },
            AuthorizedRelay {
                node: f2.node,
                domain: Domain::Earth,
                operator_id: 3,
            },
        ];
        let opts = VerifyOptions {
            allowlist,
            profile: Profile::M2,
            ..Default::default()
        };
        assert_eq!(verify_chain(&chain, &opts), Err(VerifyError::DiversityInsufficient));
    }

    #[test]
    fn tampered_timestamp_breaks_signature() {
        let (mut chain, allowlist) = three_hop_chain();
        chain.receipts[1].tout = 1_700_000_999; // egress forged
        let opts = VerifyOptions {
            allowlist,
            ..Default::default()
        };
        assert!(matches!(
            verify_chain(&chain, &opts),
            Err(VerifyError::BadSignature(_))
        ));
    }

    #[test]
    fn reorder_breaks_hash_chain() {
        let (mut chain, allowlist) = three_hop_chain();
        chain.receipts.swap(1, 2);
        let opts = VerifyOptions {
            allowlist: allowlist.clone(),
            ..Default::default()
        };
        assert!(matches!(
            verify_chain(&chain, &opts),
            Err(VerifyError::HashChainBroken(_)) | Err(VerifyError::NonMonotonic(_))
        ));
    }

    #[test]
    fn splice_of_different_instances_detected() {
        // First two receipts valid for instance A, third receipt from instance B.
        let f0 = fixture(1);
        let f1 = fixture(2);
        let _f2 = fixture(3);
        let mut chain_a = Chain::new([0x11; 32], [0x22; 16]);
        chain_a
            .append_signed(&f0.secret, f0.node, 1_700_000_000, 1_700_000_030)
            .unwrap();
        chain_a
            .append_signed(&f1.secret, f1.node, 1_700_000_100, 1_700_000_130)
            .unwrap();

        let mut chain_b = Chain::new([0x11; 32], [0x33; 16]);
        // NOTE: chain B reuses hop 1's node, but with a fresh nonce.
        chain_b
            .append_signed(&f1.secret, f1.node, 1_700_000_200, 1_700_000_230)
            .unwrap();
        let fork_receipt = chain_b.receipts[0];

        let mut spliced = chain_a.clone();
        spliced.receipts.push(fork_receipt);
        let allowlist = vec![
            AuthorizedRelay {
                node: f0.node,
                domain: Domain::Earth,
                operator_id: 1,
            },
            AuthorizedRelay {
                node: f1.node,
                domain: Domain::Earth,
                operator_id: 2,
            },
        ];
        let opts = VerifyOptions {
            allowlist,
            ..Default::default()
        };
        // The third receipt either carries the wrong nonce or a broken prev.
        assert!(matches!(
            verify_chain(&spliced, &opts),
            Err(VerifyError::NonceMismatch) | Err(VerifyError::HashChainBroken(_))
        ));
    }

    #[test]
    fn unknown_relay_rejected() {
        let (chain, allowlist) = three_hop_chain();
        let mut opts = VerifyOptions {
            allowlist,
            ..Default::default()
        };
        // Drop the middle relay from the allowlist.
        opts.allowlist.remove(1);
        assert!(matches!(
            verify_chain(&chain, &opts),
            Err(VerifyError::UnknownRelay(_))
        ));
    }

    #[test]
    fn oversize_chain_rejected() {
        let (chain, allowlist) = three_hop_chain();
        let opts = VerifyOptions {
            allowlist,
            max_chain_bytes: 100, // room for ~half a receipt
            ..Default::default()
        };
        assert_eq!(verify_chain(&chain, &opts), Err(VerifyError::Oversize));
    }

    #[test]
    fn mtp_anchoring_respects_bounds() {
        // terminal TAI 1_700_000_230 → a specific unix instant.
        let terminal = 1_700_000_230u64;
        let t_utc = posix_from_tai_u64(terminal);
        let mtp = MTPReference {
            tip_height: 800_000,
            hexpiry: 800_010,
            kappa: 6,
            mtp_utc_secs: t_utc + 4000,
            delta_mtp_secs: 3_600,
        };
        // δ = 200s and t⋆ + 200 ≤ mtp + 3600 holds; height also fits.
        assert!(arrived_before_expiry(terminal, &mtp, 200));

        // But if the tip is only 4 blocks below expiry and κ=6, reject.
        let mtp_late = MTPReference {
            hexpiry: 800_004,
            ..mtp
        };
        assert!(!arrived_before_expiry(terminal, &mtp_late, 200));

        // And if the message plainly arrived after the MTP bound, reject.
        let mtp_early = MTPReference {
            mtp_utc_secs: t_utc - 10_000,
            ..mtp
        };
        assert!(!arrived_before_expiry(terminal, &mtp_early, 200));
    }

    #[test]
    fn transcript_commit_opens() {
        let (chain, _allowlist) = three_hop_chain();
        let commit = transcript_commit(&chain).expect("commit");
        assert_eq!(commit.hops, 3);
        assert!(transcript_open(&commit, &chain).is_ok());

        // Tampering breaks the open.
        let mut tampered = chain.clone();
        tampered.receipts[0].tout += 1;
        assert!(transcript_open(&commit, &tampered).is_err());
    }
}