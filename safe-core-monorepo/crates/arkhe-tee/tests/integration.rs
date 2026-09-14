//! Integration tests for `arkhe-tee`.
//!
//! Everything asserted here runs without TDX hardware, without a PCCS and
//! without network access. Quotes are synthesised locally with `scale`
//! (`parity-scale-codec`, the codec `dcap-qvl` itself decodes with), so the
//! *parsing* paths are exercised against the real `dcap_qvl` types — but no
//! signature is ever checked, because no real quote exists here.
//!
//! What is deliberately absent: any test of quote generation, RTMR extension
//! against real hardware, or collateral fetched from a real PCCS. See
//! `README.md`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use dcap_qvl::quote::{
    AuthData, AuthDataV3, AuthDataV4, CertificationData, Data, EnclaveReport, Header, Quote,
    QEReportCertificationData, Report, TDReport10, TDReport15,
};
use dcap_qvl::QuoteCollateralV3;
use scale::Encode;

use arkhe_tee::binding::ProvenanceBinding;
use arkhe_tee::collateral_cache::{
    self, CachedCollateral, CollateralSource, CollateralValidity,
};
use arkhe_tee::quote_meta::{parse_tdx_quote, TdReportKind, TEE_TYPE_SGX, TEE_TYPE_TDX};
use arkhe_tee::report_data::{derive_report_data, report_data_matches, REPORT_DATA_DIGEST_LEN};
use arkhe_tee::rtmr::{agent_artifact_digest, extend_digest};
use arkhe_tee::types::{Digest32, Digest48, ReportData64};
use arkhe_tee::verifier::{select_collateral, TdxVerifier};
use arkhe_tee::TeeError;

// ---------------------------------------------------------------------------
// Synthetic quote construction
// ---------------------------------------------------------------------------

/// `BODY_TD_REPORT15_TYPE` from `dcap-qvl`'s private `constants.rs:26`.
const BODY_TD_REPORT15_TYPE: u16 = 3;

fn header(version: u16, tee_type: u32) -> Header {
    Header {
        version,
        attestation_key_type: 2,
        tee_type,
        qe_svn: 0,
        pce_svn: 0,
        qe_vendor_id: [0u8; 16],
        user_data: [0u8; 20],
    }
}

fn td_report10(rtmrs: [[u8; 48]; 4], mr_td: [u8; 48], report_data: [u8; 64]) -> TDReport10 {
    TDReport10 {
        tee_tcb_svn: [0u8; 16],
        mr_seam: [0u8; 48],
        mr_signer_seam: [0u8; 48],
        seam_attributes: [0u8; 8],
        td_attributes: [0u8; 8],
        xfam: [0u8; 8],
        mr_td,
        mr_config_id: [0x10u8; 48],
        mr_owner: [0x11u8; 48],
        mr_owner_config: [0x12u8; 48],
        rt_mr0: rtmrs[0],
        rt_mr1: rtmrs[1],
        rt_mr2: rtmrs[2],
        rt_mr3: rtmrs[3],
        report_data,
    }
}

/// A structurally valid but cryptographically meaningless auth-data body.
fn auth_data_v3() -> AuthData {
    AuthData::V3(AuthDataV3 {
        ecdsa_signature: [0u8; 64],
        ecdsa_attestation_key: [0u8; 64],
        qe_report: [0u8; 384],
        qe_report_signature: [0u8; 64],
        qe_auth_data: Data::new(Vec::new()),
        certification_data: CertificationData {
            cert_type: 5,
            body: Data::new(Vec::new()),
        },
    })
}

/// `AuthData::V4`. `Quote::decode` selects the v4 auth-data format for quote
/// versions 4 *and* 5 (see dcap-qvl `src/quote.rs`, `decode_auth_data`), so every
/// synthetic v4/v5 quote below uses this.
fn auth_data_v4() -> AuthData {
    AuthData::V4(AuthDataV4 {
        ecdsa_signature: [0u8; 64],
        ecdsa_attestation_key: [0u8; 64],
        certification_data: CertificationData {
            cert_type: 5,
            body: Data::new(Vec::new()),
        },
        qe_report_data: QEReportCertificationData {
            qe_report: [0u8; 384],
            qe_report_signature: [0u8; 64],
            qe_auth_data: Data::new(Vec::new()),
            certification_data: CertificationData {
                cert_type: 5,
                body: Data::new(Vec::new()),
            },
        },
    })
}

