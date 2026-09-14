//! Client for the ORCID public API.

use crate::error::OrcidError;
use crate::id::OrcidId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The ORCID public API version this client targets.
///
/// Used by [`OrcidClient::new`]; tests and callers that need to point at a
/// different host (a mock server, a mirror, a proxy) use
/// [`OrcidClient::with_base_url`] instead.
pub const DEFAULT_BASE_URL: &str = "https://pub.orcid.org/v3.0";

/// A verified ORCID identity.
///
/// Produced by [`OrcidClient::verify`], which only returns one after the
/// ORCID public API answered with a person record for the requested iD.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrcidVerification {
    /// `"given-names family-name"`, the form an attribution line wants.
    /// Never empty: a response with no name at all is an error rather
    /// than a verification with a blank name.
    pub display_name: String,

    /// The iD this verification is about, in canonical form.
    pub id: OrcidId,

    /// The `given-names` field as returned, when present.
    pub given_names: Option<String>,

    /// The `family-name` field as returned, when present.
    pub family_name: Option<String>,
}

impl fmt::Display for OrcidVerification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.display_name, self.id)
    }
}

/// Talks to the ORCID public API.
///
/// Cheap to clone (the underlying `reqwest::Client` pools connections), and
/// holds no per-verification state: the same client verifies any number of
/// iDs.
///
/// ```
/// use arkhe_orcid::OrcidClient;
///
/// let client = OrcidClient::new();
/// assert_eq!(client.base_url(), "https://pub.orcid.org/v3.0");
///
/// let mock = OrcidClient::with_base_url("http://127.0.0.1:8080/");
/// assert_eq!(mock.base_url(), "http://127.0.0.1:8080");
/// ```
#[derive(Clone, Debug)]
pub struct OrcidClient {
    base_url: String,
    http: reqwest::Client,
}

