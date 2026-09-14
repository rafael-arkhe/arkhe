//! Typed topics — the first half of §5.2's `EventBus / Publisher / Subscriber`
//! contract.
//!
//! The canonical paper (DOI 10.5281/zenodo.21383201, §5.2) specifies:
//!
//! > `EventBus / Publisher / Subscriber` — typed topics (attestation
//! > completed/failed, storage stored, amendments proposed/applied, kill switch
//! > activated, credentials verified/rejected, alerts); mandatory `EventMeta`
//! > with identifiers and schema version on every event; malformed events are
//! > unpublishable.
//!
//! ## Encoding note (a declared divergence)
//!
//! The paper names the topics in **prose**; it does not fix a string syntax.
//! This crate encodes them as `lower_snake` segments joined by `.`, and pins
//! the mapping in [`CANONICAL_TOPICS`]. The prose word is the authority; the
//! spelling is this crate's choice, recorded here so it can be amended rather
//! than guessed:
//!
//! | §5.2 prose | [`Topic`] constant | string |
//! |---|---|---|
//! | attestation completed | [`ATTESTATION_COMPLETED`] | `attestation.completed` |
//! | attestation failed | [`ATTESTATION_FAILED`] | `attestation.failed` |
//! | storage stored | [`STORAGE_STORED`] | `storage.stored` |
//! | amendments proposed | [`AMENDMENTS_PROPOSED`] | `amendments.proposed` |
//! | amendments applied | [`AMENDMENTS_APPLIED`] | `amendments.applied` |
//! | kill switch activated | [`KILL_SWITCH_ACTIVATED`] | `kill_switch.activated` |
//! | credentials verified | [`CREDENTIALS_VERIFIED`] | `credentials.verified` |
//! | credentials rejected | [`CREDENTIALS_REJECTED`] | `credentials.rejected` |
//! | alerts | [`ALERTS`] | `alerts` |
//!
//! ## Forward compatibility (`T-E3`)
//!
//! A topic is *not* an enum. It is a validated string, so a topic introduced by
//! a future crate — `arkhe-physics.published`, say — is accepted by this crate's
//! parser today, subscribable today, and publishes today. Nothing in the bus
//! branches on a closed topic set, so an unknown topic cannot panic a consumer
//! and cannot be mistaken for a protocol error. [`Topic::is_canonical`] reports
//! only whether the topic is one of the nine the paper names.

use std::borrow::Cow;
use std::fmt;

use crate::error::{BusError, BusResult};

/// Longest accepted topic, in bytes.
pub const MAX_TOPIC_LEN: usize = 128;

/// Largest accepted number of `.`-separated segments.
pub const MAX_TOPIC_SEGMENTS: usize = 8;

/// `attestation.completed` — §5.2 "attestation completed".
pub const ATTESTATION_COMPLETED: Topic = Topic::borrowed("attestation.completed");
/// `attestation.failed` — §5.2 "attestation failed".
pub const ATTESTATION_FAILED: Topic = Topic::borrowed("attestation.failed");
/// `storage.stored` — §5.2 "storage stored".
pub const STORAGE_STORED: Topic = Topic::borrowed("storage.stored");
/// `amendments.proposed` — §5.2 "amendments proposed".
pub const AMENDMENTS_PROPOSED: Topic = Topic::borrowed("amendments.proposed");
/// `amendments.applied` — §5.2 "amendments applied".
pub const AMENDMENTS_APPLIED: Topic = Topic::borrowed("amendments.applied");
/// `kill_switch.activated` — §5.2 "kill switch activated".
pub const KILL_SWITCH_ACTIVATED: Topic = Topic::borrowed("kill_switch.activated");
/// `credentials.verified` — §5.2 "credentials verified".
pub const CREDENTIALS_VERIFIED: Topic = Topic::borrowed("credentials.verified");
/// `credentials.rejected` — §5.2 "credentials rejected".
pub const CREDENTIALS_REJECTED: Topic = Topic::borrowed("credentials.rejected");
/// `alerts` — §5.2 "alerts".
pub const ALERTS: Topic = Topic::borrowed("alerts");

