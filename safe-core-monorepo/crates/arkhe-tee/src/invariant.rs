//! Crate-local invariants.
//!
//! These are *this crate's* invariants, with its own identifier prefix. They are
//! deliberately not drawn from the repository's governed canonical ID spaces
//! (`I6xx`, `I-xx`), which would collide with identifiers owned elsewhere.
//!
//! | ID | Name | Statement |
//! |---|---|---|
//! | `TEE-01` | `report_data_shape` | `report_data` is exactly 64 bytes: `BLAKE3(domain \|\| payload)` in the first 32, zeros in the last 32, and derivation is deterministic. |
//! | `TEE-02` | `rtmr_extension_law` | An RTMR extension is `SHA-384(previous \|\| new)` over full 48-byte operands — never truncated and never reordered. |
//! | `TEE-03` | `collateral_provenance_honesty` | Collateral that did not come from a live PCCS fetch is reported as `degraded` and never classified `Fresh`. |
//! | `TEE-04` | `binding_agreement` | A quote whose `MRTD`, RTMRs or `report_data` disagree with a recorded [`ProvenanceBinding`] is rejected, naming the field that moved. |

use crate::binding::ProvenanceBinding;
use crate::error::{TeeError, TeeResult};
use crate::quote_meta::TdxQuoteMetadata;
use crate::report_data::{derive_report_data, REPORT_DATA_DIGEST_LEN, REPORT_DATA_LEN};
use crate::rtmr::{extend_digest, sha384, RTMR_DIGEST_LEN};
use crate::verifier::QuoteVerification;

/// A declared invariant of this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeeInvariant {
    /// Stable identifier, `TEE-01`..`TEE-04`.
    pub id: &'static str,
    /// Short machine-friendly name.
    pub name: &'static str,
    /// What the invariant asserts.
    pub description: &'static str,
}

/// `TEE-01` — `report_data` shape and determinism.
pub const TEE_01: TeeInvariant = TeeInvariant {
    id: "TEE-01",
    name: "report_data_shape",
    description: "report_data is 64 bytes: BLAKE3(domain || payload) in the first 32 bytes, \
                  zeros in the last 32, and the derivation is deterministic",
};

/// `TEE-02` — RTMR extension law.
pub const TEE_02: TeeInvariant = TeeInvariant {
    id: "TEE-02",
    name: "rtmr_extension_law",
    description: "an RTMR extension is SHA-384(previous || new) over two full 48-byte \
                  operands, never truncated and never reordered",
};

/// `TEE-03` — collateral provenance honesty.
pub const TEE_03: TeeInvariant = TeeInvariant {
    id: "TEE-03",
    name: "collateral_provenance_honesty",
    description: "collateral that did not come from a live PCCS fetch is reported as degraded \
                  and is never classified as Fresh",
};

/// `TEE-04` — binding agreement.
pub const TEE_04: TeeInvariant = TeeInvariant {
    id: "TEE-04",
    name: "binding_agreement",
    description: "a quote whose MRTD, RTMRs or report_data disagree with a recorded \
                  ProvenanceBinding is rejected, naming the diverging field",
};

/// Every invariant declared by this crate, in identifier order.
pub const ALL: [TeeInvariant; 4] = [TEE_01, TEE_02, TEE_03, TEE_04];

/// Look up an invariant by its identifier.
pub fn by_id(id: &str) -> Option<&'static TeeInvariant> {
    ALL.iter().find(|inv| inv.id == id)
}

/// Check `TEE-01` on freshly derived values.
pub fn check_tee_01() -> TeeResult<()> {
    let payloads: [&[u8]; 4] = [b"", b"x", b"agent-artifact", &[0u8; 33]];

    for payload in payloads {
        let derived = derive_report_data(payload);

        if derived.len() != REPORT_DATA_LEN {
            return Err(TeeError::internal(format!(
                "TEE-01: report_data is {} bytes, expected {REPORT_DATA_LEN}",
                derived.len()
            )));
        }
        if derived[REPORT_DATA_DIGEST_LEN..].iter().any(|b| *b != 0) {
            return Err(TeeError::internal(
                "TEE-01: report_data tail is not zero-padded",
            ));
        }
        if derived[..REPORT_DATA_DIGEST_LEN].iter().all(|b| *b == 0) {
            return Err(TeeError::internal(
                "TEE-01: report_data digest half is all zeros",
            ));
        }
        if derive_report_data(payload) != derived {
            return Err(TeeError::internal(
                "TEE-01: report_data derivation is not deterministic",
            ));
        }
    }
    Ok(())
}

