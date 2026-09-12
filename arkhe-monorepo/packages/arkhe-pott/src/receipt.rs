//! PoTT receipts and chains of custody.
//!
//! A [`Receipt`] is the per-hop attestation appended by every relay that
//! touches an interplanetary Bitcoin payload. Receipts are chained: hop `i`
//! binds to hop `i−1` through
//!
//! ```text
//! prev0   := 0^256
//! previ   := H(R(i−1) \ s(i−1)) = SHA-256(signing_payload(R(i−1)))
//! Mi      := signing_payload(Ri)             (canonical CBOR of keys 0–5)
//! si      := SchnorrBIP340(NodeIDi, Mi)
//! ```
//!
//! Removing or reordering any interior hop changes every subsequent `prev` and
//! invalidates the signatures — the chain cannot be cut-and-rejoined without
//! detection.
//!
//! Relay identities are BIP-340 x-only secp256k1 public keys ([`NodeId`], 32
//! bytes). Signing/verification is done with the pure-Rust `k256` crate
//! (no C toolchain required).

use alloc::vec::Vec;

use k256::schnorr::{Signature as SchnorrSignature, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use signature::{Signer, Verifier};

use crate::wire::{self, H32, NodeId, Nonce16, Sig64, TAI};

/// Error produced while building or parsing a chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    /// Wire parse failed (bad CBOR).
    Wire(wire::WireError),
    /// Egress time precedes ingress time on the same hop.
    BackwardsTime,
    /// A later hop's ingress time precedes the previous hop's egress time.
    NonMonotonic,
    /// Key material rejected by the BIP-340 signer.
    Signing,
    /// The provided secret cannot be turned into a BIP-340 signing key.
    InvalidKey,
    /// Attempted to parse a chain with no receipts.
    Empty,
}

impl From<wire::WireError> for ChainError {
    fn from(e: wire::WireError) -> Self {
        ChainError::Wire(e)
    }
}

/// A single hop receipt (canonical CBOR map keys `0..=6`, see [`wire`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Receipt {
    /// Payload digest `h = H(P)`.
    pub h: H32,
    /// Per-message nonce (minted once per payload instance, echoed unchanged).
    pub nu: Nonce16,
    /// Relay NodeID (BIP-340 x-only public key).
    pub node: NodeId,
    /// Ingress time (TAI seconds, CUC epoch 1958-01-01).
    pub tin: TAI,
    /// Egress time (TAI seconds).
    pub tout: TAI,
    /// Anti-splice binding `previ = H(R(i−1) \ s(i−1))`.
    pub prev: H32,
    /// BIP-340 Schnorr signature over the canonical CBOR of keys 0–5.
    pub sig: Sig64,
}

impl Receipt {
    /// Builds a receipt from explicit fields (used by relays and tests).
    #[must_use]
    pub const fn new(
        h: H32,
        nu: Nonce16,
        node: NodeId,
        tin: TAI,
        tout: TAI,
        prev: H32,
        sig: Sig64,
    ) -> Self {
        Self {
            h,
            nu,
            node,
            tin,
            tout,
            prev,
            sig,
        }
    }

    /// Canonical CBOR of keys 0–5 — the exact byte range `Mi` signed by the
    /// relay and hashed by the next hop's `prev`.
    #[must_use]
    pub fn signing_payload(&self) -> Vec<u8> {
        wire::encode_signing_payload(&self.h, &self.nu, &self.node, self.tin, self.tout, &self.prev)
    }

    /// Full canonical CBOR (keys 0–6) — the wire form of the receipt.
    #[must_use]
    pub fn to_wire(&self) -> Vec<u8> {
        wire::encode_receipt(&wire::WireReceipt {
            h: self.h,
            nu: self.nu,
            node: self.node,
            tin: self.tin,
            tout: self.tout,
            prev: self.prev,
            sig: self.sig,
        })
    }

