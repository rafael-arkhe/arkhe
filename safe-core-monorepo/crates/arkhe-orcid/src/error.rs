//! The single error type shared by ORCID iD parsing and API verification.

use thiserror::Error;

/// Every way ORCID handling can fail.
///
/// One flat enum covers both halves of the crate on purpose: `arkhe-agi`
/// returns `OrcidError` straight out of `AgiCoordinator::attest_with_orcid`,
/// so a caller wanting to distinguish "the user mistyped the iD" from
/// "the network is down" would otherwise have to nest two error types.
///
/// The first two variants are produced by [`OrcidId::parse`](crate::OrcidId::parse)
/// and are the only ones with no network involvement:
///
/// ```
/// use arkhe_orcid::{OrcidError, OrcidId};
///
/// assert!(matches!(OrcidId::parse("0000-0002-1825"), Err(OrcidError::InvalidFormat(_))));
/// assert!(matches!(OrcidId::parse("0000-0002-1825-0098"), Err(OrcidError::InvalidChecksum(_))));
/// ```
#[derive(Debug, Error)]
pub enum OrcidError {
    /// The string is not shaped like an ORCID iD: wrong length, or a
    /// character outside `[0-9]` in the body / `[0-9Xx]` in the final
    /// position.
    #[error("invalid ORCID iD format: {0}")]
    InvalidFormat(String),

    /// The iD is well-shaped but its final character is not the ISO 7064
    /// mod 11-2 check character for the first 15 digits. This is the
    /// signature of a mistyped (or invented) identifier — a real ORCID iD
    /// can never fail here.
    #[error("invalid ORCID iD checksum: {0}")]
    InvalidChecksum(String),

    /// The ORCID public API answered `404` — the well-formed iD does not
    /// map to any registered researcher.
    #[error("no ORCID record exists for iD {orcid}")]
    NotFound {
        /// The iD that was looked up, in canonical form.
        orcid: String,
    },

    /// The ORCID public API answered `429`. The identity was not verified
    /// and the caller should back off rather than retry immediately.
    #[error("the ORCID public API rate-limited this client")]
    RateLimited,

    /// The ORCID public API answered `5xx`.
    #[error("the ORCID public API failed with HTTP {status}")]
    Provider {
        /// The HTTP status code returned by the provider.
        status: u16,
    },

    /// The response was `2xx` but did not contain what an ORCID person
    /// record must contain: valid JSON, or a name to build a display name
    /// from.
    #[error("unexpected ORCID API response: {reason}")]
    UnexpectedResponse {
        /// What was wrong with the response, in caller-readable terms.
        reason: String,
    },

    /// The request never produced an HTTP response (DNS, connection,
    /// TLS, timeout). Distinct from [`OrcidError::Provider`], which means
    /// the provider answered — and answered badly.
    #[error("could not reach the ORCID public API: {0}")]
    Transport(String),
}