fn td10_quote(rtmrs: [[u8; 48]; 4], mr_td: [u8; 48], report_data: [u8; 64]) -> Vec<u8> {
    Quote {
        header: header(4, TEE_TYPE_TDX),
        report: Report::TD10(td_report10(rtmrs, mr_td, report_data)),
        auth_data: auth_data_v4(),
    }
    .encode()
}

fn td15_quote(
    rtmrs: [[u8; 48]; 4],
    mr_td: [u8; 48],
    report_data: [u8; 64],
    mr_service_td: [u8; 48],
) -> Vec<u8> {
    Quote {
        header: header(5, TEE_TYPE_TDX),
        report: Report::TD15(TDReport15 {
            base: td_report10(rtmrs, mr_td, report_data),
            tee_tcb_svn2: [0u8; 16],
            mr_service_td,
        }),
        auth_data: auth_data_v4(),
    }
    .encode()
}

/// An SGX quote at quote version 4 (v4 auth data).
fn sgx_quote() -> Vec<u8> {
    Quote {
        header: header(4, TEE_TYPE_SGX),
        report: Report::SgxEnclave(enclave_report()),
        auth_data: auth_data_v4(),
    }
    .encode()
}

/// An SGX quote at quote version 3, the version that decodes `AuthDataV3`.
fn sgx_quote_v3() -> Vec<u8> {
    Quote {
        header: header(3, TEE_TYPE_SGX),
        report: Report::SgxEnclave(enclave_report()),
        auth_data: auth_data_v3(),
    }
    .encode()
}

fn enclave_report() -> EnclaveReport {
    EnclaveReport {
        cpu_svn: [0u8; 16],
        misc_select: 0,
        reserved1: [0u8; 28],
        attributes: [0u8; 16],
        mr_enclave: [0u8; 32],
        reserved2: [0u8; 32],
        mr_signer: [0u8; 32],
        reserved3: [0u8; 96],
        isv_prod_id: 0,
        isv_svn: 0,
        reserved4: [0u8; 60],
        report_data: [0u8; 64],
    }
}

/// The synthetic quote's `BODY_TD_REPORT15_TYPE` marker is what the encoder
/// emits; this guards the constant used above against silent drift.
#[test]
fn synthetic_quote_encoding_uses_the_documented_body_type() {
    let quote = td15_quote([[0u8; 48]; 4], [0u8; 48], [0u8; 64], [0u8; 48]);
    let version = u16::from_le_bytes([quote[0], quote[1]]);
    assert_eq!(version, 5);

    // header (8 + 4 + 2 + 2 + 2 + 16 + 20 bytes) then body_type: u16
    let body_type_offset = 2 + 2 + 4 + 2 + 2 + 16 + 20;
    let body_type = u16::from_le_bytes([
        quote[body_type_offset],
        quote[body_type_offset + 1],
    ]);
    assert_eq!(body_type, BODY_TD_REPORT15_TYPE);
}

// ---------------------------------------------------------------------------
// Quote metadata parsing
// ---------------------------------------------------------------------------

#[test]
fn a_td10_quote_round_trips_through_the_real_parser() {
    let rtmrs = [[0x01u8; 48], [0x02u8; 48], [0x03u8; 48], [0x04u8; 48]];
    let mr_td = [0xaau8; 48];
    let report_data = derive_report_data(b"agent-artifact-hash");

    let meta = parse_tdx_quote(&td10_quote(rtmrs, mr_td, report_data)).expect("parses");

    assert_eq!(meta.quote_version, 4);
    assert_eq!(meta.tee_type, TEE_TYPE_TDX);
    assert_eq!(meta.report_kind, TdReportKind::Td10);
    assert!(meta.mr_td == mr_td);
    assert!(meta.mr_config_id == [0x10u8; 48]);
    assert!(meta.mr_owner == [0x11u8; 48]);
    assert!(meta.mr_owner_config == [0x12u8; 48]);
    assert_eq!(meta.report_data.as_bytes(), &report_data);
    assert_eq!(meta.mr_td_hex(), hex::encode(mr_td));
    assert!(!meta.all_rtmrs_zero());

    for (index, expected) in rtmrs.iter().enumerate() {
        assert!(meta.rtmr(index as u32).unwrap() == *expected, "rtmr{index}");
    }
    assert!(meta.rtmr(4).is_none());
    assert!(meta.mr_service_td.is_none(), "TD10 carries no MR_SERVICE_TD");
}

