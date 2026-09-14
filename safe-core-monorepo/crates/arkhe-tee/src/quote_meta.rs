//! Quote metadata extraction.
//!
//! Everything here goes through the *real* `dcap-qvl` 0.6.3 API:
//! `dcap_qvl::quote::Quote::parse` decodes the quote, then
//! `Report::as_td10()` / `Report::as_td15()` expose the TD report payload. There
//! is no `tdx_quote::Quote::parse` call anywhere in this crate — that crate is
//! AGPL-3.0-or-later and is deliberately not a dependency.

use serde::{Deserialize, Serialize};

use crate::error::{TeeError, TeeResult};
use crate::report_data::report_data_matches;
use crate::rtmr::RTMR_COUNT;
use crate::types::{Digest48, ReportData64};

/// `tee_type` value for an SGX enclave quote.
///
/// Mirrors `dcap-qvl`'s private `constants::TEE_TYPE_SGX` (`src/constants.rs:17`).
pub const TEE_TYPE_SGX: u32 = 0x0000_0000;

/// `tee_type` value for a TDX quote.
///
/// Mirrors `dcap-qvl`'s private `constants::TEE_TYPE_TDX` (`src/constants.rs:18`).
pub const TEE_TYPE_TDX: u32 = 0x0000_0081;

/// Which TDX report body the quote carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TdReportKind {
    /// TDX 1.0: `TDReport10`.
    Td10,
    /// TDX 1.5: `TDReport15`, whose base is a `TDReport10`.
    Td15,
}

impl TdReportKind {
    /// Lowercase identifier used in JSON output.
    pub const fn as_str(self) -> &'static str {
        match self {
            TdReportKind::Td10 => "td10",
            TdReportKind::Td15 => "td15",
        }
    }
}

/// Measurements and identity fields of a TDX quote.
///
/// `rtmrs` holds `rt_mr0..rt_mr3` in index order; `mr_service_td` is only
/// present for TDX 1.5 (`TDReport15`) quotes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TdxQuoteMetadata {
    /// Quote format version from the header (4 or 5 for TDX).
    pub quote_version: u16,
    /// `tee_type` from the header.
    pub tee_type: u32,
    /// Which report body was decoded.
    pub report_kind: TdReportKind,
    /// `TEE_TCB_SVN` of the report.
    pub tee_tcb_svn: [u8; 16],
    /// `MRTD` — the initial measurement of the TD.
    pub mr_td: Digest48,
    /// `MR_CONFIG_ID`.
    pub mr_config_id: Digest48,
    /// `MR_OWNER`.
    pub mr_owner: Digest48,
    /// `MR_OWNER_CONFIG`.
    pub mr_owner_config: Digest48,
    /// `RTMR0..RTMR3`, in index order.
    pub rtmrs: [Digest48; RTMR_COUNT as usize],
    /// The 64-byte `report_data` field.
    pub report_data: ReportData64,
    /// `MR_SERVICE_TD`, present only for TDX 1.5 quotes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mr_service_td: Option<Digest48>,
}

impl TdxQuoteMetadata {
    /// The RTMR at `index`, or `None` when out of range.
    pub fn rtmr(&self, index: u32) -> Option<Digest48> {
        self.rtmrs.get(index as usize).copied()
    }

    /// True when the quote's `report_data` is the value derived from `payload`.
    pub fn report_data_matches(&self, payload: &[u8]) -> bool {
        report_data_matches(payload, self.report_data.as_bytes())
    }

    /// True when every RTMR is still zero, i.e. nothing has been extended yet.
    pub fn all_rtmrs_zero(&self) -> bool {
        self.rtmrs.iter().all(|r| r.is_zero())
    }

    /// Hex rendering of `MRTD`.
    pub fn mr_td_hex(&self) -> String {
        self.mr_td.to_hex()
    }

    /// Hex rendering of `report_data`.
    pub fn report_data_hex(&self) -> String {
        self.report_data.to_hex()
    }
}

/// Decode a TDX quote and extract its measurements.
///
/// Returns [`TeeError::NotATdxQuote`] for SGX quotes and
/// [`TeeError::QuoteParse`] for bytes that are not a quote at all.
pub fn parse_tdx_quote(raw_quote: &[u8]) -> TeeResult<TdxQuoteMetadata> {
    // `Quote::parse` decodes v3/v4/v5, selecting TD10 or TD15 from the header
    // version and body type. (dcap-qvl 0.6.3, src/quote.rs:602.)
    let quote = dcap_qvl::quote::Quote::parse(raw_quote).map_err(TeeError::quote_parse)?;

    let version = quote.header.version;
    let tee_type = quote.header.tee_type;

    if quote.report.is_sgx() {
        return Err(TeeError::NotATdxQuote { tee_type });
    }

    let report_kind = if quote.report.as_td15().is_some() {
        TdReportKind::Td15
    } else {
        TdReportKind::Td10
    };

    // `as_td10()` also returns the base report of a TD15 quote.
    let td = quote
        .report
        .as_td10()
        .ok_or(TeeError::MissingTdReport { version })?;

    Ok(TdxQuoteMetadata {
        quote_version: version,
        tee_type,
        report_kind,
        tee_tcb_svn: td.tee_tcb_svn,
        mr_td: td.mr_td.into(),
        mr_config_id: td.mr_config_id.into(),
        mr_owner: td.mr_owner.into(),
        mr_owner_config: td.mr_owner_config.into(),
        rtmrs: [
            td.rt_mr0.into(),
            td.rt_mr1.into(),
            td.rt_mr2.into(),
            td.rt_mr3.into(),
        ],
        report_data: td.report_data.into(),
        mr_service_td: quote.report.as_td15().map(|t| t.mr_service_td.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_is_rejected_without_panicking() {
        for candidate in [
            &b""[..],
            &b"not a quote"[..],
            &[0u8; 64][..],
            &[0xffu8; 4096][..],
        ] {
            match parse_tdx_quote(candidate) {
                Err(TeeError::QuoteParse { .. }) => {}
                other => panic!("expected QuoteParse for {candidate:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn tee_type_constants_match_the_tdx_spec() {
        assert_eq!(TEE_TYPE_SGX, 0x0000_0000);
        assert_eq!(TEE_TYPE_TDX, 0x0000_0081);
    }

    #[test]
    fn report_kind_is_displayable() {
        assert_eq!(TdReportKind::Td10.as_str(), "td10");
        assert_eq!(TdReportKind::Td15.as_str(), "td15");
    }
}
