//! Error type for `arkhe-tee`.
//!
//! Every fallible operation in this crate fails closed: there is no path in
//! which a missing PCCS, a corrupt cache, an unreadable RTMR or a malformed
//! quote yields a successful verification result.

use std::path::PathBuf;

use thiserror::Error;

/// Result alias used throughout the crate.
pub type TeeResult<T> = Result<T, TeeError>;

/// Errors produced by `arkhe-tee`.
#[derive(Debug, Error)]
pub enum TeeError {
    /// The quote bytes could not be decoded by `dcap_qvl::quote::Quote::parse`.
    #[error("quote could not be parsed: {reason}")]
    QuoteParse {
        /// Underlying decode error, formatted with its full context chain.
        reason: String,
    },

    /// The quote is an SGX enclave quote; this crate only handles TDX.
    #[error("quote is not a TDX quote (tee_type=0x{tee_type:08x})")]
    NotATdxQuote {
        /// The `tee_type` field read from the quote header.
        tee_type: u32,
    },

    /// The quote decoded but carried no TDX report payload.
    #[error("quote version {version} carries no TDX report payload")]
    MissingTdReport {
        /// Quote format version that was decoded.
        version: u16,
    },

    /// A collateral fetch against the configured PCCS failed.
    #[error("PCCS collateral fetch failed for {pccs_url}: {reason}")]
    CollateralFetch {
        /// PCCS base URL that was contacted.
        pccs_url: String,
        /// Transport or protocol failure.
        reason: String,
    },

    /// Neither the network nor the local cache could supply collateral.
    #[error("no usable collateral: {reason}")]
    NoCollateralAvailable {
        /// Combined description of the network failure and cache lookup.
        reason: String,
    },

    /// Reading or writing a collateral cache file failed.
    #[error("collateral cache I/O failed at {path}: {source}")]
    CacheIo {
        /// Path of the cache file.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A cache file existed but did not contain a usable `CachedCollateral`.
    #[error("collateral cache at {path} is unusable: {reason}")]
    CacheCorrupt {
        /// Path of the cache file.
        path: PathBuf,
        /// Why the file was rejected.
        reason: String,
    },

    /// No cache directory was configured while a cache lookup was required.
    #[error("no collateral cache directory is configured")]
    NoCacheDirectory,

    /// `dcap_qvl::verify::verify` rejected the quote.
    #[error("quote verification failed: {reason}")]
    VerificationFailed {
        /// Reason reported by the verifier.
        reason: String,
    },

    /// The verified `report_data` did not match the locally derived value.
    #[error("report_data mismatch: expected {expected}, quote carries {observed}")]
    ReportDataMismatch {
        /// Hex of the value derived locally from the claimed payload.
        expected: String,
        /// Hex of the value carried by the quote.
        observed: String,
    },

    /// An RTMR index outside `0..RTMR_COUNT` was requested.
    #[error("RTMR index {index} is out of range (valid indices: 0..{max_inclusive})")]
    RtmrIndexOutOfRange {
        /// Requested index.
        index: u32,
        /// Highest valid index.
        max_inclusive: u32,
    },

    /// The host provides no RTMR extension backend.
    #[error("no RTMR extension backend available: {tried}")]
    RtmrUnsupportedPlatform {
        /// Description of the backends that were probed.
        tried: String,
    },

    /// An RTMR backend existed but the operation failed.
    #[error("RTMR backend {backend} failed during {op}: {reason}")]
    RtmrBackendFailure {
        /// Backend that was used.
        backend: String,
        /// Operation that failed (`read`, `write`, `ioctl`, ...).
        op: String,
        /// Failure detail.
        reason: String,
    },

    /// A quote disagrees with a previously recorded provenance binding.
    #[error("provenance binding mismatch on `{field}`: bound={expected}, observed={actual}")]
    BindingMismatch {
        /// Name of the disagreeing field.
        field: String,
        /// Value recorded in the binding.
        expected: String,
        /// Value observed in the quote.
        actual: String,
    },

    /// A supposed hash did not match its expected shape (normally a digest).
    #[error("collateral digest mismatch on `{field}`")]
    DigestMismatch {
        /// Name of the digest that did not match.
        field: String,
    },

    /// A hex string could not be decoded into fixed-width bytes.
    #[error("invalid hex for a {expected_len}-byte value: {value:?} ({reason})")]
    InvalidHex {
        /// The offending input.
        value: String,
        /// Human-readable description of the failure.
        reason: String,
        /// Number of bytes the target type requires.
        expected_len: usize,
    },

    /// The synchronous API was called from inside an existing async runtime.
    #[error(
        "synchronous verification cannot run inside an existing async runtime; \
         use `TdxVerifier::verify_async` instead"
    )]
    NestedRuntime,

    /// JSON (de)serialization failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Generic I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// An internal invariant of this crate was violated.
    #[error("internal error: {0}")]
    Internal(String),
}

impl TeeError {
    /// Convenience constructor for [`TeeError::Internal`].
    pub fn internal(msg: impl Into<String>) -> Self {
        TeeError::Internal(msg.into())
    }

    /// Convenience constructor for [`TeeError::QuoteParse`].
    pub fn quote_parse(reason: impl std::fmt::Display) -> Self {
        TeeError::QuoteParse {
            reason: format!("{reason:#}"),
        }
    }

    /// Convenience constructor for [`TeeError::VerificationFailed`].
    pub fn verification_failed(reason: impl std::fmt::Display) -> Self {
        TeeError::VerificationFailed {
            reason: format!("{reason:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_displayable_and_thread_safe() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<TeeError>();

        let e = TeeError::RtmrIndexOutOfRange {
            index: 7,
            max_inclusive: 3,
        };
        assert_eq!(
            e.to_string(),
            "RTMR index 7 is out of range (valid indices: 0..3)"
        );
    }

    #[test]
    fn quote_parse_helper_formats_the_chain() {
        let e = TeeError::quote_parse("outer");
        match e {
            TeeError::QuoteParse { reason } => assert_eq!(reason, "outer"),
            other => panic!("unexpected variant: {other:?}"),
        }
    }
}
