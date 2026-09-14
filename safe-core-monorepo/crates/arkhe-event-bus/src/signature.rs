//! Per-event signatures — the bus-side half of threat **T-12**, and the
//! "signed events" term of **T-13**.
//!
//! §6.2 of the canonical paper (DOI 10.5281/zenodo.21383201) states:
//!
//! > **T-12**, man-in-the-middle between crates (mTLS with 24-hour certificates
//! > plus **per-event Ed25519 signatures verified by the bus**).
//!
//! > **T-13**, replay against the event log (monotonic sequence numbers, Merkle
//! > roots, **signed events**, checkpoint fencing).
//!
//! # This crate ships no cryptographic backend, deliberately
//!
//! The bus owns *where* verification happens and *what* bytes are covered. It
//! does not own an Ed25519 implementation and holds no key material:
//!
//! * the message is [`crate::event::Event::signing_bytes`], a domain-separated,
//!   length-framed, canonically-encoded byte string;
//! * the signature is opaque bytes ([`EventSignature`]) — verbatim from the
//!   publisher, never interpreted here;
//! * verification is [`SignatureVerifier`], a trait the embedding application
//!   implements with the crypto stack *it* already audits.
//!
//! `mTLS with 24-hour certificates`, the other half of T-12, is a transport
//! concern and is not implemented here (see the README).
//!
//! Adding a real backend (e.g. `ed25519-dalek`) would mean this crate's
//! dependency graph contains a signature implementation whose failure mode is
//! silent forgery. The Phase 1 scope keeps that decision where it belongs: with
//! the caller, whose keys it is.
//!
//! ## Fail-closed summary
//!
//! | Bus state | Event | Result |
//! |---|---|---|
//! | [`SignaturePolicy::Required`] | any | verified, or [`crate::error::BusError::SignatureVerifierUnavailable`] / [`crate::error::BusError::MissingSignature`] / [`crate::error::BusError::InvalidSignature`] |
//! | [`SignaturePolicy::Optional`] | unsigned | accepted |
//! | [`SignaturePolicy::Optional`] | signed, verifier configured | verified, or rejected |
//! | [`SignaturePolicy::Optional`] | signed, **no verifier** | rejected with `SignatureVerifierUnavailable` — an unverifiable signature is never accepted on faith |

use std::fmt;

use crate::error::{BusError, BusResult};

/// Longest accepted signature, in bytes.
///
/// Ed25519 signatures are 64 bytes. The cap is 128 so a scheme that prefixes a
/// length or an algorithm identifier still fits; it exists as an input-size
/// guard (a hostile publisher must not be able to hand the bus an unbounded
/// blob), not as a cryptographic claim.
pub const MAX_SIGNATURE_LEN: usize = 128;

/// Whether the bus accepts unsigned events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SignaturePolicy {
    /// Unsigned events are accepted. A signature that *is* present must still
    /// verify.
    #[default]
    Optional,
    /// Every accepted event must carry a signature that verifies. This is the
    /// policy T-12's mitigation implies for a bus carrying privileged traffic.
    Required,
}

impl fmt::Display for SignaturePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignaturePolicy::Optional => f.write_str("optional"),
            SignaturePolicy::Required => f.write_str("required"),
        }
    }
}

/// A signature, as opaque bytes.
///
/// The bus never parses this value. It is hex-encoded on the wire so that an
/// event's JSON form stays human-inspectable alongside its payload, and so that
/// a signature survives a round trip byte-for-byte.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EventSignature(Vec<u8>);

impl EventSignature {
    /// Validate and wrap signature bytes.
    ///
    /// Rejects an empty signature and anything longer than
    /// [`MAX_SIGNATURE_LEN`].
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> BusResult<Self> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(BusError::malformed("signature", "signature is empty"));
        }
        if bytes.len() > MAX_SIGNATURE_LEN {
            return Err(BusError::malformed(
                "signature",
                format!(
                    "signature is {} bytes, limit is {MAX_SIGNATURE_LEN}",
                    bytes.len()
                ),
            ));
        }
        Ok(Self(bytes))
    }

    /// Decode from lowercase or uppercase hex.
    pub fn from_hex(value: &str) -> BusResult<Self> {
        let bytes = hex::decode(value)
            .map_err(|e| BusError::malformed("signature", format!("not hex: {e}")))?;
        Self::from_bytes(bytes)
    }

    /// The raw signature bytes, exactly as the publisher supplied them.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Lowercase hex rendering.
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }

    /// Signature length, in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always `false` — the constructor rejects empty signatures. Present so
    /// that `len` has the conventional companion.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for EventSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EventSignature({} bytes, {})", self.0.len(), self.to_hex())
    }
}