/// The nine topic strings named by §5.2, in the paper's order.
///
/// This list is informational: the bus never filters against it. See the module
/// docs on `T-E3` forward compatibility.
pub const CANONICAL_TOPICS: [&str; 9] = [
    "attestation.completed",
    "attestation.failed",
    "storage.stored",
    "amendments.proposed",
    "amendments.applied",
    "kill_switch.activated",
    "credentials.verified",
    "credentials.rejected",
    "alerts",
];

/// A validated publish/subscribe topic.
///
/// Grammar: one or more `.`-separated segments, at most [`MAX_TOPIC_SEGMENTS`]
/// of them, at most [`MAX_TOPIC_LEN`] bytes in total; every byte must be
/// lowercase ASCII `a-z`, a digit, or `_`; no empty segment, no leading dot, no
/// trailing dot, no consecutive dots. Nothing else is accepted — in particular
/// no whitespace, no uppercase, no non-ASCII, no `/`.
///
/// The inner `Cow` is why the canonical constants above can be `const`: a
/// `Cow::Borrowed` needs no allocation and no runtime validation, while
/// [`Topic::parse`] yields an owned, equally valid value. There is no public
/// constructor that skips validation, so an invalid `Topic` cannot exist.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Topic(Cow<'static, str>);

impl Topic {
    /// Private const constructor used by the canonical constants. Kept private
    /// so that no caller can bypass [`Topic::parse`]; the canonical constants
    /// are re-validated by this module's tests.
    const fn borrowed(name: &'static str) -> Self {
        Self(Cow::Borrowed(name))
    }

    /// Validate and wrap a topic string.
    ///
    /// Unknown but well-formed topics are accepted: see the module docs.
    pub fn parse(value: impl AsRef<str>) -> BusResult<Self> {
        let value = value.as_ref();
        validate_topic(value)?;
        Ok(Self(Cow::Owned(value.to_string())))
    }

    /// The topic as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The `.`-separated segments.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('.')
    }

    /// True when this is one of the nine topics §5.2 names.
    pub fn is_canonical(&self) -> bool {
        CANONICAL_TOPICS.contains(&self.as_str())
    }

    /// Re-validate the stored string.
    ///
    /// This is a real re-check of the raw characters, not a no-op: it is the
    /// second gate applied by [`crate::meta::EventMeta::validate`] on the way
    /// into [`crate::bus::EventBus::publish`].
    pub fn validate(&self) -> BusResult<()> {
        validate_topic(&self.0)
    }
}

/// The topic grammar, in one place, so that it cannot drift between the
/// constructor and the re-check.
fn validate_topic(value: &str) -> BusResult<()> {
    if value.is_empty() {
        return Err(BusError::invalid_topic(value, "topic is empty"));
    }
    if value.len() > MAX_TOPIC_LEN {
        return Err(BusError::invalid_topic(
            value,
            format!("topic is {} bytes, limit is {MAX_TOPIC_LEN}", value.len()),
        ));
    }

    let mut dots = 0usize;
    // A segment boundary starts at the beginning of the string, so a leading
    // dot is an empty first segment.
    let mut at_segment_start = true;

    for (offset, byte) in value.bytes().enumerate() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' | b'_' => at_segment_start = false,
            b'.' => {
                if at_segment_start {
                    return Err(BusError::invalid_topic(
                        value,
                        format!("empty segment at byte offset {offset}"),
                    ));
                }
                at_segment_start = true;
                dots += 1;
            }
            _ => {
                return Err(BusError::invalid_topic(
                    value,
                    format!(
                        "byte 0x{byte:02x} at offset {offset} is not allowed \
                         (lowercase ASCII letters, digits, `_` and `.` only)"
                    ),
                ));
            }
        }
    }

    if at_segment_start {
        return Err(BusError::invalid_topic(value, "topic ends with a dot"));
    }

    let segments = dots + 1;
    if segments > MAX_TOPIC_SEGMENTS {
        return Err(BusError::invalid_topic(
            value,
            format!("topic has {segments} segments, limit is {MAX_TOPIC_SEGMENTS}"),
        ));
    }

    Ok(())
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Topic({})", self.0)
    }
}

impl AsRef<str> for Topic {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Topic {
    type Error = BusError;

    fn try_from(value: String) -> BusResult<Self> {
        Topic::parse(value)
    }
}

impl TryFrom<&str> for Topic {
    type Error = BusError;