#[test]
fn a_td15_quote_also_exposes_mr_service_td() {
    let rtmrs = [[0u8; 48]; 4];
    let mr_service_td = [0x5au8; 48];
    let report_data = derive_report_data(b"td15");

    let meta = parse_tdx_quote(&td15_quote(rtmrs, [0xbbu8; 48], report_data, mr_service_td))
        .expect("parses");

    assert_eq!(meta.quote_version, 5);
    assert_eq!(meta.report_kind, TdReportKind::Td15);
    assert!(meta.mr_td == [0xbbu8; 48]);
    assert!(meta.mr_service_td.unwrap() == mr_service_td);
    // `as_td10()` returns the TD15 base report, so the RTMRs are still there.
    assert!(meta.all_rtmrs_zero());
    assert_eq!(meta.report_data.as_bytes(), &report_data);
}

#[test]
fn an_sgx_quote_is_rejected_with_an_explicit_error() {
    // Quote v4 and v3 SGX quotes both decode successfully — the rejection comes
    // from the tee_type check, not from a decode failure.
    for raw in [sgx_quote(), sgx_quote_v3()] {
        match parse_tdx_quote(&raw) {
            Err(TeeError::NotATdxQuote { tee_type }) => assert_eq!(tee_type, TEE_TYPE_SGX),
            other => panic!("expected NotATdxQuote, got {other:?}"),
        }
    }
}

#[test]
fn malformed_input_is_rejected_without_panicking() {
    let good = td10_quote([[0u8; 48]; 4], [0u8; 48], [0u8; 64]);

    let mut cases: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"totally not a quote".to_vec(),
        vec![0u8; 3],
        vec![0xffu8; 4096],
        good[..good.len() / 2].to_vec(), // truncated
    ];
    // Valid header, corrupted body: version 9 is not a known quote version.
    let mut bad_version = good.clone();
    bad_version[0] = 9;
    cases.push(bad_version);

    for case in cases {
        let result = parse_tdx_quote(&case);
        assert!(result.is_err(), "expected an error for {case:?}");
        assert!(
            matches!(result, Err(TeeError::QuoteParse { .. })),
            "expected QuoteParse, got {result:?}"
        );
    }
}

#[test]
fn report_data_mismatch_is_detected_on_a_parsed_quote() {
    let claimed = b"the-payload-we-claim";
    let other = b"a-different-payload";

    let honest = parse_tdx_quote(&td10_quote([[0u8; 48]; 4], [0u8; 48], derive_report_data(claimed)))
        .expect("parses");
    assert!(honest.report_data_matches(claimed));
    assert!(!honest.report_data_matches(other));

    // A quote built for some other payload must not match our claim.
    let dishonest =
        parse_tdx_quote(&td10_quote([[0u8; 48]; 4], [0u8; 48], derive_report_data(other)))
            .expect("parses");
    assert!(!dishonest.report_data_matches(claimed));

    // Flipping a single bit in the tail of report_data must also be caught:
    // the tail is required to be zero, so this is a real structural check.
    let mut tampered = derive_report_data(claimed);
    tampered[REPORT_DATA_DIGEST_LEN] = 0x01;
    let tampered_meta = parse_tdx_quote(&td10_quote([[0u8; 48]; 4], [0u8; 48], tampered))
        .expect("parses");
    assert!(!tampered_meta.report_data_matches(claimed));
    assert!(!report_data_matches(claimed, &tampered));
}