impl OrcidClient {
    /// A client pointed at the ORCID public API
    /// ([`DEFAULT_BASE_URL`]), which is what production code wants.
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    /// A client pointed at `url`, with any trailing slash trimmed.
    ///
    /// This is how tests point the client at a local mock server, and how
    /// an operator points it at a caching proxy.
    pub fn with_base_url(url: impl Into<String>) -> Self {
        Self {
            base_url: url.into().trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    /// The base URL this client issues requests against, without a
    /// trailing slash.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Fetches the person record for `id` and turns it into an
    /// [`OrcidVerification`].
    ///
    /// Issues `GET {base_url}/{id}/person` with `Accept:
    /// application/json` — the iD rendered through
    /// [`Display`](fmt::Display), so the canonical hyphenated form is what
    /// travels on the wire.
    ///
    /// Status handling:
    ///
    /// | Response | Error |
    /// |---|---|
    /// | `2xx` | parsed; a body that is not JSON, or that carries no name, is [`OrcidError::UnexpectedResponse`] |
    /// | `404` | [`OrcidError::NotFound`] |
    /// | `429` | [`OrcidError::RateLimited`] |
    /// | `5xx` | [`OrcidError::Provider`] |
    /// | other non-`2xx` | [`OrcidError::UnexpectedResponse`] |
    /// | no response at all | [`OrcidError::Transport`] |
    pub async fn verify(&self, id: &OrcidId) -> Result<OrcidVerification, OrcidError> {
        let url = person_url(&self.base_url, id);

        let response = self
            .http
            .get(&url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| OrcidError::Transport(format!("GET {url}: {e}")))?;

        let status = response.status().as_u16();
        match status {
            200..=299 => {}
            404 => {
                return Err(OrcidError::NotFound {
                    orcid: id.to_string(),
                })
            }
            429 => return Err(OrcidError::RateLimited),
            500..=599 => return Err(OrcidError::Provider { status }),
            other => {
                return Err(OrcidError::UnexpectedResponse {
                    reason: format!("GET {url} answered HTTP {other}"),
                })
            }
        }

        let person: PersonResponse =
            response
                .json()
                .await
                .map_err(|e| OrcidError::UnexpectedResponse {
                    reason: format!("GET {url} did not return a JSON person record: {e}"),
                })?;

        let name = person.name.unwrap_or_default();
        let given_names = value_of(name.given_names);
        let family_name = value_of(name.family_name);

        let display_name = display_name_from(given_names.as_deref(), family_name.as_deref())
            .ok_or_else(|| OrcidError::UnexpectedResponse {
                reason: format!(
                    "the person record for {id} carries no name (given-names and family-name are both absent or empty)"
                ),
            })?;

        Ok(OrcidVerification {
            display_name,
            id: id.clone(),
            given_names,
            family_name,
        })
    }
}

impl Default for OrcidClient {
    fn default() -> Self {
        Self::new()
    }
}

/// `GET {base_url}/{id}/person`.
fn person_url(base_url: &str, id: &OrcidId) -> String {
    format!("{base_url}/{id}/person")
}

/// Joins the two name halves with a single space, dropping whichever half
/// is absent or blank. `None` when nothing is left — a record whose name
/// is entirely missing.
fn display_name_from(given_names: Option<&str>, family_name: Option<&str>) -> Option<String> {
    let parts: Vec<&str> = [given_names, family_name]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

/// ORCID wraps every scalar in an object: `{"value": "Josiah"}`.
fn value_of(field: Option<ValueField>) -> Option<String> {
    field
        .and_then(|f| f.value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// The subset of ORCID's `/person` record this crate reads.
///
/// Every field is optional: ORCID omits `name` entirely for records that
/// have not been filled in, and omits either half of the name for people
/// who go by one name.
#[derive(Debug, Default, Deserialize)]
struct PersonResponse {
    name: Option<PersonName>,
}

/// The `name` object of a person record.
#[derive(Debug, Default, Deserialize)]
struct PersonName {
    #[serde(rename = "given-names", default)]
    given_names: Option<ValueField>,

    #[serde(rename = "family-name", default)]
    family_name: Option<ValueField>,
}

/// One ORCID scalar field: `{"value": "…"}`.
#[derive(Debug, Deserialize)]
struct ValueField {
    value: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_person_url_appends_person_to_the_canonical_id() {
        let id = OrcidId::parse("0000-0002-1825-0097").unwrap();
        assert_eq!(
            person_url("https://pub.orcid.org/v3.0", &id),
            "https://pub.orcid.org/v3.0/0000-0002-1825-0097/person"
        );
    }

    #[test]
    fn a_trailing_slash_on_the_base_url_does_not_double_up() {
        let client = OrcidClient::with_base_url("http://127.0.0.1:1/");
        let id = OrcidId::parse("0000-0002-1825-0097").unwrap();
        assert_eq!(
            person_url(client.base_url(), &id),
            "http://127.0.0.1:1/0000-0002-1825-0097/person"
        );
    }

    #[test]
    fn display_name_joins_both_halves() {
        assert_eq!(
            display_name_from(Some("Josiah"), Some("Carberry")),
            Some("Josiah Carberry".to_string())
        );
    }

    #[test]
    fn display_name_keeps_whichever_half_exists() {
        assert_eq!(
            display_name_from(Some("Josiah"), None),
            Some("Josiah".to_string())
        );
        assert_eq!(
            display_name_from(None, Some("Carberry")),
            Some("Carberry".to_string())
        );
        assert_eq!(
            display_name_from(Some("  "), Some("Carberry")),
            Some("Carberry".to_string())
        );
    }

    #[test]
    fn display_name_is_none_when_there_is_no_name() {
        assert_eq!(display_name_from(None, None), None);
        assert_eq!(display_name_from(Some(""), Some("   ")), None);
    }

    #[test]
    fn verification_displays_as_name_and_id() {
        let verification = OrcidVerification {
            display_name: "Josiah Carberry".to_string(),
            id: OrcidId::parse("0000-0002-1825-0097").unwrap(),
            given_names: Some("Josiah".to_string()),
            family_name: Some("Carberry".to_string()),
        };
        assert_eq!(
            verification.to_string(),
            "Josiah Carberry (0000-0002-1825-0097)"
        );
    }
}