    fn try_from(value: &str) -> BusResult<Self> {
        Topic::parse(value)
    }
}

impl From<Topic> for String {
    fn from(value: Topic) -> String {
        value.0.into_owned()
    }
}

impl serde::Serialize for Topic {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Topic {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Topic::parse(value).map_err(<D::Error as serde::de::Error>::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_constants_are_well_formed_and_agree_with_the_list() {
        assert_eq!(CANONICAL_TOPICS.len(), 9);

        let constants: [&Topic; 9] = [
            &ATTESTATION_COMPLETED,
            &ATTESTATION_FAILED,
            &STORAGE_STORED,
            &AMENDMENTS_PROPOSED,
            &AMENDMENTS_APPLIED,
            &KILL_SWITCH_ACTIVATED,
            &CREDENTIALS_VERIFIED,
            &CREDENTIALS_REJECTED,
            &ALERTS,
        ];

        for (constant, name) in constants.iter().zip(CANONICAL_TOPICS) {
            // The const constructor is private and unchecked, so the constants
            // are validated here, by the same parser callers must use.
            assert_eq!(constant.as_str(), name);
            assert_eq!(Topic::parse(name).expect("canonical topic parses"), **constant);
            assert!(constant.is_canonical());
            assert!(constant.validate().is_ok());
        }
    }

    #[test]
    fn parse_accepts_unknown_topics() {
        let future = Topic::parse("arkhe_physics.experiment.published").expect("parses");
        assert_eq!(future.segments().count(), 3);
        assert!(!future.is_canonical());
        assert_eq!(future.as_str(), "arkhe_physics.experiment.published");
    }

    #[test]
    fn parse_rejects_empty_space_unicode_upper_and_shape_violations() {
        let rejected = [
            "",
            " ",
            "attestation completed",
            "Attestation.Completed",
            ".attestation",
            "attestation.",
            "attestation..completed",
            "attestation/completed",
            "attestation:completed",
            "attestação.completed",
            "alert!",
        ];
        for value in rejected {
            match Topic::parse(value) {
                Err(BusError::InvalidTopic { topic, reason }) => {
                    assert_eq!(topic, value);
                    assert!(!reason.is_empty());
                }
                other => panic!("expected InvalidTopic for {value:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_enforces_length_and_segment_caps() {
        let long = "a".repeat(MAX_TOPIC_LEN + 1);
        assert!(Topic::parse(&long).is_err());
        assert!(Topic::parse("a".repeat(MAX_TOPIC_LEN)).is_ok());

        let deep = "a.b.c.d.e.f.g.h.i";
        assert!(Topic::parse(deep).is_err());
        let at_cap = "a.b.c.d.e.f.g.h";
        assert_eq!(
            Topic::parse(at_cap).expect("at cap parses").segments().count(),
            MAX_TOPIC_SEGMENTS
        );
    }

    #[test]
    fn single_segment_topics_are_legal() {
        assert_eq!(ALERTS.segments().count(), 1);
        assert!(Topic::parse("alerts").is_ok());
    }

    #[test]
    fn json_round_trip_is_a_plain_string() {
        let json = serde_json::to_string(&STORAGE_STORED).expect("serializes");
        assert_eq!(json, "\"storage.stored\"");
        assert_eq!(
            serde_json::from_str::<Topic>(&json).expect("deserializes"),
            STORAGE_STORED
        );
    }

    #[test]
    fn deserializing_an_invalid_topic_fails_instead_of_being_accepted() {
        let err = serde_json::from_str::<Topic>("\"Storage Stored\"");
        assert!(err.is_err(), "invalid topic must not deserialize");
    }

    #[test]
    fn conversions_and_display_agree() {
        let topic = Topic::parse("credentials.verified").expect("parses");
        assert_eq!(topic.to_string(), "credentials.verified");
        assert_eq!(format!("{topic:?}"), "Topic(credentials.verified)");
        assert_eq!(topic.as_ref(), "credentials.verified");
        assert_eq!(String::from(topic.clone()), "credentials.verified");
        assert_eq!(
            Topic::try_from(String::from("credentials.verified")).expect("try_from string"),
            topic
        );
    }
}
