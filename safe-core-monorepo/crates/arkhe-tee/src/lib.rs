//! # `arkhe-tee` — TDX / RTMR attestation primitives
//!
//! This crate implements the parts of TDX attestation that can be written,
//! compiled and tested against a real quote-verification library:
//!
//! | Module | Purpose |
//! |---|---|
//! | [`report_data`] | Derive the 64-byte `report_data` value from a payload: `BLAKE3("ARKHE-TEE" \|\| payload)` zero-padded to 64 bytes. Pure and deterministic. |
//! | [`quote_meta`] | Extract `MRTD`, `MR_CONFIG_ID`, `MR_OWNER`, `MR_OWNER_CONFIG`, `RTMR0..3` and `report_data` from a quote. |
//! | [`collateral_cache`] | Cache PCCS collateral (`QuoteCollateralV3`) on disk with a TTL and explicit provenance. |
//! | [`verifier`] | Fetch collateral from a PCCS, fall back to the local cache, then verify with `dcap_qvl::verify::verify`. |
//! | [`rtmr`] | Extend RTMRs through the Linux TSM sysfs interface or `/dev/tdx-guest`. |
//! | [`binding`] | Bind a GDID + binary hash + quote measurements into a comparable record. |
//! | [`invariant`] | The crate-local invariants `TEE-01..TEE-04` with executable checks. |
//!
//! ## Dependency choice
//!
//! Verification is delegated to [`dcap_qvl`] 0.6.3 (MIT), which provides quote
//! decoding, TDX/SGX report accessors and the full DCAP verification path.
//!
//! The crate deliberately does **not** depend on `tdx-quote`, whose real license
//! is `AGPL-3.0-or-later` (verified in its `Cargo.toml`) and which would
//! contaminate this crate's `MIT OR Apache-2.0` licensing. Everything the
//! original plan wanted from it — quote v4/v5 parsing, `report_data`, the RTMRs,
//! `MRTD` — is reachable through `dcap_qvl::quote::Quote::parse` plus
//! `Report::as_td10()` / `Report::as_td15()`.
//!
//! ## Scope and honesty
//!
//! Nothing in this crate has been executed against TDX hardware. Quote
//! generation, RTMR extension and real PCCS collateral all require a TDX
//! platform; the tests exercise only the pure logic, the offline cache path, and
//! — through `dcap_qvl` — the rejection of malformed quotes. See `README.md`.

#![warn(missing_docs)]

pub mod binding;
pub mod collateral_cache;
pub mod error;
pub mod invariant;
pub mod quote_meta;
pub mod report_data;
pub mod rtmr;
pub mod types;
pub mod verifier;

pub use binding::ProvenanceBinding;
pub use collateral_cache::{
    load_newest, CachedCollateral, CollateralSource, CollateralValidity, DEFAULT_COLLATERAL_TTL,
};
pub use error::{TeeError, TeeResult};
pub use quote_meta::{
    parse_tdx_quote, TdReportKind, TdxQuoteMetadata, TEE_TYPE_SGX, TEE_TYPE_TDX,
};
pub use report_data::{
    derive_report_data, derive_report_data_with_domain, report_data_matches, REPORT_DATA_DOMAIN,
    REPORT_DATA_LEN,
};
pub use rtmr::{
    agent_artifact_digest, extend_digest, RtmrBackend, RtmrExtension, RTMR_COUNT, RTMR_DIGEST_LEN,
};
pub use types::{Digest32, Digest48, ReportData64};
pub use verifier::{
    select_collateral, CollateralDecision, QuoteVerification, TdxVerifier,
};

/// Microseconds since the Unix epoch, or `0` if the system clock is set before
/// 1970.
///
/// This is the crate's only source of wall-clock time; every time-dependent API
/// also accepts an explicit `now` so that it can be tested deterministically.
pub fn now_unix_us() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

/// Render a Unix timestamp in microseconds as an RFC 3339 / ISO 8601 string.
///
/// Returns `None` for values that do not fit a `chrono` `DateTime<Utc>`.
pub fn unix_us_to_rfc3339(unix_us: u64) -> Option<String> {
    let secs = (unix_us / 1_000_000) as i64;
    let nanos = ((unix_us % 1_000_000) * 1_000) as u32;
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nanos).map(|dt| dt.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_unix_us_looks_like_a_current_timestamp() {
        // 2020-01-01T00:00:00Z in microseconds.
        let lower = 1_577_836_800_000_000u64;
        assert!(now_unix_us() > lower);
    }

    #[test]
    fn rfc3339_rendering_is_stable() {
        assert_eq!(
            unix_us_to_rfc3339(0).as_deref(),
            Some("1970-01-01T00:00:00+00:00")
        );
        assert_eq!(
            unix_us_to_rfc3339(1_700_000_000_500_000).as_deref(),
            Some("2023-11-14T22:13:20.500+00:00")
        );
    }
}