// ---------------------------------------------------------------------------
// Verification of invalid quotes
// ---------------------------------------------------------------------------

/// A structurally complete collateral set with no valid signature anywhere.
fn unusable_collateral() -> QuoteCollateralV3 {
    QuoteCollateralV3 {
        pck_crl_issuer_chain: "-----BEGIN CERTIFICATE-----\nnot a certificate".into(),
        root_ca_crl: vec![0u8; 8],
        pck_crl: vec![0u8; 8],
        tcb_info_issuer_chain: "-----BEGIN CERTIFICATE-----\nnot a certificate".into(),
        tcb_info: "{}".into(),
        tcb_info_signature: vec![0u8; 8],
        qe_identity_issuer_chain: "-----BEGIN CERTIFICATE-----\nnot a certificate".into(),
        qe_identity: "{}".into(),
        qe_identity_signature: vec![0u8; 8],
        pck_certificate_chain: None,
    }
}

#[test]
fn dcap_qvl_returns_an_error_for_garbage_quotes_instead_of_panicking() {
    let collateral = unusable_collateral();

    for case in [
        &b""[..],
        &b"garbage"[..],
        &[0u8; 128][..],
        &[0xabu8; 1024][..],
    ] {
        let result = dcap_qvl::verify::verify(case, &collateral, 0);
        assert!(
            result.is_err(),
            "dcap_qvl::verify::verify accepted garbage input {case:?}"
        );
    }
}

#[test]
fn dcap_qvl_rejects_a_well_formed_but_unsigned_quote() {
    // The synthetic quote decodes cleanly, so this exercises the verification
    // path itself rather than the decoder: an unsigned quote must be refused.
    let quote = td10_quote([[0u8; 48]; 4], [0xccu8; 48], [0u8; 64]);
    let result = dcap_qvl::verify::verify(&quote, &unusable_collateral(), 1_700_000_000);
    match result {
        Ok(report) => panic!("unsigned quote was accepted: {report:?}"),
        Err(e) => assert!(!format!("{e:#}").is_empty()),
    }
}

// ---------------------------------------------------------------------------
// Collateral cache
// ---------------------------------------------------------------------------

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Scratch directory removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "arkhe-tee-it-{tag}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Scratch(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn collateral_with_tcb_info(tcb_info: &str) -> QuoteCollateralV3 {
    let mut collateral = unusable_collateral();
    collateral.tcb_info = tcb_info.to_string();
    collateral
}

const TCB_INFO_JSON: &str = r#"{
    "id": "TDX",
    "version": 3,
    "issueDate": "2024-01-01T00:00:00Z",
    "nextUpdate": "2030-01-01T00:00:00Z",
    "fmspc": "00906EA10000",
    "pceId": "0000",
    "tcbType": 0,
    "tcbEvaluationDataNumber": 17,
    "tcbLevels": []
}"#;

const TTL: Duration = Duration::from_secs(3600);

#[test]
fn cache_survives_a_full_write_read_json_round_trip() {
    let scratch = Scratch::new("roundtrip");
    let now = 1_700_000_000_000_000u64;

    let entry = CachedCollateral::new(
        collateral_with_tcb_info(TCB_INFO_JSON),
        now,
        CollateralSource::Pccs {
            url: "https://pccs.phala.network".into(),
        },
    );
    let written = entry.save(scratch.path()).expect("saves");
    assert!(written.exists());

    let loaded = collateral_cache::load_newest(scratch.path())
        .expect("loads")
        .expect("entry present");

    assert_eq!(loaded.collateral, entry.collateral);
    assert_eq!(loaded.fetched_at_unix_us, now);
    assert_eq!(
        loaded.validity(TTL, now),
        CollateralValidity::LocalCache,
        "a loaded entry is Fresh only if it came from the network"
    );
    assert!(loaded.is_degraded());
    assert_eq!(loaded.age_at(now), Duration::ZERO);
    assert_eq!(loaded.fmspc().as_deref(), Some("00906EA10000"));
}

