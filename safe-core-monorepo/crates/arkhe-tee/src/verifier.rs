//! Quote verification: collateral acquisition, cache fallback, and the real
//! `dcap-qvl` one-shot verification call.
//!
//! The flow is
//!
//! ```text
//! CollateralClient::with_default_http(pccs_url).fetch(quote)   -- live PCCS
//!         |                                             (on any failure)
//!         +--> load_newest(cache_dir)                      -- local cache
//!                     |
//!                     v
//!   dcap_qvl::verify::verify(quote, collateral, now_secs)
//! ```
//!
//! Both `CollateralClient` and `verify::verify` are `dcap-qvl` 0.6.3's actual
//! public API; there is no free-standing `get_collateral(url, quote, timeout)`
//! function and none is used here.
//!
//! Degradation is always declared: collateral that did not come from a live PCCS
//! fetch yields `degraded = true` and a source of `local-cache`, never `pccs`
//! (invariant `TEE-03`).

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

#[cfg(feature = "pccs")]
use dcap_qvl::collateral::CollateralClient;
use dcap_qvl::QuoteCollateralV3;
use serde::{Deserialize, Serialize};

use crate::collateral_cache::{
    self, CachedCollateral, CollateralSource, CollateralValidity, DEFAULT_COLLATERAL_TTL,
};
use crate::error::{TeeError, TeeResult};
use crate::quote_meta::{parse_tdx_quote, TdReportKind, TdxQuoteMetadata};
use crate::report_data::derive_report_data;
use crate::types::ReportData64;

/// Default PCCS base URL used when none is given, re-exported from `dcap-qvl`.
#[cfg(feature = "pccs")]
pub const DEFAULT_PCCS_URL: &str = dcap_qvl::PHALA_PCCS_URL;

/// Default PCCS base URL used when none is given.
///
/// Without the `pccs` feature `dcap_qvl::PHALA_PCCS_URL` is not compiled in, so
/// the same value is spelled out here; a test asserts the two agree whenever
/// both are available.
#[cfg(not(feature = "pccs"))]
pub const DEFAULT_PCCS_URL: &str = "https://pccs.phala.network";

/// Default network timeout for a collateral fetch.
pub const DEFAULT_NETWORK_TIMEOUT: Duration = Duration::from_secs(30);

/// Which collateral was used, and how trustworthy its provenance is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollateralDecision {
    /// The collateral itself, together with its fetch time and provenance.
    pub cached: CachedCollateral,
    /// Freshness classification against the configured TTL.
    pub validity: CollateralValidity,
    /// Age of the collateral at decision time.
    pub age: Duration,
    /// True whenever the collateral did not come from a live PCCS fetch.
    pub degraded: bool,
    /// Why the live PCCS fetch was not used, when it failed.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub network_error: Option<String>,
    /// Why freshly fetched collateral could not be persisted, when that failed.
    /// A cache-write failure never fails verification.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cache_write_error: Option<String>,
}

impl CollateralDecision {
    /// The collateral payload handed to the verifier.
    pub fn collateral(&self) -> &QuoteCollateralV3 {
        &self.cached.collateral
    }

    /// Provenance of the collateral used.
    pub fn source(&self) -> &CollateralSource {
        &self.cached.source
    }
}

/// Choose between a live fetch and a cached entry. Pure, so the fallback policy
/// is testable without a network or a TDX host.
///
/// A successful live fetch always wins. Otherwise the cached entry is used and
/// the decision is flagged `degraded`. With neither available the call fails
/// closed with [`TeeError::NoCollateralAvailable`].
pub fn select_collateral(
    network: Result<CachedCollateral, String>,
    cache: Option<CachedCollateral>,
    ttl: Duration,
    now_unix_us: u64,
) -> TeeResult<CollateralDecision> {
    match network {
        Ok(entry) => Ok(CollateralDecision {
            validity: entry.validity(ttl, now_unix_us),
            age: entry.age_at(now_unix_us),
            degraded: false,
            network_error: None,
            cache_write_error: None,
            cached: entry,
        }),
        Err(network_error) => match cache {
            Some(entry) => Ok(CollateralDecision {
                validity: entry.validity(ttl, now_unix_us),
                age: entry.age_at(now_unix_us),
                degraded: true,
                network_error: Some(network_error),
                cache_write_error: None,
                cached: entry,
            }),
            None => Err(TeeError::NoCollateralAvailable {
                reason: format!("live PCCS fetch failed ({network_error}) and no cached collateral was found"),
            }),
        },
    }
}

