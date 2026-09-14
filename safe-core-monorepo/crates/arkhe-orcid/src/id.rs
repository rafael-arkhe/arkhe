//! Offline parsing and checksum validation of ORCID iDs.

use crate::error::OrcidError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Number of digits preceding the check character (hyphens aside).
const BODY_LEN: usize = 15;

/// A syntactically valid ORCID iD with a correct ISO 7064 mod 11-2 check
/// character.
///
/// The inner string is always the canonical form
/// (`XXXX-XXXX-XXXX-XXXX`, `X` uppercase), so [`Display`](fmt::Display),
/// [`OrcidId::as_str`] and the serde representation all agree — parsing
/// `0000000218250097` and serialising it yields
/// `"0000-0002-1825-0097"`.
///
/// Construction is fallible on purpose: there is no `OrcidId::new(String)`
/// that skips validation, so an `OrcidId` value *is* the proof that both
/// the format and the check character were verified.
///
/// ```
/// use arkhe_orcid::OrcidId;
///
/// let id = OrcidId::parse("0000-0002-1825-0097")?;
/// assert_eq!(id.to_string(), "0000-0002-1825-0097");
/// assert_eq!(id.to_string().len(), 19);
/// # Ok::<(), arkhe_orcid::OrcidError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrcidId(String);

impl OrcidId {
    /// Parses an ORCID iD, validating both its shape and its check
    /// character.
    ///
    /// Hyphens are optional and are ignored wherever they appear, so
    /// `0000000218250097`, `0000-0002-1825-0097` and `000-00002-18250097`
    /// all parse to the same value. The final character may be `0`-`9` or
    /// `X`/`x`, and is canonicalised to uppercase. Any other
    /// character, and any length other than 16 significant characters,
    /// is [`OrcidError::InvalidFormat`]; a well-shaped iD whose final
    /// character is not the correct ISO 7064 mod 11-2 check character is
    /// [`OrcidError::InvalidChecksum`].
    pub fn parse(s: &str) -> Result<Self, OrcidError> {
        let compact: Vec<char> = s.chars().filter(|c| *c != '-').collect();

        if compact.len() != BODY_LEN + 1 {
            return Err(OrcidError::InvalidFormat(format!(
                "'{s}' has {} significant characters after removing hyphens, expected {}",
                compact.len(),
                BODY_LEN + 1
            )));
        }

        let (body, check) = compact.split_at(BODY_LEN);
        let mut digits = Vec::with_capacity(BODY_LEN);
        for (position, c) in body.iter().enumerate() {
            let digit = c.to_digit(10).ok_or_else(|| {
                OrcidError::InvalidFormat(format!(
                    "'{s}' has non-digit character '{c}' at position {position} of the body"
                ))
            })?;
            digits.push(digit);
        }

        let expected = checksum_digit(&digits);
        let actual = match check[0] {
            'X' | 'x' => 10,
            c => c.to_digit(10).ok_or_else(|| {
                OrcidError::InvalidFormat(format!(
                    "'{s}' ends with '{c}', expected a digit or an X check character"
                ))
            })?,
        };

        if actual != expected {
            return Err(OrcidError::InvalidChecksum(format!(
                "'{s}' ends with '{}', but the ISO 7064 mod 11-2 check character for the given 15 digits is '{}'",
                check[0],
                check_character(expected)
            )));
        }

        let body: String = body.iter().collect();
        Ok(Self(format!(
            "{}-{}-{}-{}{}",
            &body[0..4],
            &body[4..8],
            &body[8..12],
            &body[12..15],
            check_character(expected)
        )))
    }