#[test]
fn a_fresh_cache_entry_is_used_when_the_network_is_unavailable() {
    let scratch = Scratch::new("fresh");
    let now = 1_700_000_000_000_000u64;

    CachedCollateral::new(
        collateral_with_tcb_info(TCB_INFO_JSON),
        now - 60_000_000, // one minute old
        CollateralSource::Pccs {
            url: "https://pccs.phala.network".into(),
        },
    )
    .save(scratch.path())
    .unwrap();

    let cached = collateral_cache::load_newest(scratch.path()).unwrap().unwrap();
    let decision = select_collateral(Err("network unreachable".into()), Some(cached), TTL, now)
        .expect("decision");

    assert_eq!(decision.validity, CollateralValidity::LocalCache);
    assert!(decision.degraded);
    assert_eq!(decision.age, Duration::from_secs(60));
    assert!(decision.source().describe().starts_with("local-cache:"));
    assert!(decision.network_error.is_some());
}

#[test]
fn a_stale_cache_entry_is_still_usable_but_reported_as_stale() {
    let scratch = Scratch::new("stale");
    let now = 1_700_000_000_000_000u64;

    CachedCollateral::new(
        collateral_with_tcb_info(TCB_INFO_JSON),
        now - TTL.as_micros() as u64 - 1,
        CollateralSource::Pccs {
            url: "https://pccs.phala.network".into(),
        },
    )
    .save(scratch.path())
    .unwrap();

    let cached = collateral_cache::load_newest(scratch.path()).unwrap().unwrap();
    let decision = select_collateral(Err("timeout".into()), Some(cached), TTL, now).unwrap();

    assert_eq!(decision.validity, CollateralValidity::StaleCache);
    assert!(decision.degraded);
}

#[test]
fn an_absent_cache_yields_no_collateral_and_no_decision() {
    let scratch = Scratch::new("absent");

    assert!(collateral_cache::load_newest(scratch.path()).unwrap().is_none());

    match select_collateral(Err("offline".into()), None, TTL, 0) {
        Err(TeeError::NoCollateralAvailable { reason }) => assert!(reason.contains("offline")),
        other => panic!("expected NoCollateralAvailable, got {other:?}"),
    }
}

#[test]
fn a_corrupt_cache_is_reported_rather_than_ignored() {
    let scratch = Scratch::new("corrupt");
    std::fs::write(scratch.path().join("collateral-fmspc-broken.json"), "{ nope").unwrap();

    match collateral_cache::load_newest(scratch.path()) {
        Err(TeeError::CacheCorrupt { reason, .. }) => assert!(!reason.is_empty()),
        other => panic!("expected CacheCorrupt, got {other:?}"),
    }
}

#[test]
fn a_live_fetch_always_beats_the_cache() {
    let now = 1_700_000_000_000_000u64;
    let from_pccs = CachedCollateral::new(
        collateral_with_tcb_info(TCB_INFO_JSON),
        now,
        CollateralSource::Pccs {
            url: "https://pccs.phala.network".into(),
        },
    );
    let from_cache = CachedCollateral::new(
        collateral_with_tcb_info(TCB_INFO_JSON),
        now - 10_000_000,
        CollateralSource::LocalCache {
            path: PathBuf::from("/tmp/whatever.json"),
        },
    );

    let decision = select_collateral(Ok(from_pccs), Some(from_cache), TTL, now).unwrap();
    assert_eq!(decision.validity, CollateralValidity::Fresh);
    assert!(!decision.degraded);
    assert!(decision.source().is_pccs());
    assert!(decision.network_error.is_none());
}