/// The outcome of a successful verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuoteVerification {
    /// True when `dcap_qvl::verify::verify` accepted the quote.
    ///
    /// This means the signature chain and the TCB policy were accepted — it does
    /// *not* mean the platform is up to date. Read [`Self::tcb_status`] for that.
    pub valid: bool,
    /// `report_data` read from the verified report.
    pub report_data: ReportData64,
    /// TCB status returned by the verifier, e.g. `UpToDate` or `OutOfDate`.
    pub tcb_status: String,
    /// Advisory IDs attached to the verification result.
    pub advisory_ids: Vec<String>,
    /// Quoting-enclave TCB status.
    pub qe_status: String,
    /// Platform TCB status.
    pub platform_status: String,
    /// Quote format version that was verified.
    pub quote_version: u16,
    /// Which TD report body the quote carried.
    pub report_kind: TdReportKind,
    /// Where the collateral came from.
    pub collateral_source: CollateralSource,
    /// Age of the collateral when the decision was made.
    pub collateral_age: Duration,
    /// Freshness classification of the collateral.
    pub collateral_validity: CollateralValidity,
    /// True when the collateral did not come from a live PCCS fetch.
    pub degraded: bool,
    /// When verification completed, in microseconds since the Unix epoch.
    pub verified_at_unix_us: u64,
}

impl QuoteVerification {
    /// True when the quote's `report_data` is the value derived from `payload`.
    pub fn report_data_matches(&self, payload: &[u8]) -> bool {
        self.report_data.as_bytes() == &derive_report_data(payload)
    }

    /// Verification time rendered as RFC 3339, when representable.
    pub fn verified_at_rfc3339(&self) -> Option<String> {
        crate::unix_us_to_rfc3339(self.verified_at_unix_us)
    }
}

/// A TDX quote verifier bound to one PCCS URL and one cache policy.
#[derive(Debug, Clone)]
pub struct TdxVerifier {
    pccs_url: String,
    cache_dir: Option<PathBuf>,
    collateral_ttl: Duration,
    network_timeout: Duration,
}

impl TdxVerifier {
    /// Build a verifier.
    ///
    /// A `network_timeout` of zero disables the network path: the fetch times out
    /// immediately and every verification is served from the cache. That is a
    /// supported way to run strictly offline.
    pub fn new(
        pccs_url: impl Into<String>,
        cache_dir: Option<PathBuf>,
        collateral_ttl: Duration,
        network_timeout: Duration,
    ) -> Self {
        Self {
            pccs_url: pccs_url.into(),
            cache_dir,
            collateral_ttl,
            network_timeout,
        }
    }

    /// A verifier with `dcap-qvl`'s default PCCS URL, a 24-hour TTL, a 30-second
    /// timeout, and no cache directory.
    pub fn production() -> Self {
        Self::new(
            DEFAULT_PCCS_URL,
            None,
            DEFAULT_COLLATERAL_TTL,
            DEFAULT_NETWORK_TIMEOUT,
        )
    }