    /// Parses a receipt from its canonical CBOR bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ChainError> {
        let w = wire::decode_receipt(bytes)?;
        Ok(Self {
            h: w.h,
            nu: w.nu,
            node: w.node,
            tin: w.tin,
            tout: w.tout,
            prev: w.prev,
            sig: w.sig,
        })
    }

    /// Verifies this hop's BIP-340 signature under the given x-only key.
    pub fn verify_schnorr(&self, xonly: &NodeId) -> bool {
        let Ok(vk) = VerifyingKey::from_bytes(xonly) else {
            return false;
        };
        let Ok(sig) = SchnorrSignature::try_from(&self.sig[..]) else {
            return false;
        };
        vk.verify(&self.signing_payload(), &sig).is_ok()
    }
}

/// Computes `previ = H(R(i−1) \ s(i−1))` for an antecedent receipt.
#[must_use]
pub fn prev_hash_of(prev: &Receipt) -> H32 {
    sha256(&prev.signing_payload())
}

/// SHA-256 digest.
#[must_use]
pub fn sha256(bytes: &[u8]) -> H32 {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

/// Bitcoin double-SHA-256 digest.
#[must_use]
pub fn double_sha256(bytes: &[u8]) -> H32 {
    sha256(&sha256(bytes))
}

/// Payload classes with Bitcoin-native digest selection (paper §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadKind {
    /// Serialized 80-byte block header → double-SHA-256.
    Header,
    /// Serialized transaction → double-SHA-256.
    Transaction,
    /// BIP157/158 compact block filter → its double-SHA-256 filter hash.
    CompactFilter,
    /// Any other payload → SHA-256 over canonical bytes.
    Generic,
}

/// Computes `H(P)` per the PoTT payload digest rules.
#[must_use]
pub fn payload_digest(kind: PayloadKind, payload: &[u8]) -> H32 {
    match kind {
        PayloadKind::Header | PayloadKind::Transaction | PayloadKind::CompactFilter => {
            double_sha256(payload)
        }
        PayloadKind::Generic => sha256(payload),
    }
}

/// The all-zero `prev` used for the first hop.
pub const PREV_ZERO: H32 = [0u8; 32];

/// Chain of custody for one payload instance: `[R0, R1, …, RN]`.
///
/// Following the 529-RUST-VALIDATE-KERNEL-API convention, a [`Chain`] parsed
/// from the wire is *untrusted*: call [`crate::verify::verify_chain`] under an
/// authorized relay allowlist before treating its timings as evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Chain {
    /// Payload digest carried by every receipt.
    pub h: H32,
    /// Per-message nonce carried by every receipt.
    pub nu: Nonce16,
    /// Receipts in custody order (index 0 = origin).
    pub receipts: Vec<Receipt>,
}

impl Chain {
    /// Starts a new evidence set for payload `h` with nonce `nu`.
    #[must_use]
    pub fn new(h: H32, nu: Nonce16) -> Self {
        Self {
            h,
            nu,
            receipts: Vec::new(),
        }
    }

    /// Number of hops (receipts).
    pub fn hop_count(&self) -> usize {
        self.receipts.len()
    }

    /// Chain size cap default (paper: ≤ 32 hops or ≤ 8 kB per bundle).
    pub const DEFAULT_MAX_HOPS: usize = 32;
    /// Chain size cap default in bytes.
    pub const DEFAULT_MAX_CHAIN_BYTES: usize = 8 * 1024;

    /// Appends a signed receipt for this relay hop.
    ///
    /// The relay signs `Mi` = canonical CBOR of keys 0–5 under `secret_key`,
    /// and the new receipt binds to the previous hop through
    /// `prev = SHA-256(signing_payload(previous))`.
    pub fn append_signed(
        &mut self,
        secret_key: &[u8; 32],
        node: NodeId,
        tin: TAI,
        tout: TAI,
    ) -> Result<(), ChainError> {
        if tout < tin {
            return Err(ChainError::BackwardsTime);
        }
        if let Some(last) = self.receipts.last() {
            // Strict ordering: t_out(i) < t_in(i+1) is required by the paper.
            if tin <= last.tout {
                return Err(ChainError::NonMonotonic);
            }
        }

        let signing_key = secret_key_from_slice(secret_key)?;
        let prev = match self.receipts.last() {
            Some(last) => prev_hash_of(last),
            None => PREV_ZERO,
        };
        let pending = Receipt::new(self.h, self.nu, node, tin, tout, prev, [0u8; 64]);
        let payload = pending.signing_payload();
        let sig: SchnorrSignature = signing_key.sign(&payload);
        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&sig.to_bytes());
        self.receipts.push(Receipt::new(
            self.h, self.nu, node, tin, tout, prev, sig_bytes,
        ));
        Ok(())
    }

    /// Maximum nominal egress timestamp across the chain (TAI).
    pub fn terminal_tai(&self) -> Option<TAI> {
        self.receipts.last().map(|r| r.tout)
    }

    /// Serialized size of all receipts (keys 0–6), the on-wire overhead.
    pub fn wire_bytes(&self) -> usize {
        self.receipts.iter().map(|r| r.to_wire().len()).sum()
    }
}