    /// The canonical, hyphenated representation. Equal to
    /// [`Display`](fmt::Display)'s output.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OrcidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for OrcidId {
    type Err = OrcidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl AsRef<str> for OrcidId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OrcidId {
    type Error = OrcidError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<OrcidId> for String {
    fn from(value: OrcidId) -> Self {
        value.0
    }
}

/// ISO 7064 mod 11-2 check character for the 15 body digits: `total` is
/// folded as `total = (total + digit) * 2`, and the check value is the
/// residue that brings it back to `1 mod 11` — `10` meaning `X`.
fn checksum_digit(body: &[u32]) -> u32 {
    let mut total = 0u32;
    for digit in body {
        total = (total + digit) * 2;
    }
    (12 - (total % 11)) % 11
}

/// Renders a check value as the character that appears in the iD.
fn check_character(value: u32) -> char {
    if value == 10 {
        'X'
    } else {
        // `checksum_digit` only ever returns 0..=10.
        char::from_digit(value, 10).unwrap_or('?')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_real_orcid_id_is_accepted() {
        // Josiah Carberry's iD, the example used throughout ORCID's docs.
        let id = OrcidId::parse("0000-0002-1825-0097").unwrap();
        assert_eq!(id.as_str(), "0000-0002-1825-0097");
        // 16 significant characters plus three hyphens.
        assert_eq!(id.as_str().len(), 19);

        // A second, unrelated real iD, to pin the algorithm rather than a
        // single expected output.
        assert!(OrcidId::parse("0000-0001-5109-3700").is_ok());
    }

    #[test]
    fn a_wrong_checksum_digit_is_rejected() {
        let err = OrcidId::parse("0000-0002-1825-0098").unwrap_err();
        assert!(matches!(err, OrcidError::InvalidChecksum(_)), "got {err:?}");
    }

    #[test]
    fn an_id_ending_in_x_checksum_is_accepted() {
        // Body 000000021825002 has check character X (residue 10).
        let id = OrcidId::parse("0000-0002-1825-002X").unwrap();
        assert_eq!(id.as_str(), "0000-0002-1825-002X");
    }

    #[test]
    fn lowercase_x_checksum_is_accepted_and_canonicalized_uppercase() {
        let id = OrcidId::parse("0000-0002-1825-002x").unwrap();
        assert_eq!(id.as_str(), "0000-0002-1825-002X");
    }

    #[test]
    fn hyphens_are_optional_on_input_but_present_in_canonical_form() {
        let bare = OrcidId::parse("0000000218250097").unwrap();
        let hyphenated = OrcidId::parse("0000-0002-1825-0097").unwrap();
        assert_eq!(bare, hyphenated);
        assert_eq!(bare.to_string(), "0000-0002-1825-0097");

        // A hyphen placed anywhere is ignored, not just in the canonical
        // 4-4-4-4 positions.
        assert_eq!(OrcidId::parse("000-00002-18250097").unwrap(), bare);
    }

    #[test]
    fn wrong_length_is_rejected() {
        for input in [
            "0000-0002-1825",
            "0000-0002-1825-009",
            "0000-0002-1825-00971",
            "",
        ] {
            let err = OrcidId::parse(input).unwrap_err();
            assert!(
                matches!(err, OrcidError::InvalidFormat(_)),
                "{input:?} gave {err:?}"
            );
        }
    }

    #[test]
    fn non_digit_characters_in_the_base_are_rejected() {
        for input in ["000A-0002-1825-0097", "0000-0002-1825-009Y"] {
            let err = OrcidId::parse(input).unwrap_err();
            assert!(
                matches!(err, OrcidError::InvalidFormat(_)),
                "{input:?} gave {err:?}"
            );
        }
    }

    #[test]
    fn format_errors_are_checked_before_the_checksum() {
        // Well-shaped but with a wrong check character, and also
        // containing a letter: the format error must win, because there
        // is nothing to run the checksum against.
        let err = OrcidId::parse("000A-0002-1825-0098").unwrap_err();
        assert!(matches!(err, OrcidError::InvalidFormat(_)), "got {err:?}");
    }

    #[test]
    fn from_str_matches_parse() {
        assert_eq!(
            "0000-0002-1825-0097".parse::<OrcidId>().unwrap(),
            OrcidId::parse("0000-0002-1825-0097").unwrap()
        );
    }

    #[test]
    fn serde_uses_the_canonical_string_form_in_both_directions() {
        let id = OrcidId::parse("0000000218250097").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""0000-0002-1825-0097""#);
        assert_eq!(serde_json::from_str::<OrcidId>(&json).unwrap(), id);

        // Deserialising garbage is an error rather than a panic or an
        // unchecked value.
        assert!(serde_json::from_str::<OrcidId>(r#""nope""#).is_err());
    }
}