/// Check `TEE-02` against the SHA-384 definition of an extension.
pub fn check_tee_02() -> TeeResult<()> {
    let a = [0xa5u8; RTMR_DIGEST_LEN];
    let b = [0x5au8; RTMR_DIGEST_LEN];

    let mut concatenated = [0u8; RTMR_DIGEST_LEN * 2];
    concatenated[..RTMR_DIGEST_LEN].copy_from_slice(&a);
    concatenated[RTMR_DIGEST_LEN..].copy_from_slice(&b);

    if extend_digest(&a, &b) != sha384(&[&concatenated]) {
        return Err(TeeError::internal(
            "TEE-02: extend_digest is not SHA-384(previous || new)",
        ));
    }
    if extend_digest(&a, &b) == extend_digest(&b, &a) {
        return Err(TeeError::internal(
            "TEE-02: extension is insensitive to operand order",
        ));
    }
    if extend_digest(&a, &b).len() != RTMR_DIGEST_LEN {
        return Err(TeeError::internal("TEE-02: extension output is not 48 bytes"));
    }
    Ok(())
}

/// Check `TEE-03` on a concrete verification result.
pub fn check_tee_03(verification: &QuoteVerification) -> TeeResult<()> {
    use crate::collateral_cache::CollateralValidity;

    let from_pccs = verification.collateral_source.is_pccs();

    if from_pccs != !verification.degraded {
        return Err(TeeError::internal(format!(
            "TEE-03: degraded={} disagrees with collateral source {}",
            verification.degraded,
            verification.collateral_source.describe()
        )));
    }
    if verification.collateral_validity == CollateralValidity::Fresh && !from_pccs {
        return Err(TeeError::internal(format!(
            "TEE-03: cached collateral ({}) was classified Fresh",
            verification.collateral_source.describe()
        )));
    }
    Ok(())
}

/// Check `TEE-04` by comparing a binding against fresh quote metadata.
pub fn check_tee_04(binding: &ProvenanceBinding, metadata: &TdxQuoteMetadata) -> TeeResult<()> {
    if binding.gdid.trim().is_empty() {
        return Err(TeeError::internal("TEE-04: binding has an empty gdid"));
    }
    binding.verify_against(metadata)
}

/// Run the invariants that need no external input.
pub fn check_static() -> Vec<(&'static TeeInvariant, TeeResult<()>)> {
    vec![
        (&TEE_01, check_tee_01()),
        (&TEE_02, check_tee_02()),
    ]
}