fn secret_key_from_slice(secret: &[u8; 32]) -> Result<SigningKey, ChainError> {
    SigningKey::from_bytes(secret).map_err(|_| ChainError::InvalidKey)
}

/// Builds a BIP-340 signing key from a 32-byte secret (operator/test tooling).
#[must_use]
pub fn signing_key_from_secret(secret: &[u8; 32]) -> SigningKey {
    SigningKey::from_bytes(secret).expect("any 32-byte value is a valid BIP-340 secret")
}

/// Resolves the BIP-340 x-only public key (the PoTT [`NodeId`]) for a signing key.
#[must_use]
pub fn xonly_pubkey(signing_key: &SigningKey) -> NodeId {
    let mut node = [0u8; 32];
    node.copy_from_slice(&signing_key.verifying_key().to_bytes());
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_h() -> H32 {
        [0x42; 32]
    }

    fn test_nu() -> Nonce16 {
        [0x77; 16]
    }

    fn keys() -> ([u8; 32], NodeId) {
        let mut secret = [1u8; 32];
        secret[31] = 2;
        let sk = secret_key_from_slice(&secret).expect("valid secret");
        let mut node = [0u8; 32];
        node.copy_from_slice(&sk.verifying_key().to_bytes());
        (secret, node)
    }

    #[test]
    fn two_hop_chain_signs_and_verifies() {
        let (secret, node) = keys();
        let mut chain = Chain::new(test_h(), test_nu());
        chain
            .append_signed(&secret, node, 1_700_000_000, 1_700_000_060)
            .expect("hop 0");
        chain
            .append_signed(&secret, node, 1_700_000_120, 1_700_000_200)
            .expect("hop 1");

        assert_eq!(chain.hop_count(), 2);
        assert_eq!(chain.receipts[0].prev, PREV_ZERO);
        assert_eq!(chain.receipts[1].prev, prev_hash_of(&chain.receipts[0]));
        for r in &chain.receipts {
            assert!(r.verify_schnorr(&r.node), "signature must verify");
        }
    }

    #[test]
    fn non_monotonic_time_rejected() {
        let (secret, node) = keys();
        let mut chain = Chain::new(test_h(), test_nu());
        chain
            .append_signed(&secret, node, 1_700_000_000, 1_700_000_060)
            .expect("hop 0");
        // Next hop ingresses *before* hop 0 egresses.
        assert_eq!(
            chain.append_signed(&secret, node, 1_700_000_030, 1_700_000_100),
            Err(ChainError::NonMonotonic)
        );
    }

    #[test]
    fn backwards_time_rejected() {
        let (secret, _node) = keys();
        let mut chain = Chain::new(test_h(), test_nu());
        assert_eq!(
            chain.append_signed(&secret, [0u8; 32], 200, 100),
            Err(ChainError::BackwardsTime)
        );
    }

    #[test]
    fn double_sha256_is_sha_of_sha() {
        let msg = b"arkhe pott";
        assert_eq!(double_sha256(msg), sha256(&sha256(msg)));
    }

    #[test]
    fn payload_digest_by_kind() {
        let payload = [0xabu8; 40];
        assert_eq!(
            payload_digest(PayloadKind::Header, &payload),
            double_sha256(&payload)
        );
        assert_eq!(
            payload_digest(PayloadKind::Generic, &payload),
            sha256(&payload)
        );
    }
}