    /// Use `dir` as the collateral cache directory.
    pub fn with_cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(dir.into());
        self
    }

    /// Override the collateral TTL.
    pub fn with_collateral_ttl(mut self, ttl: Duration) -> Self {
        self.collateral_ttl = ttl;
        self
    }

    /// Override the network timeout. Zero disables the network path.
    pub fn with_network_timeout(mut self, timeout: Duration) -> Self {
        self.network_timeout = timeout;
        self
    }

    /// PCCS base URL in use.
    pub fn pccs_url(&self) -> &str {
        &self.pccs_url
    }

    /// Configured cache directory, if any.
    pub fn cache_dir(&self) -> Option<&PathBuf> {
        self.cache_dir.as_ref()
    }

    /// Configured collateral TTL.
    pub fn collateral_ttl(&self) -> Duration {
        self.collateral_ttl
    }

    /// Configured network timeout.
    pub fn network_timeout(&self) -> Duration {
        self.network_timeout
    }

    /// True when the network path is disabled by a zero timeout.
    pub fn is_offline_forced(&self) -> bool {
        self.network_timeout.is_zero()
    }

    /// Fetch collateral straight from the PCCS.
    ///
    /// Uses `CollateralClient::with_default_http` and `CollateralClient::fetch`,
    /// wrapped in a timeout so the configured `network_timeout` is enforced
    /// (the client builds its own HTTP client with a fixed 180-second timeout
    /// that callers cannot override).
    ///
    /// Compiled only with the `pccs` feature; see [`Self::fetch_collateral`] for
    /// the offline build.
    #[cfg(feature = "pccs")]
    pub async fn fetch_collateral(
        &self,
        raw_quote: &[u8],
        now_unix_us: u64,
    ) -> TeeResult<CachedCollateral> {
        let client = CollateralClient::with_default_http(self.pccs_url.clone()).map_err(|e| {
            TeeError::CollateralFetch {
                pccs_url: self.pccs_url.clone(),
                reason: format!("could not build an HTTP client for the PCCS: {e:#}"),
            }
        })?;

        let collateral = tokio::time::timeout(self.network_timeout, client.fetch(raw_quote))
            .await
            .map_err(|_| TeeError::CollateralFetch {
                pccs_url: self.pccs_url.clone(),
                reason: format!("timed out after {:?}", self.network_timeout),
            })?
            .map_err(|e| TeeError::CollateralFetch {
                pccs_url: self.pccs_url.clone(),
                reason: format!("{e:#}"),
            })?;

        Ok(CachedCollateral::new(
            collateral,
            now_unix_us,
            CollateralSource::Pccs {
                url: self.pccs_url.clone(),
            },
        ))
    }

    /// Without the `pccs` feature the build carries no TLS stack, so the network
    /// path is unavailable. Every verification is then served from the local
    /// cache, or fails closed.
    #[cfg(not(feature = "pccs"))]
    pub async fn fetch_collateral(
        &self,
        _raw_quote: &[u8],
        _now_unix_us: u64,
    ) -> TeeResult<CachedCollateral> {
        Err(TeeError::CollateralFetch {
            pccs_url: self.pccs_url.clone(),
            reason: "this build was compiled without the `pccs` feature and therefore has no \
                     TLS stack; only the local collateral cache is available"
                .to_string(),
        })
    }

    /// Obtain collateral, preferring the PCCS and falling back to the local
    /// cache. Freshly fetched collateral is persisted best-effort.
    pub async fn acquire_collateral(&self, raw_quote: &[u8]) -> TeeResult<CollateralDecision> {
        let now_unix_us = crate::now_unix_us();

        let network = match self.fetch_collateral(raw_quote, now_unix_us).await {
            Ok(entry) => Ok(entry),
            Err(e) => Err(e.to_string()),
        };

        // A cache problem must not sink an otherwise successful live fetch, so
        // the read error is carried forward instead of propagated.
        let (cache, cache_read_error) = match &self.cache_dir {
            Some(dir) => match collateral_cache::load_newest(dir) {
                Ok(entry) => (entry, None),
                Err(e) => (None, Some(e.to_string())),
            },
            None => (None, None),
        };

        let cache_status = match (&self.cache_dir, &cache, &cache_read_error) {
            (None, _, _) => "cache_dir=<none>".to_string(),
            (Some(dir), _, Some(err)) => format!("cache_dir={} unreadable: {err}", dir.display()),
            (Some(dir), Some(_), None) => format!("cache_dir={} entry=found", dir.display()),
            (Some(dir), None, None) => format!("cache_dir={} entry=absent", dir.display()),
        };

        let mut decision = select_collateral(network, cache, self.collateral_ttl, now_unix_us)
            .map_err(|e| match e {
                TeeError::NoCollateralAvailable { reason } => TeeError::NoCollateralAvailable {
                    reason: format!("{reason}; pccs_url={}; {cache_status}", self.pccs_url),
                },
                other => other,
            })?;

        if decision.cached.source.is_pccs() {
            if let Some(dir) = &self.cache_dir {
                if let Err(e) = decision.cached.save(dir) {
                    decision.cache_write_error = Some(e.to_string());
                }
            }
        }

        Ok(decision)
    }

    /// Verify a quote against collateral obtained from the PCCS or the cache.
    pub async fn verify_async(&self, raw_quote: &[u8]) -> TeeResult<QuoteVerification> {
        // Parse first: this rejects SGX quotes and malformed input with a
        // specific error, before any collateral is requested.
        let metadata: TdxQuoteMetadata = parse_tdx_quote(raw_quote)?;

        let decision = self.acquire_collateral(raw_quote).await?;

        let verified_at_unix_us = crate::now_unix_us();
        let now_secs = verified_at_unix_us / 1_000_000;

        // Real one-shot API: dcap_qvl::verify::verify(raw_quote, collateral, now_secs).
        let report = dcap_qvl::verify::verify(raw_quote, decision.collateral(), now_secs)
            .map_err(TeeError::verification_failed)?;

        let td = report
            .report
            .as_td10()
            .ok_or(TeeError::NotATdxQuote {
                tee_type: metadata.tee_type,
            })?;
        let report_data = ReportData64::from(td.report_data);

        // The decoder used for the metadata and the verifier's own view of the
        // quote must agree; a divergence would mean verification and metadata
        // are talking about different bytes.
        if report_data != metadata.report_data {
            return Err(TeeError::internal(format!(
                "report_data divergence between the parsed quote ({}) and the verified report ({})",
                metadata.report_data.to_hex(),
                report_data.to_hex()
            )));
        }

        Ok(QuoteVerification {
            valid: true,
            report_data,
            tcb_status: report.status,
            advisory_ids: report.advisory_ids,
            qe_status: report.qe_status.status.to_string(),
            platform_status: report.platform_status.status.to_string(),
            quote_version: metadata.quote_version,
            report_kind: metadata.report_kind,
            collateral_source: decision.cached.source.clone(),
            collateral_age: decision.age,
            collateral_validity: decision.validity,
            degraded: decision.degraded,
            verified_at_unix_us,
        })
    }

    /// Verify a quote and additionally require that `report_data` is the value
    /// derived from `payload`.
    pub async fn verify_async_with_claim(
        &self,
        raw_quote: &[u8],
        payload: &[u8],
    ) -> TeeResult<QuoteVerification> {
        let verification = self.verify_async(raw_quote).await?;
        if !verification.report_data_matches(payload) {
            return Err(TeeError::ReportDataMismatch {
                expected: ReportData64::from(derive_report_data(payload)).to_hex(),
                observed: verification.report_data.to_hex(),
            });
        }
        Ok(verification)
    }

    /// Synchronous [`Self::verify_async`], driven by a private current-thread
    /// Tokio runtime.
    ///
    /// Calling this from inside an existing async runtime returns
    /// [`TeeError::NestedRuntime`] rather than panicking.
    pub fn verify(&self, raw_quote: &[u8]) -> TeeResult<QuoteVerification> {
        self.block_on(self.verify_async(raw_quote))?
    }

    /// Synchronous [`Self::verify_async_with_claim`].
    pub fn verify_with_claim(&self, raw_quote: &[u8], payload: &[u8]) -> TeeResult<QuoteVerification> {
        self.block_on(self.verify_async_with_claim(raw_quote, payload))?
    }

    /// Drive `future` on a fresh current-thread runtime.
    fn block_on<F: Future>(&self, future: F) -> TeeResult<F::Output> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(TeeError::NestedRuntime);
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| TeeError::internal(format!("could not start a tokio runtime: {e}")))?;
        Ok(runtime.block_on(future))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcap_qvl::QuoteCollateralV3;
    use std::path::Path;

    fn collateral() -> QuoteCollateralV3 {
        QuoteCollateralV3 {
            pck_crl_issuer_chain: "issuer".into(),
            root_ca_crl: vec![1],
            pck_crl: vec![2],
            tcb_info_issuer_chain: "tcb-issuer".into(),
            tcb_info: "{}".into(),
            tcb_info_signature: vec![3],
            qe_identity_issuer_chain: "qe-issuer".into(),
            qe_identity: "{}".into(),
            qe_identity_signature: vec![4],
            pck_certificate_chain: None,
        }
    }

    fn pccs_entry(fetched_at: u64) -> CachedCollateral {
        CachedCollateral::new(
            collateral(),
            fetched_at,
            CollateralSource::Pccs {
                url: "https://pccs.example".into(),
            },
        )
    }

    fn cache_entry(fetched_at: u64) -> CachedCollateral {
        CachedCollateral::new(
            collateral(),
            fetched_at,
            CollateralSource::LocalCache {
                path: PathBuf::from("/tmp/collateral-x.json"),
            },
        )
    }

    const TTL: Duration = Duration::from_secs(3600);

    #[test]
    fn a_live_fetch_wins_and_is_not_degraded() {
        let now = 10_000_000_000u64;
        let decision =
            select_collateral(Ok(pccs_entry(now)), Some(cache_entry(now)), TTL, now).unwrap();

        assert_eq!(decision.validity, CollateralValidity::Fresh);
        assert!(!decision.degraded);
        assert!(decision.source().is_pccs());
        assert_eq!(decision.age, Duration::ZERO);
        assert!(decision.network_error.is_none());
    }

    #[test]
    fn a_failed_fetch_falls_back_to_a_fresh_cache_entry() {
        let now = 10_000_000_000u64;
        let fetched_at = now - 60_000_000; // one minute ago
        let decision =
            select_collateral(Err("connection refused".into()), Some(cache_entry(fetched_at)), TTL, now)
                .unwrap();

        assert_eq!(decision.validity, CollateralValidity::LocalCache);
        assert!(decision.degraded, "cache fallback must be declared degraded");
        assert!(!decision.source().is_pccs());
        assert_eq!(decision.age, Duration::from_secs(60));
        assert_eq!(decision.network_error.as_deref(), Some("connection refused"));
    }

    #[test]
    fn a_failed_fetch_falls_back_to_a_stale_cache_entry_and_says_so() {
        let now = 10_000_000_000u64;
        let fetched_at = now - (TTL.as_micros() as u64) - 1;
        let decision =
            select_collateral(Err("timeout".into()), Some(cache_entry(fetched_at)), TTL, now).unwrap();

        assert_eq!(decision.validity, CollateralValidity::StaleCache);
        assert!(decision.degraded);
    }

    #[test]
    fn no_fetch_and_no_cache_fails_closed() {
        let now = 10_000_000_000u64;
        match select_collateral(Err("offline".into()), None, TTL, now) {
            Err(TeeError::NoCollateralAvailable { reason }) => {
                assert!(reason.contains("offline"), "{reason}");
            }
            other => panic!("expected NoCollateralAvailable, got {other:?}"),
        }
    }

    #[test]
    fn provenance_classification_is_consistent_for_every_combination() {
        let now = 10_000_000_000u64;
        let cases: [(Result<CachedCollateral, String>, Option<CachedCollateral>); 3] = [
            (Ok(pccs_entry(now)), None),
            (Err("x".into()), Some(cache_entry(now))),
            (
                Err("x".into()),
                Some(cache_entry(now - TTL.as_micros() as u64 - 1)),
            ),
        ];
        // TEE-03: `Fresh` if and only if the source is a live PCCS fetch, and
        // `degraded` if and only if it is not.
        for (network, cache) in cases {
            let decision = select_collateral(network, cache, TTL, now).unwrap();
            let from_pccs = decision.source().is_pccs();
            assert_eq!(decision.degraded, !from_pccs);
            assert_eq!(decision.validity == CollateralValidity::Fresh, from_pccs);
        }
    }

    #[test]
    fn verifier_configuration_is_inspectable() {
        let verifier = TdxVerifier::production();
        assert_eq!(verifier.pccs_url(), DEFAULT_PCCS_URL);
        assert!(verifier.cache_dir().is_none());
        assert_eq!(verifier.collateral_ttl(), DEFAULT_COLLATERAL_TTL);
        assert!(!verifier.is_offline_forced());

        let offline = verifier
            .clone()
            .with_cache_dir(Path::new("/tmp/x"))
            .with_collateral_ttl(Duration::from_secs(5))
            .with_network_timeout(Duration::ZERO);
        assert!(offline.is_offline_forced());
        assert_eq!(offline.collateral_ttl(), Duration::from_secs(5));
        assert_eq!(offline.cache_dir().map(|p| p.display().to_string()), Some("/tmp/x".into()));
    }

    #[test]
    fn a_garbage_quote_is_rejected_before_any_collateral_is_requested() {
        // The quote is decoded first, so malformed input fails as `QuoteParse`
        // without a PCCS request being made at all — the fail-closed ordering.
        let verifier = TdxVerifier::production();
        match verifier.verify(b"definitely not a quote") {
            Err(TeeError::QuoteParse { reason }) => assert!(!reason.is_empty()),
            other => panic!("expected QuoteParse, got {other:?}"),
        }
    }

    #[test]
    fn an_sgx_or_unparseable_quote_is_rejected_before_collateral_is_requested() {
        let verifier = TdxVerifier::production();
        match verifier.verify(&[0u8; 16]) {
            Err(TeeError::QuoteParse { .. }) => {}
            other => panic!("expected QuoteParse, got {other:?}"),
        }
    }

    #[cfg(feature = "pccs")]
    #[test]
    fn the_default_pccs_url_is_dcap_qvls_own_constant() {
        assert_eq!(DEFAULT_PCCS_URL, dcap_qvl::PHALA_PCCS_URL);
        assert_eq!(DEFAULT_PCCS_URL, "https://pccs.phala.network");
    }

    #[cfg(not(feature = "pccs"))]
    #[test]
    fn without_the_pccs_feature_the_network_path_fails_closed() {
        let verifier = TdxVerifier::production();
        let inner = verifier
            .block_on(verifier.fetch_collateral(&[0u8; 16], 0))
            .expect("the private runtime starts");

        match inner {
            Err(TeeError::CollateralFetch { reason, .. }) => {
                assert!(reason.contains("pccs"), "{reason}");
            }
            other => panic!("expected CollateralFetch, got {other:?}"),
        }
        assert_eq!(DEFAULT_PCCS_URL, "https://pccs.phala.network");
    }
}