/// Run every invariant against a concrete verification and binding pair.
///
/// Failures are returned, not panicked on, so a caller can decide whether to
/// abort or to record them as evidence.
pub fn check_for(
    verification: &QuoteVerification,
    binding: &ProvenanceBinding,
    metadata: &TdxQuoteMetadata,
) -> Vec<(&'static TeeInvariant, TeeResult<()>)> {
    vec![
        (&TEE_01, check_tee_01()),
        (&TEE_02, check_tee_02()),
        (&TEE_03, check_tee_03(verification)),
        (&TEE_04, check_tee_04(binding, metadata)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collateral_cache::{
        CachedCollateral, CollateralSource, CollateralValidity,
    };
    use crate::quote_meta::{TdReportKind, TEE_TYPE_TDX};
    use crate::types::{Digest32, Digest48, ReportData64};
    use std::path::PathBuf;

    fn metadata(mr_td: [u8; 48], report_data: [u8; 64]) -> TdxQuoteMetadata {
        TdxQuoteMetadata {
            quote_version: 4,
            tee_type: TEE_TYPE_TDX,
            report_kind: TdReportKind::Td10,
            tee_tcb_svn: [0; 16],
            mr_td: mr_td.into(),
            mr_config_id: [0x10; 48].into(),
            mr_owner: [0x11; 48].into(),
            mr_owner_config: [0x12; 48].into(),
            rtmrs: [
                [0x01; 48].into(),
                [0x02; 48].into(),
                [0x03; 48].into(),
                [0x04; 48].into(),
            ],
            report_data: report_data.into(),
            mr_service_td: None,
        }
    }

    fn binding_for(meta: &TdxQuoteMetadata) -> ProvenanceBinding {
        ProvenanceBinding::from_quote_meta(
            "gdid:arkhe:invariant-test",
            Digest32::from([0x77; 32]),
            meta,
            1,
        )
        .unwrap()
    }

    fn verification_with(source: CollateralSource, degraded: bool, validity: CollateralValidity) -> QuoteVerification {
        QuoteVerification {
            valid: true,
            report_data: ReportData64::ZERO,
            tcb_status: "UpToDate".into(),
            advisory_ids: vec![],
            qe_status: "UpToDate".into(),
            platform_status: "UpToDate".into(),
            quote_version: 4,
            report_kind: TdReportKind::Td10,
            collateral_source: source,
            collateral_age: std::time::Duration::ZERO,
            collateral_validity: validity,
            degraded,
            verified_at_unix_us: 0,
        }
    }

    #[test]
    fn identifiers_are_this_crates_own_and_unique() {
        assert_eq!(ALL.len(), 4);
        for (i, inv) in ALL.iter().enumerate() {
            assert_eq!(inv.id, format!("TEE-0{}", i + 1));
            assert!(!inv.description.is_empty());
            assert!(!inv.name.is_empty());
        }
        assert_eq!(by_id("TEE-03").map(|i| i.name), Some("collateral_provenance_honesty"));
        assert!(by_id("TEE-99").is_none());
        assert!(by_id("I601").is_none(), "canonical IDs are not reused here");
    }

    #[test]
    fn static_invariants_hold() {
        for (invariant, result) in check_static() {
            if let Err(e) = result {
                panic!("{} ({}) failed: {e}", invariant.id, invariant.name);
            }
        }
    }

    #[test]
    fn tee_03_rejects_a_cached_source_reported_as_fresh() {
        let lying = verification_with(
            CollateralSource::LocalCache {
                path: PathBuf::from("/tmp/x.json"),
            },
            false,
            CollateralValidity::Fresh,
        );
        assert!(check_tee_03(&lying).is_err());

        let honest_cache = verification_with(
            CollateralSource::LocalCache {
                path: PathBuf::from("/tmp/x.json"),
            },
            true,
            CollateralValidity::LocalCache,
        );
        assert!(check_tee_03(&honest_cache).is_ok());

        let honest_stale = verification_with(
            CollateralSource::LocalCache {
                path: PathBuf::from("/tmp/x.json"),
            },
            true,
            CollateralValidity::StaleCache,
        );
        assert!(check_tee_03(&honest_stale).is_ok());

        let honest_pccs = verification_with(
            CollateralSource::Pccs {
                url: "https://pccs.example".into(),
            },
            false,
            CollateralValidity::Fresh,
        );
        assert!(check_tee_03(&honest_pccs).is_ok());
    }

    #[test]
    fn tee_03_rejects_degraded_claimed_for_a_live_fetch() {
        let confusing = verification_with(
            CollateralSource::Pccs {
                url: "https://pccs.example".into(),
            },
            true,
            CollateralValidity::Fresh,
        );
        assert!(check_tee_03(&confusing).is_err());
    }

    #[test]
    fn everything_passes_for_a_consistent_pair() {
        let meta = metadata([0xaa; 48], [0x55; 64]);
        let binding = binding_for(&meta);
        let verification = verification_with(
            CollateralSource::Pccs {
                url: "https://pccs.example".into(),
            },
            false,
            CollateralValidity::Fresh,
        );

        for (invariant, result) in check_for(&verification, &binding, &meta) {
            if let Err(e) = result {
                panic!("{} ({}) failed: {e}", invariant.id, invariant.name);
            }
        }
    }

    #[test]
    fn tee_04_catches_a_diverging_quote() {
        let meta = metadata([0xaa; 48], [0x55; 64]);
        let binding = binding_for(&meta);

        let mut moved = meta.clone();
        moved.mr_td = Digest48::from([0xbb; 48]);
        match check_tee_04(&binding, &moved) {
            Err(TeeError::BindingMismatch { field, .. }) => assert_eq!(field, "mr_td"),
            other => panic!("expected a mr_td mismatch, got {other:?}"),
        }
    }

    #[test]
    fn tee_04_rejects_an_anonymous_binding() {
        let meta = metadata([0xaa; 48], [0x55; 64]);
        let mut binding = binding_for(&meta);
        binding.gdid = "  ".into();
        match check_tee_04(&binding, &meta) {
            Err(TeeError::Internal(msg)) => assert!(msg.contains("empty gdid"), "{msg}"),
            other => panic!("expected an empty-gdid error, got {other:?}"),
        }
    }

    #[test]
    fn an_unused_cached_collateral_constructor_stays_reachable() {
        // Keeps the cache type imported in this module's test namespace so that
        // the invariant tests document where `CollateralValidity` comes from.
        let entry = CachedCollateral::new(
            dcap_qvl::QuoteCollateralV3 {
                pck_crl_issuer_chain: String::new(),
                root_ca_crl: vec![],
                pck_crl: vec![],
                tcb_info_issuer_chain: String::new(),
                tcb_info: String::new(),
                tcb_info_signature: vec![],
                qe_identity_issuer_chain: String::new(),
                qe_identity: String::new(),
                qe_identity_signature: vec![],
                pck_certificate_chain: None,
            },
            0,
            CollateralSource::Pccs {
                url: "https://pccs.example".into(),
            },
        );
        assert_eq!(
            entry.validity(std::time::Duration::from_secs(1), 0),
            CollateralValidity::Fresh
        );
    }
}