impl fmt::Display for EventSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl AsRef<[u8]> for EventSignature {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl serde::Serialize for EventSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> serde::Deserialize<'de> for EventSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        EventSignature::from_hex(&value).map_err(<D::Error as serde::de::Error>::custom)
    }
}

/// Verifies a signature over an event's canonical bytes.
///
/// Implemented by the embedding application with whatever crypto stack it
/// audits. Both methods the bus calls are on [`crate::event::Event`]:
/// [`crate::event::Event::signing_bytes`] (the message) and
/// [`crate::event::Event::signature`] (the signature).
///
/// # Contract
///
/// * Return `true` only for a signature that is valid for `message` under the
///   key or keys this verifier is responsible for.
/// * Return `false` for anything else. `false` is a **fail-closed** answer: the
///   bus refuses the event ([`BusError::InvalidSignature`]).
/// * Never panic. A verifier that panics on malformed input can take down a
///   publisher; the bus cannot defend against that on the verifier's behalf.
/// * `message` is attacker-influenced only in the sense that it came from a
///   publisher; it is length-framed and domain-separated by this crate, so it
///   cannot be confused with an event on another domain.
pub trait SignatureVerifier: Send + Sync {
    /// `true` iff `signature` is valid for `message`.
    fn verify(&self, message: &[u8], signature: &[u8]) -> bool;
}

impl<F> SignatureVerifier for F
where
    F: Fn(&[u8], &[u8]) -> bool + Send + Sync,
{
    fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        self(message, signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_defaults_to_optional_and_prints() {
        assert_eq!(SignaturePolicy::default(), SignaturePolicy::Optional);
        assert_eq!(SignaturePolicy::Optional.to_string(), "optional");
        assert_eq!(SignaturePolicy::Required.to_string(), "required");
    }

    #[test]
    fn signature_rejects_empty_and_oversized_values() {
        match EventSignature::from_bytes(Vec::new()) {
            Err(BusError::Malformed { field, reason }) => {
                assert_eq!(field, "signature");
                assert!(reason.contains("empty"), "{reason}");
            }
            other => panic!("expected Malformed, got {other:?}"),
        }

        let oversized = vec![0u8; MAX_SIGNATURE_LEN + 1];
        assert!(EventSignature::from_bytes(oversized).is_err());
        assert!(EventSignature::from_bytes(vec![0u8; MAX_SIGNATURE_LEN]).is_ok());
    }

    #[test]
    fn hex_round_trip_is_exactly_the_input_bytes() {
        let raw: Vec<u8> = (0..64u8).collect();
        let signature = EventSignature::from_bytes(raw.clone()).expect("valid");
        assert_eq!(signature.len(), 64);
        assert!(!signature.is_empty());
        assert_eq!(signature.as_bytes(), raw.as_slice());
        assert_eq!(signature.as_ref(), raw.as_slice());
        assert_eq!(
            EventSignature::from_hex(&signature.to_hex()).expect("round trip"),
            signature
        );
        assert_eq!(signature.to_string(), signature.to_hex());
    }

    #[test]
    fn from_hex_rejects_garbage_and_empty() {
        assert!(EventSignature::from_hex("zz").is_err());
        assert!(EventSignature::from_hex("").is_err());
        assert!(EventSignature::from_hex("0").is_err());
    }

    #[test]
    fn json_round_trip_is_a_hex_string() {
        let signature = EventSignature::from_bytes(vec![0xabu8; 64]).expect("valid");
        let json = serde_json::to_string(&signature).expect("serializes");
        assert_eq!(json.len(), 2 + 128);
        assert_eq!(
            serde_json::from_str::<EventSignature>(&json).expect("deserializes"),
            signature
        );
        assert!(serde_json::from_str::<EventSignature>("\"\"").is_err());
    }

    #[test]
    fn a_closure_is_a_verifier() {
        let always = |_message: &[u8], _signature: &[u8]| true;
        assert!(always.verify(b"m", b"s"));

        let verifier: &dyn SignatureVerifier = &always;
        assert!(verifier.verify(b"m", b"s"));
    }

    #[test]
    fn debug_never_claims_more_than_it_knows() {
        let signature = EventSignature::from_bytes(vec![1u8, 2, 3]).expect("valid");
        let rendered = format!("{signature:?}");
        assert!(rendered.contains("3 bytes"), "{rendered}");
        assert!(!rendered.contains("valid"), "{rendered}");
    }
}
