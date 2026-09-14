//! Derivation of the TDX `report_data` field from an application payload.
//!
//! TDX reports carry a 64-byte `report_data` field. This crate fills it with
//! `BLAKE3("ARKHE-TEE" || payload)` in the first 32 bytes and zeros in the
//! remaining 32 bytes. The domain prefix keeps the value from being confusable
//! with any other 32-byte digest an application might place in the same field.
//!
//! Both functions here are pure: they depend on nothing but their arguments, so
//! they are fully testable without hardware.

/// Domain separation prefix mixed into `report_data`.
pub const REPORT_DATA_DOMAIN: &[u8] = b"ARKHE-TEE";

/// Width of the `report_data` field, in bytes.
pub const REPORT_DATA_LEN: usize = 64;

/// Width of the BLAKE3 digest placed at the start of `report_data`, in bytes.
pub const REPORT_DATA_DIGEST_LEN: usize = 32;

/// Derive `report_data` as `BLAKE3(REPORT_DATA_DOMAIN || payload)`, zero-padded
/// to 64 bytes.
///
/// Deterministic: equal payloads always produce equal output.
pub fn derive_report_data(payload: &[u8]) -> [u8; REPORT_DATA_LEN] {
    derive_report_data_with_domain(REPORT_DATA_DOMAIN, payload)
}

/// Like [`derive_report_data`], with a caller-supplied domain prefix.
///
/// The digest occupies bytes `0..32`; bytes `32..64` are zero.
pub fn derive_report_data_with_domain(
    domain: &[u8],
    payload: &[u8],
) -> [u8; REPORT_DATA_LEN] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(payload);
    let digest = hasher.finalize();

    let mut out = [0u8; REPORT_DATA_LEN];
    out[..REPORT_DATA_DIGEST_LEN].copy_from_slice(digest.as_bytes());
    out
}

/// True when `observed` (as read from a quote) is the `report_data` derived from
/// `payload`.
pub fn report_data_matches(payload: &[u8], observed: &[u8; REPORT_DATA_LEN]) -> bool {
    // `report_data` is public input, not a secret, so a plain comparison is
    // appropriate here — no constant-time requirement.
    derive_report_data(payload) == *observed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_deterministic() {
        let a = derive_report_data(b"agent-artifact-1");
        let b = derive_report_data(b"agent-artifact-1");
        assert_eq!(a, b);
    }

    #[test]
    fn first_32_bytes_are_the_digest_and_tail_is_zero() {
        let out = derive_report_data(b"payload");
        assert_eq!(out.len(), REPORT_DATA_LEN);

        assert!(
            out[..REPORT_DATA_DIGEST_LEN].iter().any(|b| *b != 0),
            "digest half must not be all zeros"
        );
        assert!(
            out[REPORT_DATA_DIGEST_LEN..].iter().all(|b| *b == 0),
            "tail half must be zero-padded"
        );
    }

    #[test]
    fn digest_half_matches_a_single_shot_blake3_over_domain_then_payload() {
        let payload = b"cross-check-payload";
        let out = derive_report_data(payload);

        // Independent construction: concatenate then hash once, instead of
        // feeding the hasher incrementally.
        let mut joined = Vec::with_capacity(REPORT_DATA_DOMAIN.len() + payload.len());
        joined.extend_from_slice(REPORT_DATA_DOMAIN);
        joined.extend_from_slice(payload);

        assert_eq!(&out[..REPORT_DATA_DIGEST_LEN], blake3::hash(&joined).as_bytes());
    }

    #[test]
    fn different_payloads_produce_different_report_data() {
        let a = derive_report_data(b"payload-a");
        let b = derive_report_data(b"payload-b");
        assert_ne!(a, b);
    }

    #[test]
    fn empty_payload_is_still_well_formed() {
        let out = derive_report_data(b"");
        assert!(out[..REPORT_DATA_DIGEST_LEN].iter().any(|b| *b != 0));
        assert!(out[REPORT_DATA_DIGEST_LEN..].iter().all(|b| *b == 0));
    }

    #[test]
    fn domain_is_part_of_the_commitment() {
        let payload = b"same-payload";
        let a = derive_report_data_with_domain(b"ARKHE-TEE", payload);
        let b = derive_report_data_with_domain(b"ARKHE-OTHER", payload);
        assert_ne!(a, b);
        assert_eq!(a, derive_report_data(payload));
    }

    #[test]
    fn match_helper_rejects_a_single_flipped_bit() {
        let payload = b"payload";
        let mut rd = derive_report_data(payload);
        assert!(report_data_matches(payload, &rd));

        rd[0] ^= 0x01;
        assert!(!report_data_matches(payload, &rd));

        let mut rd_tail = derive_report_data(payload);
        rd_tail[REPORT_DATA_LEN - 1] = 0xff;
        assert!(!report_data_matches(payload, &rd_tail));

        assert!(!report_data_matches(b"other-payload", &derive_report_data(payload)));
    }
}