#[test]
fn offline_mode_is_configurable_and_fails_closed_without_a_cache() {
    let verifier = TdxVerifier::new(
        "http://127.0.0.1:1",
        None,
        TTL,
        Duration::ZERO, // network path disabled
    );
    assert!(verifier.is_offline_forced());

    // No cache and no network: the call must fail, not invent a result.
    match verifier.verify(&td10_quote([[0u8; 48]; 4], [0u8; 48], [0u8; 64])) {
        Err(TeeError::NoCollateralAvailable { reason }) => {
            assert!(reason.contains("127.0.0.1:1"), "{reason}");
        }
        other => panic!("expected NoCollateralAvailable, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Provenance binding
// ---------------------------------------------------------------------------

#[test]
fn a_binding_round_trips_and_verifies_against_its_own_quote() {
    let rtmrs = [[0x01u8; 48], [0x02u8; 48], [0x03u8; 48], [0x04u8; 48]];
    let report_data = derive_report_data(b"agent-payload");
    let raw = td10_quote(rtmrs, [0xddu8; 48], report_data);
    let meta = parse_tdx_quote(&raw).expect("parses");

    let binding = ProvenanceBinding::from_quote_meta(
        "gdid:arkhe:integration",
        ProvenanceBinding::hash_binary(b"agent-binary"),
        &meta,
        1_700_000_000_000_000,
    )
    .expect("binds");

    let json = binding.to_json().expect("serializes");
    let back = ProvenanceBinding::from_json(&json).expect("deserializes");
    assert_eq!(back, binding);
    assert_eq!(back.digest(), binding.digest());

    assert!(binding.verify_against(&meta).is_ok());
}

#[test]
fn a_binding_rejects_a_quote_whose_rtmrs_moved() {
    let rtmrs = [[0x01u8; 48], [0x02u8; 48], [0x03u8; 48], [0x04u8; 48]];
    let report_data = derive_report_data(b"agent-payload");
    let meta = parse_tdx_quote(&td10_quote(rtmrs, [0xddu8; 48], report_data)).unwrap();

    let binding = ProvenanceBinding::from_quote_meta(
        "gdid:arkhe:integration",
        ProvenanceBinding::hash_binary(b"agent-binary"),
        &meta,
        0,
    )
    .unwrap();

    // Same quote, but RTMR2 was extended by something else.
    let mut moved_rtmrs = rtmrs;
    moved_rtmrs[2] = extend_digest(&rtmrs[2], &agent_artifact_digest(&[0x07u8; 32]));
    let moved_meta = parse_tdx_quote(&td10_quote(moved_rtmrs, [0xddu8; 48], report_data)).unwrap();

    match binding.verify_against(&moved_meta) {
        Err(TeeError::BindingMismatch { field, .. }) => assert_eq!(field, "rtmr2"),
        other => panic!("expected an rtmr2 mismatch, got {other:?}"),
    }

    // And the binding can predict that post-extension value without hardware.
    let predicted = binding
        .rtmr_after_agent_artifact_extension(2, &Digest32::from([0x07u8; 32]))
        .expect("rtmr2 is in range");
    assert!(predicted == moved_meta.rtmrs[2]);
}

#[test]
fn bound_measurement_types_serialize_as_hex() {
    let digest = Digest48::from([0x0au8; 48]);
    let report_data = ReportData64::from(derive_report_data(b"x"));
    let json = serde_json::json!({ "digest": digest, "report_data": report_data }).to_string();

    assert!(json.contains(&"0a".repeat(48)));
    assert!(!json.contains("["), "byte arrays must not serialize as number lists");
}

// ---------------------------------------------------------------------------
// RTMR driver, off Linux
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "linux"))]
#[test]
fn rtmr_extension_fails_with_a_typed_platform_error_off_linux() {
    use arkhe_tee::rtmr::{
        detect_backend, extend, extend_with_agent_artifact, read_rtmr, RTMR_DIGEST_LEN,
    };

    assert_eq!(detect_backend(), None);

    for result in [
        read_rtmr(0).map(|_| ()),
        extend(0, &[0u8; RTMR_DIGEST_LEN]).map(|_| ()),
        extend_with_agent_artifact(3, &[0u8; 32]).map(|_| ()),
    ] {
        match result {
            Err(TeeError::RtmrUnsupportedPlatform { tried }) => {
                assert!(!tried.is_empty(), "the error should name what was probed");
            }
            other => panic!("expected RtmrUnsupportedPlatform, got {other:?}"),
        }
    }

    // The digest maths is platform independent and still correct here.
    let expected = extend_digest(&[0u8; 48], &agent_artifact_digest(&[0u8; 32]));
    assert_eq!(expected.len(), RTMR_DIGEST_LEN);
}
