//! Constitutional **values**, their hash-addressing, and the append-only
//! governance audit trail.
//!
//! # Values versus mechanisms (paper §9.1)
//!
//! Section 9.1 of the Arkhe paper requires a *strict channel separation whereby
//! values live in the constitution (governed, versioned, hash-addressed) and
//! mechanisms live in code (audited, invariant-checked)*.
//!
//! This module is mechanism. It knows how to represent, canonicalise, version,
//! hash and verify constitutional values, but it does not contain any of them:
//! every quorum fraction and every timelock is data supplied by the caller
//! through a [`Constitution`]. The parameter values prescribed by §4.4 are
//! ordinary configuration living in [`crate::paper_defaults`], and the test
//! `mechanism_contains_no_hardcoded_paper_values` fails if any of them
//! reappears as a numeric literal inside a mechanism module.
//!
//! The separation is also structural: [`AmendmentClass`] is the only thing that
//! decides *which* parameter pair applies, and it reads that pair out of the
//! constitution it is handed rather than holding it itself.
//!
//! # Hash addressing
//!
//! [`Constitution::canonical_bytes`] is a deterministic, length-prefixed,
//! domain-separated encoding of the whole value set; [`Constitution::canonical_hash`]
//! is its BLAKE3 digest. That digest is the identity of a constitution version
//! and is what a deployment anchors on-chain (ADR-011). Changing any single
//! value changes the digest, so a vote is always a vote about a specific,
//! unambiguous set of values.

use crate::amendment::{Amendment, AmendmentId, RejectionReason};
use arkhe_core::hash::blake3_hash;
use arkhe_core::ArkheHash;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use thiserror::Error;

/// Domain separation tag for the canonical constitution encoding.
pub(crate) const CONSTITUTION_DOMAIN: &[u8] = b"arkhe.constitution.v1";

/// Domain separation tag for the append-only audit chain.
pub(crate) const AUDIT_DOMAIN: &[u8] = b"arkhe.governance.audit.v1";

/// Domain separation tag for the anchored decision record.
pub(crate) const DECISION_DOMAIN: &[u8] = b"arkhe.governance.decision.v1";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors produced by the constitutional mechanisms.
///
/// Every failure mode of the amendment flow has its own variant; no failure is
/// collapsed into a generic error and none of them is silent.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GovernanceError {
    /// The proposal did not reach the quorum required by the constitution.
    #[error(
        "quorum not met on amendment {amendment}: {approvals}/{eligible} approvals, required {required}"
    )]
    QuorumNotMet {
        /// The amendment that was checked.
        amendment: AmendmentId,
        /// Distinct approvals recorded.
        approvals: u32,
        /// Seats eligible to approve.
        eligible: u32,
        /// The quorum the constitution required.
        required: Fraction,
    },

    /// The timelock guarding the amendment has not elapsed yet.
    #[error("timelock not elapsed on amendment {amendment}: unlocks at {unlock_at}, called at {now}")]
    TimelockNotElapsed {
        /// The amendment that was checked.
        amendment: AmendmentId,
        /// Instant at which the amendment becomes applicable.
        unlock_at: DateTime<Utc>,
        /// Instant of the rejected call.
        now: DateTime<Utc>,
    },

    /// The proposal's voting window closed without the proposal reaching quorum.
    #[error("amendment {amendment} expired: voting window closed at {deadline} without quorum")]
    ProposalExpired {
        /// The amendment that expired.
        amendment: AmendmentId,
        /// End of the voting window.
        deadline: DateTime<Utc>,
    },

    /// The proposal was already applied to the constitution.
    #[error("amendment {amendment} was already applied")]
    AlreadyApplied {
        /// The amendment that was already applied.
        amendment: AmendmentId,
    },

    /// The proposal was already rejected.
    #[error("amendment {amendment} was already rejected: {reason}")]
    AlreadyRejected {
        /// The amendment that was already rejected.
        amendment: AmendmentId,
        /// Why it was rejected.
        reason: RejectionReason,
    },

    /// The amendment identifier is unknown to this store.
    #[error("unknown amendment {amendment}")]
    UnknownAmendment {
        /// The identifier that could not be resolved.
        amendment: AmendmentId,
    },

    /// A governor voted twice on the same amendment.
    #[error("governor {governor} already voted on amendment {amendment}")]
    AlreadyVoted {
        /// The amendment voted on.
        amendment: AmendmentId,
        /// The governor whose vote was duplicated.
        governor: GovernorId,
    },

    /// The voter is not seated in the council recorded in the amendment.
    #[error("governor {governor} is not seated in the council recorded in amendment {amendment}")]
    NotACouncilMember {
        /// The amendment voted on.
        amendment: AmendmentId,
        /// The governor that is not seated.
        governor: GovernorId,
    },

    /// The constitution changed after the proposal was made.
    #[error("amendment {amendment} targets constitution {expected} but the constitution in force is {actual}")]
    StaleBaseConstitution {
        /// The amendment whose base hash no longer matches.
        amendment: AmendmentId,
        /// Hexadecimal hash the amendment was proposed against.
        expected: String,
        /// Hexadecimal hash of the constitution currently in force.
        actual: String,
    },

    /// A constitutional value is not usable.
    #[error("invalid constitutional parameter '{parameter}': {reason}")]
    InvalidParameter {
        /// Name of the offending parameter.
        parameter: String,
        /// Why it is unusable.
        reason: String,
    },

    /// The append-only audit chain does not verify.
    #[error("governance audit chain is broken at entry {index}")]
    AuditChainBroken {
        /// Zero-based index of the first entry that fails verification.
        index: usize,
    },

    /// The stored constitution history does not verify.
    #[error("constitution history is broken at version index {index}")]
    ConstitutionHistoryBroken {
        /// Zero-based index of the first history entry that fails verification.
        index: usize,
    },
}

/// Builds a [`GovernanceError::InvalidParameter`].
fn invalid(parameter: &str, reason: &str) -> GovernanceError {
    GovernanceError::InvalidParameter {
        parameter: parameter.to_owned(),
        reason: reason.to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Values: governor identifiers, council, fractions, timelocks, classes
// ---------------------------------------------------------------------------

/// Identifier of a member of the ethics council.
///
/// The Arkhe workspace has no DID value type reachable from this crate (the
/// orphan `audit_policy`/`gdpr` modules referenced an `arkhe_identity::ArkheDid`
/// that does not exist), so a governor is identified by its textual
/// representation. The type is deliberately opaque so that it can be swapped for
/// a real DID type later without touching the mechanisms.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GovernorId(String);

impl GovernorId {
    /// Wraps a textual governor identifier.
    pub fn new(identifier: impl Into<String>) -> Self {
        Self(identifier.into())
    }

    /// Returns the textual identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GovernorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for GovernorId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// The set of governors eligible to vote, as recorded in the constitution.
///
/// Membership is a constitutional *value*: it is versioned and covered by the
/// constitution hash, and it can only change through the amendment flow.
/// The set is normalised (sorted, de-duplicated) on construction and on
/// deserialisation so that two councils with the same members always have the
/// same canonical encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<GovernorId>", into = "Vec<GovernorId>")]
pub struct Council {
    members: Vec<GovernorId>,
}

impl Council {
    /// Builds a normalised council from any iterator of governor identifiers.
    pub fn new<I>(members: I) -> Self
    where
        I: IntoIterator<Item = GovernorId>,
    {
        let mut members: Vec<GovernorId> = members.into_iter().collect();
        members.sort();
        members.dedup();
        Self { members }
    }

    /// Returns the seats, in canonical order.
    pub fn members(&self) -> &[GovernorId] {
        &self.members
    }

    /// Returns the number of seats, saturating at [`u32::MAX`].
    ///
    /// Saturation is fail-closed: an unrepresentable council size makes the
    /// quorum threshold unreachable rather than trivially reachable.
    pub fn size(&self) -> u32 {
        u32::try_from(self.members.len()).unwrap_or(u32::MAX)
    }

    /// Returns the number of seats as a `usize`.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Returns `true` when the council has no seats.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Returns `true` when `governor` holds a seat.
    pub fn contains(&self, governor: &GovernorId) -> bool {
        self.members.binary_search(governor).is_ok()
    }
}

impl From<Vec<GovernorId>> for Council {
    fn from(members: Vec<GovernorId>) -> Self {
        Self::new(members)
    }
}

impl From<Council> for Vec<GovernorId> {
    fn from(council: Council) -> Self {
        council.members
    }
}

/// An exact quorum fraction, held as an integer numerator over a denominator.
///
/// Quorums are never evaluated in floating point: `4/7` and `5/7` are exactly
/// representable here, whereas `f64` cannot represent either exactly and would
/// make the boundary comparison depend on rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fraction {
    /// Approvals required.
    pub numerator: u32,
    /// Seats counted.
    pub denominator: u32,
}

impl Fraction {
    /// Builds a fraction. Validity is checked by [`Fraction::is_valid`] and by
    /// [`Constitution::validate`], not here, so that malformed values can be
    /// reported as typed errors instead of panicking.
    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    /// Returns the numerator.
    pub const fn numerator(&self) -> u32 {
        self.numerator
    }

    /// Returns the denominator.
    pub const fn denominator(&self) -> u32 {
        self.denominator
    }

    /// Returns `true` when the fraction is usable as a quorum.
    ///
    /// A fraction is usable when the denominator is non-zero, the numerator is
    /// non-zero (a zero numerator would make every proposal pass with no votes
    /// at all, which is fail-open) and the numerator does not exceed the
    /// denominator.
    pub const fn is_valid(&self) -> bool {
        self.denominator > 0 && self.numerator > 0 && self.numerator <= self.denominator
    }

    /// Returns `true` when `approvals` out of `eligible` satisfy this quorum.
    ///
    /// # Boundary semantics
    ///
    /// The comparison is **inclusive** (`>=`): a proposal carrying *exactly* the
    /// quorum satisfies it. With a `4/7` quorum and an eligible council of
    /// seven, four approvals satisfy and three do not.
    ///
    /// Invalid fractions are never satisfied (fail-closed).
    pub const fn satisfied_by(&self, approvals: u32, eligible: u32) -> bool {
        if !self.is_valid() {
            return false;
        }
        let left = (approvals as u128) * (self.denominator as u128);
        let right = (self.numerator as u128) * (eligible as u128);
        left >= right
    }

    /// Returns the smallest number of approvals that satisfies this quorum for
    /// a council of `eligible` seats, i.e. `ceil(eligible * numerator / denominator)`.
    ///
    /// Invalid fractions return [`u32::MAX`], which is unreachable in practice
    /// and therefore fail-closed.
    pub const fn threshold_for(&self, eligible: u32) -> u32 {
        if !self.is_valid() {
            return u32::MAX;
        }
        let scaled = (eligible as u128) * (self.numerator as u128);
        let divisor = self.denominator as u128;
        let threshold = scaled.div_ceil(divisor);
        if threshold > u32::MAX as u128 {
            u32::MAX
        } else {
            threshold as u32
        }
    }
}

impl fmt::Display for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}

/// Number of seconds in one day, used only to express timelocks in days.
pub const SECONDS_PER_DAY: i64 = 86_400;

/// A duration that a change must remain publicly visible before it can apply.
///
/// Section 9.1 of the paper: the timelock converts governance capture from an
/// instantaneous attack into one that must *survive at least seven days of
/// public visibility, alerting, and counter-action*. The duration itself is a
/// constitutional value; this type only carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timelock {
    /// The duration in seconds.
    pub seconds: i64,
}

impl Timelock {
    /// Builds a timelock from a number of seconds.
    pub const fn from_seconds(seconds: i64) -> Self {
        Self { seconds }
    }

    /// Builds a timelock from a number of days.
    pub const fn from_days(days: i64) -> Self {
        Self {
            seconds: days * SECONDS_PER_DAY,
        }
    }

    /// Returns the duration in seconds.
    pub const fn seconds(&self) -> i64 {
        self.seconds
    }

    /// Returns `true` when the timelock actually delays anything.
    pub const fn is_positive(&self) -> bool {
        self.seconds > 0
    }

    /// Returns the instant at which something started at `start` becomes
    /// applicable.
    pub fn deadline_from(&self, start: DateTime<Utc>) -> DateTime<Utc> {
        start + ChronoDuration::seconds(self.seconds)
    }
}

impl fmt::Display for Timelock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}s", self.seconds)
    }
}

/// How much scrutiny an amendment requires.
///
/// The class is a mechanism input: it selects *which* pair of constitutional
/// values governs the amendment. It does not carry any value of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmendmentClass {
    /// An ordinary amendment (paper P-G1 quorum, P-G2 timelock).
    Common,
    /// A critical amendment — kill-switch or TEE configuration (paper P-G3).
    Critical,
}

impl AmendmentClass {
    /// Stable machine-readable label.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Critical => "critical",
        }
    }
}

impl fmt::Display for AmendmentClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ---------------------------------------------------------------------------
// The constitution
// ---------------------------------------------------------------------------

/// The machine-readable constitution: the complete set of values the governance
/// mechanisms read.
///
/// Every field is a value, not a mechanism. Nothing in this crate hard-codes
/// one: the values shipped with the paper are constructed in
/// [`crate::paper_defaults`] and every caller is free to supply different ones.
///
/// The fields are public so that a constitution can be written down declaratively
/// (and read back through `serde`). Invariants are checked by
/// [`Constitution::validate`], which [`ConstitutionStore::new`] and every
/// proposal run before a value is put into force.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constitution {
    /// Monotonic version number, starting at one.
    pub version: u32,
    /// The council eligible to vote.
    pub council: Council,
    /// Quorum required to amend the constitution (paper P-G1).
    pub amendment_quorum: Fraction,
    /// Timelock attached to an ordinary amendment (paper P-G2).
    pub amendment_timelock: Timelock,
    /// Quorum required for a critical amendment (paper P-G3).
    pub critical_quorum: Fraction,
    /// Timelock attached to a critical amendment (paper P-G3).
    pub critical_timelock: Timelock,
    /// Quorum required to trigger the kill-switch (paper P-S1).
    pub kill_switch_quorum: Fraction,
    /// How long a proposal may stay open before it is rejected for lack of
    /// quorum (deny-on-timeout).
    pub proposal_window: Timelock,
}

impl Constitution {
    /// Checks every value and returns a typed error for the first that is
    /// unusable, so that a malformed constitution can never be put into force.
    pub fn validate(&self) -> Result<(), GovernanceError> {
        if self.version == 0 {
            return Err(invalid("version", "version numbers start at one"));
        }
        if self.council.is_empty() {
            return Err(invalid("council", "the council must have at least one seat"));
        }
        validate_fraction("amendment_quorum", self.amendment_quorum)?;
        validate_fraction("critical_quorum", self.critical_quorum)?;
        validate_fraction("kill_switch_quorum", self.kill_switch_quorum)?;
        validate_timelock("amendment_timelock", self.amendment_timelock)?;
        validate_timelock("critical_timelock", self.critical_timelock)?;
        validate_timelock("proposal_window", self.proposal_window)?;
        Ok(())
    }

    /// Returns the quorum and timelock that govern `class`, read from this
    /// constitution.
    ///
    /// This is the entire "which values apply" mechanism: it selects a pair of
    /// values, it never contains one.
    pub fn parameters_for(&self, class: AmendmentClass) -> (Fraction, Timelock) {
        match class {
            AmendmentClass::Common => (self.amendment_quorum, self.amendment_timelock),
            AmendmentClass::Critical => (self.critical_quorum, self.critical_timelock),
        }
    }

    /// Returns the deterministic canonical encoding of every value in this
    /// constitution.
    ///
    /// The encoding is domain-separated and every value is length-prefixed, so
    /// no two different value sets can produce the same bytes. It does not
    /// depend on `serde` field order or on any map iteration order (the council
    /// is kept sorted), so the same values always encode to the same bytes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut encoder = CanonicalEncoder::new(CONSTITUTION_DOMAIN);
        encoder.text("version");
        encoder.u32(self.version);
        encoder.text("council");
        encoder.u32(self.council.size());
        for member in self.council.members() {
            encoder.text(member.as_str());
        }
        encoder.text("amendment-quorum");
        encoder.u32(self.amendment_quorum.numerator);
        encoder.u32(self.amendment_quorum.denominator);
        encoder.text("amendment-timelock");
        encoder.i64(self.amendment_timelock.seconds);
        encoder.text("critical-quorum");
        encoder.u32(self.critical_quorum.numerator);
        encoder.u32(self.critical_quorum.denominator);
        encoder.text("critical-timelock");
        encoder.i64(self.critical_timelock.seconds);
        encoder.text("kill-switch-quorum");
        encoder.u32(self.kill_switch_quorum.numerator);
        encoder.u32(self.kill_switch_quorum.denominator);
        encoder.text("proposal-window");
        encoder.i64(self.proposal_window.seconds);
        encoder.finish()
    }

    /// Returns the BLAKE3 digest of [`Constitution::canonical_bytes`].
    ///
    /// This is the identity of the value set, and the digest that a deployment
    /// anchors on-chain (ADR-011). Two constitutions with the same values share
    /// a hash; changing any single value changes it.
    pub fn canonical_hash(&self) -> ArkheHash {
        blake3_hash(&self.canonical_bytes())
    }
}

/// Checks a quorum fraction, reporting a typed error when it is unusable.
fn validate_fraction(parameter: &str, fraction: Fraction) -> Result<(), GovernanceError> {
    if fraction.denominator == 0 {
        return Err(invalid(parameter, "the denominator must be non-zero"));
    }
    if fraction.numerator == 0 {
        return Err(invalid(
            parameter,
            "a zero numerator would let a change pass with no approvals",
        ));
    }
    if fraction.numerator > fraction.denominator {
        return Err(invalid(
            parameter,
            "the numerator cannot exceed the denominator",
        ));
    }
    Ok(())
}

/// Checks a timelock, reporting a typed error when it is unusable.
fn validate_timelock(parameter: &str, timelock: Timelock) -> Result<(), GovernanceError> {
    if !timelock.is_positive() {
        return Err(invalid(
            parameter,
            "the duration must be strictly positive; a change without visibility is not a timelock",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Canonical encoding
// ---------------------------------------------------------------------------

/// A length-prefixed, domain-separated byte encoder.
///
/// Every value is written as `<u64 big-endian length><bytes>`, so the encoding
/// is unambiguous: no two distinct token sequences can produce the same output.
/// This is what makes the hash a hash *of the values* rather than of a
/// serialisation format.
#[derive(Debug, Default, Clone)]
pub(crate) struct CanonicalEncoder {
    buffer: Vec<u8>,
}

impl CanonicalEncoder {
    /// Starts an encoding tagged with a domain separation string.
    pub(crate) fn new(domain: &[u8]) -> Self {
        let mut encoder = Self::default();
        encoder.blob(domain);
        encoder
    }

    /// Writes a raw byte string, length-prefixed.
    pub(crate) fn blob(&mut self, value: &[u8]) {
        self.buffer
            .extend_from_slice(&(value.len() as u64).to_be_bytes());
        self.buffer.extend_from_slice(value);
    }

    /// Writes a textual token, length-prefixed.
    pub(crate) fn text(&mut self, value: &str) {
        self.blob(value.as_bytes());
    }

    /// Writes a boolean.
    pub(crate) fn flag(&mut self, value: bool) {
        self.blob(&[u8::from(value)]);
    }

    /// Writes a 32-bit unsigned integer, big-endian.
    pub(crate) fn u32(&mut self, value: u32) {
        self.blob(&value.to_be_bytes());
    }

    /// Writes a 64-bit unsigned integer, big-endian.
    pub(crate) fn u64(&mut self, value: u64) {
        self.blob(&value.to_be_bytes());
    }

    /// Writes a 64-bit signed integer, big-endian.
    pub(crate) fn i64(&mut self, value: i64) {
        self.blob(&value.to_be_bytes());
    }

    /// Writes an Arkhe hash.
    pub(crate) fn hash(&mut self, value: &ArkheHash) {
        self.blob(value);
    }

    /// Returns the encoded bytes.
    pub(crate) fn finish(self) -> Vec<u8> {
        self.buffer
    }
}

// ---------------------------------------------------------------------------
// Append-only audit trail
// ---------------------------------------------------------------------------

/// A governance event captured in the audit trail.
///
/// Every step of the amendment flow emits one of these: a proposal, a vote, an
/// application, a rejection or an expiry. Nothing in the flow can happen
/// without leaving a record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEvent {
    /// The constitution was sealed when the store was created.
    Sealed {
        /// Canonical hash of the sealed constitution.
        constitution: ArkheHash,
        /// Version number of the sealed constitution.
        version: u32,
    },
    /// An amendment was proposed.
    Proposed {
        /// The new amendment.
        amendment: AmendmentId,
        /// How much scrutiny it requires.
        class: AmendmentClass,
        /// Canonical hash of the constitution it amends.
        base: ArkheHash,
        /// Canonical hash of the value set it would put into force.
        proposed: ArkheHash,
    },
    /// A governor cast a vote.
    Voted {
        /// The amendment voted on.
        amendment: AmendmentId,
        /// Who voted.
        voter: GovernorId,
        /// Whether the vote approved the amendment.
        approve: bool,
        /// Distinct approvals after this vote.
        approvals: u32,
        /// Distinct rejections after this vote.
        rejections: u32,
        /// Seats eligible to vote.
        eligible: u32,
        /// Whether the quorum was satisfied after this vote.
        quorum_met: bool,
    },
    /// The amendment was applied and the constitution superseded.
    Applied {
        /// The amendment that was applied.
        amendment: AmendmentId,
        /// Canonical hash of the decision, to be anchored on-chain.
        decision: ArkheHash,
        /// Canonical hash of the constitution that was superseded.
        previous_constitution: ArkheHash,
        /// Canonical hash of the constitution now in force.
        new_constitution: ArkheHash,
    },
    /// The amendment was rejected.
    Rejected {
        /// The amendment that was rejected.
        amendment: AmendmentId,
        /// Why it was rejected.
        reason: RejectionReason,
    },
    /// The amendment's voting window closed without quorum (deny-on-timeout).
    Expired {
        /// The amendment that expired.
        amendment: AmendmentId,
    },
}

impl AuditEvent {
    /// Writes the event into a canonical encoder.
    pub(crate) fn encode(&self, encoder: &mut CanonicalEncoder) {
        match self {
            Self::Sealed {
                constitution,
                version,
            } => {
                encoder.text("sealed");
                encoder.hash(constitution);
                encoder.u32(*version);
            }
            Self::Proposed {
                amendment,
                class,
                base,
                proposed,
            } => {
                encoder.text("proposed");
                encoder.u64(amendment.raw());
                encoder.text(class.label());
                encoder.hash(base);
                encoder.hash(proposed);
            }
            Self::Voted {
                amendment,
                voter,
                approve,
                approvals,
                rejections,
                eligible,
                quorum_met,
            } => {
                encoder.text("voted");
                encoder.u64(amendment.raw());
                encoder.text(voter.as_str());
                encoder.flag(*approve);
                encoder.u32(*approvals);
                encoder.u32(*rejections);
                encoder.u32(*eligible);
                encoder.flag(*quorum_met);
            }
            Self::Applied {
                amendment,
                decision,
                previous_constitution,
                new_constitution,
            } => {
                encoder.text("applied");
                encoder.u64(amendment.raw());
                encoder.hash(decision);
                encoder.hash(previous_constitution);
                encoder.hash(new_constitution);
            }
            Self::Rejected { amendment, reason } => {
                encoder.text("rejected");
                encoder.u64(amendment.raw());
                encoder.text(reason.label());
            }
            Self::Expired { amendment } => {
                encoder.text("expired");
                encoder.u64(amendment.raw());
            }
        }
    }
}

/// One link in the append-only audit chain.
///
/// The fields are public so that the trail is readable and machine-consumable.
/// The trail is *not* mutable through this type: entries are only produced by
/// [`AuditLog::append`], and any edit to one invalidates the chain, which
/// [`AuditLog::verify`] detects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Position in the chain, starting at zero.
    pub sequence: u64,
    /// Hash of the previous entry, or `None` for the chain root.
    pub previous_hash: Option<ArkheHash>,
    /// Hash over every other field of this entry.
    pub current_hash: ArkheHash,
    /// The governance event.
    pub event: AuditEvent,
    /// When the event was recorded.
    pub timestamp: DateTime<Utc>,
}

/// An append-only, hash-linked record of everything governance did.
///
/// There is no removal and no editing: [`AuditLog::append`] is the only way to
/// add an entry, each entry commits to its predecessor's hash, and
/// [`AuditLog::verify`] recomputes the whole chain.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

impl AuditLog {
    /// Appends an event and returns the hash of the new entry.
    pub(crate) fn append(&mut self, event: AuditEvent, at: DateTime<Utc>) -> ArkheHash {
        let sequence = self.entries.len() as u64;
        let previous_hash = self.entries.last().map(|entry| entry.current_hash);
        let current_hash = chain_hash(sequence, previous_hash, &event, at);
        self.entries.push(AuditEntry {
            sequence,
            previous_hash,
            current_hash,
            event,
            timestamp: at,
        });
        current_hash
    }

    /// Rebuilds a log from entries.
    ///
    /// Like `Deserialize`, this constructor does not validate anything: it is
    /// the caller's job to call [`AuditLog::verify`] afterwards. Integrity of a
    /// trail is established by verification, not by the constructor, which is
    /// what makes tampering detectable rather than merely impossible to write
    /// through this API.
    pub fn from_entries(entries: Vec<AuditEntry>) -> Self {
        Self { entries }
    }

    /// Returns the entries, oldest first.
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when nothing has been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the hash of the newest entry, if any.
    pub fn head_hash(&self) -> Option<ArkheHash> {
        self.entries.last().map(|entry| entry.current_hash)
    }

    /// Recomputes the whole chain and reports the first entry that does not
    /// verify.
    ///
    /// A tampered event, a tampered timestamp, a re-ordered entry or a removed
    /// entry all fail here.
    pub fn verify(&self) -> Result<(), GovernanceError> {
        let mut previous: Option<ArkheHash> = None;
        for (index, entry) in self.entries.iter().enumerate() {
            let expected_sequence = index as u64;
            if entry.sequence != expected_sequence || entry.previous_hash != previous {
                return Err(GovernanceError::AuditChainBroken { index });
            }
            let recomputed =
                chain_hash(entry.sequence, entry.previous_hash, &entry.event, entry.timestamp);
            if recomputed != entry.current_hash {
                return Err(GovernanceError::AuditChainBroken { index });
            }
            previous = Some(entry.current_hash);
        }
        Ok(())
    }
}

/// Computes the hash of one audit entry.
fn chain_hash(
    sequence: u64,
    previous_hash: Option<ArkheHash>,
    event: &AuditEvent,
    at: DateTime<Utc>,
) -> ArkheHash {
    let mut encoder = CanonicalEncoder::new(AUDIT_DOMAIN);
    encoder.text("sequence");
    encoder.u64(sequence);
    encoder.text("previous");
    match previous_hash {
        Some(hash) => encoder.hash(&hash),
        None => encoder.text("root"),
    }
    encoder.text("recorded-at-micros");
    encoder.i64(at.timestamp_micros());
    event.encode(&mut encoder);
    blake3_hash(&encoder.finish())
}

// ---------------------------------------------------------------------------
// The store
// ---------------------------------------------------------------------------

/// One version of the constitution, together with the hash that addresses it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstitutionVersion {
    version: u32,
    hash: ArkheHash,
    constitution: Constitution,
}

impl ConstitutionVersion {
    /// Returns the version number.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Returns the canonical hash of this version's values.
    pub fn hash(&self) -> ArkheHash {
        self.hash
    }

    /// Returns the values themselves.
    pub fn constitution(&self) -> &Constitution {
        &self.constitution
    }
}

/// The constitution currently in force, its version history, its amendments and
/// its audit trail.
///
/// The store is the only authority over constitutional values: values enter it
/// through [`ConstitutionStore::new`] and can afterwards change only through
/// [`ConstitutionStore::apply`], which enforces the proposal → quorum → timelock
/// sequence. Every version that ever entered the store is kept, hash-addressed,
/// in [`ConstitutionStore::history`].
///
/// The amendment flow itself lives in [`crate::amendment`], which extends this
/// type; the fields are crate-visible so that the flow can record into the same
/// audit trail.
#[derive(Debug, Clone)]
pub struct ConstitutionStore {
    /// The constitution in force.
    pub(crate) constitution: Constitution,
    /// Every version that has been in force, oldest first.
    pub(crate) history: Vec<ConstitutionVersion>,
    /// Append-only audit trail.
    pub(crate) audit: AuditLog,
    /// Amendments, open and closed.
    pub(crate) amendments: BTreeMap<AmendmentId, Amendment>,
    /// Identifier the next proposal will receive.
    pub(crate) next_amendment_id: u64,
}

impl ConstitutionStore {
    /// Seals a constitution and opens its audit trail.
    ///
    /// `at` is the instant the constitution is sealed; it anchors the first
    /// audit entry. The constitution is validated first, so an unusable value
    /// set can never enter the store.
    pub fn new(constitution: Constitution, at: DateTime<Utc>) -> Result<Self, GovernanceError> {
        constitution.validate()?;
        let mut store = Self {
            constitution,
            history: Vec::new(),
            audit: AuditLog::default(),
            amendments: BTreeMap::new(),
            next_amendment_id: 0,
        };
        store.record_version();
        let sealed = store.constitution.canonical_hash();
        let version = store.constitution.version;
        store
            .audit
            .append(AuditEvent::Sealed { constitution: sealed, version }, at);
        Ok(store)
    }

    /// Returns the constitution in force.
    pub fn constitution(&self) -> &Constitution {
        &self.constitution
    }

    /// Returns the canonical hash of the constitution in force.
    ///
    /// This is the value a deployment anchors on-chain (ADR-011) and the value
    /// every amendment is proposed against.
    pub fn canonical_hash(&self) -> ArkheHash {
        self.constitution.canonical_hash()
    }

    /// Returns every version that has been in force, oldest first.
    pub fn history(&self) -> &[ConstitutionVersion] {
        &self.history
    }

    /// Returns the append-only audit trail.
    pub fn audit_log(&self) -> &AuditLog {
        &self.audit
    }

    /// Returns an amendment by identifier.
    pub fn amendment(&self, id: AmendmentId) -> Option<&Amendment> {
        self.amendments.get(&id)
    }

    /// Returns the amendments currently awaiting quorum or timelock.
    pub fn open_amendments(&self) -> Vec<&Amendment> {
        self.amendments
            .values()
            .filter(|amendment| amendment.is_open())
            .collect()
    }

    /// Verifies the whole store: the audit chain, the version history, and the
    /// link between the values in force and the newest recorded version.
    pub fn verify(&self) -> Result<(), GovernanceError> {
        self.audit.verify()?;

        let mut previous_version = 0;
        for (index, entry) in self.history.iter().enumerate() {
            if entry.constitution.canonical_hash() != entry.hash
                || entry.version != entry.constitution.version
                || entry.version <= previous_version
            {
                return Err(GovernanceError::ConstitutionHistoryBroken { index });
            }
            previous_version = entry.version;
        }

        match self.history.last() {
            Some(newest) if newest.hash == self.canonical_hash() => Ok(()),
            Some(_) => Err(GovernanceError::ConstitutionHistoryBroken {
                index: self.history.len(),
            }),
            None => Err(GovernanceError::ConstitutionHistoryBroken { index: 0 }),
        }
    }

    /// Records the constitution in force as a new, hash-addressed version.
    pub(crate) fn record_version(&mut self) {
        self.history.push(ConstitutionVersion {
            version: self.constitution.version,
            hash: self.constitution.canonical_hash(),
            constitution: self.constitution.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paper_defaults::paper_defaults;
    use chrono::TimeZone;
    use std::collections::BTreeSet;

    fn instant(day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, day, hour, 0, 0)
            .single()
            .expect("the test timestamps are valid")
    }

    fn rebuilt(constitution: &Constitution) -> Constitution {
        let encoded = serde_json::to_vec(constitution).expect("constitutions serialise");
        serde_json::from_slice(&encoded).expect("constitutions deserialise")
    }

    // -- provenance of the values -------------------------------------------

    #[test]
    fn paper_defaults_match_section_4_4() {
        let constitution = paper_defaults();

        assert_eq!(constitution.amendment_quorum, Fraction::new(4, 7), "P-G1");
        assert_eq!(
            constitution.amendment_timelock,
            Timelock::from_days(7),
            "P-G2"
        );
        assert_eq!(constitution.critical_quorum, Fraction::new(5, 7), "P-G3");
        assert_eq!(constitution.critical_timelock, Timelock::from_days(14), "P-G3");
        assert_eq!(
            constitution.kill_switch_quorum,
            Fraction::new(4, 7),
            "P-S1"
        );
        assert_eq!(constitution.council.len(), 7, "4/7 of a seven-seat council");
        assert!(
            constitution.validate().is_ok(),
            "the §4.4 defaults must be usable"
        );
    }

    #[test]
    fn constitutional_parameters_are_configurable() {
        // Nothing in the mechanism survives a value being replaced: a
        // constitution with entirely different values validates and hashes.
        let mut constitution = paper_defaults();
        constitution.amendment_quorum = Fraction::new(2, 3);
        constitution.amendment_timelock = Timelock::from_days(1);
        constitution.critical_quorum = Fraction::new(9, 10);
        constitution.critical_timelock = Timelock::from_seconds(90);
        constitution.council = Council::new([GovernorId::new("a"), GovernorId::new("b")]);
        constitution.kill_switch_quorum = Fraction::new(1, 2);
        constitution.proposal_window = Timelock::from_days(2);

        assert!(constitution.validate().is_ok());
        assert_eq!(constitution.parameters_for(AmendmentClass::Common).0, Fraction::new(2, 3));
        assert_eq!(
            constitution.parameters_for(AmendmentClass::Critical).1,
            Timelock::from_seconds(90)
        );
    }

    // -- hash addressing ----------------------------------------------------

    #[test]
    fn canonical_hash_is_stable_for_equal_values() {
        let first = paper_defaults();
        let second = paper_defaults();

        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
        assert_eq!(first.canonical_hash(), second.canonical_hash());

        // A round trip through serde preserves the values, so it preserves the
        // hash as well.
        assert_eq!(rebuilt(&first).canonical_hash(), first.canonical_hash());
    }

    #[test]
    fn canonical_hash_changes_when_one_value_changes() {
        let reference = paper_defaults();
        let mut variants: Vec<(&str, Constitution)> = Vec::new();

        let mut version = reference.clone();
        version.version += 1;
        variants.push(("version", version));

        let mut council = reference.clone();
        let mut members = council.council.members().to_vec();
        members.push(GovernorId::new("ethics-council-seat-extra"));
        council.council = Council::new(members);
        variants.push(("council", council));

        let mut quorum = reference.clone();
        quorum.amendment_quorum = Fraction::new(4, 8);
        variants.push(("amendment_quorum", quorum));

        let mut timelock = reference.clone();
        timelock.amendment_timelock = Timelock::from_days(8);
        variants.push(("amendment_timelock", timelock));

        let mut critical_quorum = reference.clone();
        critical_quorum.critical_quorum = Fraction::new(6, 7);
        variants.push(("critical_quorum", critical_quorum));

        let mut critical_timelock = reference.clone();
        critical_timelock.critical_timelock = Timelock::from_days(15);
        variants.push(("critical_timelock", critical_timelock));

        let mut kill_switch = reference.clone();
        kill_switch.kill_switch_quorum = Fraction::new(3, 7);
        variants.push(("kill_switch_quorum", kill_switch));

        let mut window = reference.clone();
        window.proposal_window = Timelock::from_days(8);
        variants.push(("proposal_window", window));

        for (name, variant) in variants {
            assert_ne!(
                variant.canonical_hash(),
                reference.canonical_hash(),
                "changing {name} must change the canonical hash"
            );
        }
    }

    #[test]
    fn council_order_does_not_change_the_hash() {
        let mut reordered = paper_defaults();
        let mut members = reordered.council.members().to_vec();
        members.reverse();
        reordered.council = Council::new(members);

        assert_eq!(
            reordered.canonical_hash(),
            paper_defaults().canonical_hash()
        );
    }

    #[test]
    fn a_deserialised_council_is_normalised() {
        // A duplicated, unsorted member list denotes the same council, so it
        // must deserialise to the normalised one and hash the same.
        let constitution = paper_defaults();
        let members = constitution.council.members().to_vec();
        let mut scrambled = members.clone();
        scrambled.extend(members.iter().cloned());
        scrambled.reverse();

        let mut encoded = serde_json::to_value(&constitution).expect("constitutions serialise");
        encoded["council"] = serde_json::to_value(&scrambled).expect("councils serialise");
        let restored: Constitution =
            serde_json::from_value(encoded).expect("constitutions deserialise");

        assert_eq!(restored.council.members(), members.as_slice());
        assert_eq!(restored.council, constitution.council);
        assert_eq!(restored.canonical_hash(), constitution.canonical_hash());
    }

    #[test]
    fn canonical_hash_covers_the_council_membership() {
        let mut other = paper_defaults();
        other.council = Council::new([
            GovernorId::new("seat-a"),
            GovernorId::new("seat-b"),
        ]);

        assert_eq!(other.council.len(), 2);
        assert_ne!(other.canonical_hash(), paper_defaults().canonical_hash());
    }

    // -- store and history --------------------------------------------------

    #[test]
    fn store_rejects_invalid_parameters() {
        let mut zero_denominator = paper_defaults();
        zero_denominator.amendment_quorum = Fraction::new(4, 0);
        assert!(matches!(
            ConstitutionStore::new(zero_denominator, instant(1, 0)),
            Err(GovernanceError::InvalidParameter { .. })
        ));

        let mut zero_numerator = paper_defaults();
        zero_numerator.critical_quorum = Fraction::new(0, 7);
        assert!(matches!(
            ConstitutionStore::new(zero_numerator, instant(1, 0)),
            Err(GovernanceError::InvalidParameter { .. })
        ));

        let mut numerator_over_denominator = paper_defaults();
        numerator_over_denominator.kill_switch_quorum = Fraction::new(8, 7);
        assert!(matches!(
            ConstitutionStore::new(numerator_over_denominator, instant(1, 0)),
            Err(GovernanceError::InvalidParameter { .. })
        ));

        let mut zero_timelock = paper_defaults();
        zero_timelock.amendment_timelock = Timelock::from_seconds(0);
        assert!(matches!(
            ConstitutionStore::new(zero_timelock, instant(1, 0)),
            Err(GovernanceError::InvalidParameter { .. })
        ));

        let mut empty_council = paper_defaults();
        empty_council.council = Council::new(Vec::new());
        assert!(matches!(
            ConstitutionStore::new(empty_council, instant(1, 0)),
            Err(GovernanceError::InvalidParameter { .. })
        ));

        let mut zero_version = paper_defaults();
        zero_version.version = 0;
        assert!(matches!(
            ConstitutionStore::new(zero_version, instant(1, 0)),
            Err(GovernanceError::InvalidParameter { .. })
        ));
    }

    #[test]
    fn store_seals_the_genesis_version_and_verifies() {
        let store = ConstitutionStore::new(paper_defaults(), instant(1, 0)).expect("valid");

        assert_eq!(store.history().len(), 1);
        assert_eq!(store.history()[0].version(), 1);
        assert_eq!(store.history()[0].hash(), store.canonical_hash());
        assert_eq!(
            store.history()[0].constitution().canonical_hash(),
            store.canonical_hash()
        );
        assert_eq!(
            store.audit_log().entries()[0].event,
            AuditEvent::Sealed {
                constitution: store.canonical_hash(),
                version: 1
            }
        );
        assert!(store.verify().is_ok());
    }

    #[test]
    fn store_detects_a_forged_version_history() {
        let mut store = ConstitutionStore::new(paper_defaults(), instant(1, 0)).expect("valid");
        store.history[0].hash = [9u8; 32];

        assert!(matches!(
            store.verify(),
            Err(GovernanceError::ConstitutionHistoryBroken { .. })
        ));
    }

    // -- append-only audit chain -------------------------------------------

    #[test]
    fn audit_chain_is_hash_linked_and_verifiable() {
        let mut log = AuditLog::default();
        let first = log.append(
            AuditEvent::Sealed {
                constitution: [1u8; 32],
                version: 1,
            },
            instant(1, 0),
        );
        let second = log.append(
            AuditEvent::Expired {
                amendment: AmendmentId::from_raw(0),
            },
            instant(1, 1),
        );

        assert_eq!(log.len(), 2);
        assert_eq!(log.head_hash(), Some(second));
        assert_eq!(log.entries()[0].previous_hash, None);
        assert_eq!(log.entries()[1].previous_hash, Some(first));
        assert_ne!(first, second);
        assert!(log.verify().is_ok());
    }

    #[test]
    fn audit_chain_is_deterministic() {
        let event = AuditEvent::Voted {
            amendment: AmendmentId::from_raw(0),
            voter: GovernorId::new("seat-a"),
            approve: true,
            approvals: 1,
            rejections: 0,
            eligible: 7,
            quorum_met: false,
        };
        let mut first = AuditLog::default();
        let mut second = AuditLog::default();

        assert_eq!(
            first.append(event.clone(), instant(2, 0)),
            second.append(event, instant(2, 0))
        );
    }

    #[test]
    fn audit_chain_detects_a_tampered_event() {
        let store = ConstitutionStore::new(paper_defaults(), instant(1, 0)).expect("valid");
        let mut entries = store.audit_log().entries().to_vec();
        entries[0].event = AuditEvent::Expired {
            amendment: AmendmentId::from_raw(0),
        };

        let forged = AuditLog::from_entries(entries);
        assert!(matches!(
            forged.verify(),
            Err(GovernanceError::AuditChainBroken { index: 0 })
        ));
    }

    #[test]
    fn audit_chain_detects_a_tampered_timestamp() {
        let store = ConstitutionStore::new(paper_defaults(), instant(1, 0)).expect("valid");
        let mut entries = store.audit_log().entries().to_vec();
        entries[0].timestamp = instant(1, 1);

        assert!(AuditLog::from_entries(entries).verify().is_err());
    }

    #[test]
    fn audit_chain_detects_a_tampered_link_and_a_removed_entry() {
        let mut log = AuditLog::default();
        log.append(
            AuditEvent::Expired {
                amendment: AmendmentId::from_raw(0),
            },
            instant(1, 0),
        );
        log.append(
            AuditEvent::Expired {
                amendment: AmendmentId::from_raw(1),
            },
            instant(1, 1),
        );

        let mut relinked = log.entries().to_vec();
        relinked[1].previous_hash = Some([3u8; 32]);
        assert!(AuditLog::from_entries(relinked).verify().is_err());

        let mut truncated = log.entries().to_vec();
        truncated.remove(0);
        assert!(AuditLog::from_entries(truncated).verify().is_err());

        assert!(log.verify().is_ok(), "the original chain is untouched");
    }

    #[test]
    fn audit_entries_cannot_be_reordered() {
        let mut log = AuditLog::default();
        log.append(
            AuditEvent::Expired {
                amendment: AmendmentId::from_raw(0),
            },
            instant(1, 0),
        );
        log.append(
            AuditEvent::Expired {
                amendment: AmendmentId::from_raw(1),
            },
            instant(1, 1),
        );

        let mut swapped = log.entries().to_vec();
        swapped.swap(0, 1);
        assert!(AuditLog::from_entries(swapped).verify().is_err());
    }

    // -- strict channel separation -----------------------------------------

    /// Mechanism modules. `paper_defaults.rs` is deliberately absent: it is the
    /// single, documented home of the §4.4 values, and it is configuration
    /// rather than mechanism.
    const MECHANISM_SOURCES: &[(&str, &str)] = &[
        ("lib.rs", include_str!("lib.rs")),
        ("constitution.rs", include_str!("constitution.rs")),
        ("amendment.rs", include_str!("amendment.rs")),
        ("audit_policy.rs", include_str!("audit_policy.rs")),
        ("capability.rs", include_str!("capability.rs")),
        ("gdpr.rs", include_str!("gdpr.rs")),
    ];

    /// The parameter values of the paper. Fixing any of them inside a mechanism
    /// is exactly what "none of these values is hard-coded" forbids.
    const PAPER_VALUES: [u128; 4] = [4, 5, 7, 14];

    #[test]
    fn mechanism_contains_no_hardcoded_paper_values() {
        for (name, source) in MECHANISM_SOURCES {
            let mechanism = source
                .split("#[cfg(test)]")
                .next()
                .expect("every source has a non-test prefix");
            let code = strip_comments_and_strings(mechanism);
            for literal in numeric_literals(&code) {
                assert!(
                    !PAPER_VALUES.contains(&literal),
                    "{name} hard-codes the paper value {literal} in mechanism code; \
                     it belongs in paper_defaults.rs as configuration"
                );
            }
        }
    }

    /// Two guards in one: the source scanner must not silently misread a
    /// literal, and it must not read comments or string literals as code. An
    /// earlier version of this scanner turned `7u32` into `732` and let a
    /// hard-coded `7` through, so the guard itself is guarded.
    #[test]
    fn the_source_scanner_itself_has_teeth() {
        let literals = numeric_literals(&strip_comments_and_strings(
            r#"
            let suffix = 7u32;
            let underscored = 14_i64;
            let plain = 4;
            let hexadecimal = 0x0E;
            let five = 5;
            let identifier_with_a_digit_3 = 1;
            "#,
        ));
        assert!(literals.contains(&7), "{literals:?}");
        assert!(literals.contains(&14), "{literals:?}");
        assert!(literals.contains(&4), "{literals:?}");
        assert!(literals.contains(&5), "{literals:?}");
        assert!(
            !literals.contains(&732),
            "the u32 suffix is not part of the value, got {literals:?}"
        );

        let from_noise = numeric_literals(&strip_comments_and_strings(
            "// 7 and 14 are mentioned here\nlet text = \"5 and 7 and 14\";\nlet kept = 4;",
        ));
        assert!(
            !from_noise.contains(&7) && !from_noise.contains(&14) && !from_noise.contains(&5),
            "comments and string literals are not mechanism code, got {from_noise:?}"
        );
        assert!(from_noise.contains(&4), "{from_noise:?}");
    }

    #[test]
    fn paper_defaults_is_the_only_home_of_the_paper_values() {
        // The values must exist somewhere, and that somewhere is the
        // configuration module, not a mechanism.
        let configuration = strip_comments_and_strings(
            include_str!("paper_defaults.rs")
                .split("#[cfg(test)]")
                .next()
                .expect("paper_defaults.rs has a non-test prefix"),
        );
        let literals = numeric_literals(&configuration);
        for value in PAPER_VALUES {
            assert!(
                literals.contains(&value),
                "the paper value {value} is missing from paper_defaults.rs; \
                 was it hard-coded somewhere else?"
            );
        }
    }

    /// Removes comments and string literals, keeping only executable tokens.
    fn strip_comments_and_strings(source: &str) -> String {
        let chars: Vec<char> = source.chars().collect();
        let mut out = String::with_capacity(source.len());
        let mut index = 0;
        while index < chars.len() {
            let current = chars[index];
            let next = chars.get(index + 1).copied();

            if current == '/' && next == Some('/') {
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
                continue;
            }
            if current == '/' && next == Some('*') {
                index += 2;
                while index + 1 < chars.len() && !(chars[index] == '*' && chars[index + 1] == '/') {
                    index += 1;
                }
                index = (index + 2).min(chars.len());
                continue;
            }
            if current == '"' {
                index += 1;
                while index < chars.len() && chars[index] != '"' {
                    if chars[index] == '\\' {
                        index += 1;
                    }
                    index += 1;
                }
                index = (index + 1).min(chars.len());
                out.push(' ');
                continue;
            }
            out.push(current);
            index += 1;
        }
        out
    }

    /// Extracts every numeric literal from tokenised source.
    ///
    /// A literal's type suffix (`u32`, `i64`, …) is skipped rather than folded
    /// into the value: `7u32` is the number seven, not seven hundred and
    /// thirty-two. Radix prefixes are honoured for the same reason — a guard
    /// that misreads the code it guards is worse than no guard.
    fn numeric_literals(source: &str) -> BTreeSet<u128> {
        let chars: Vec<char> = source.chars().collect();
        let mut literals = BTreeSet::new();
        let mut index = 0;
        while index < chars.len() {
            let current = chars[index];

            if current.is_ascii_alphabetic() || current == '_' {
                while index < chars.len()
                    && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
                {
                    index += 1;
                }
                continue;
            }

            if current.is_ascii_digit() {
                let (radix, start) = if current == '0' && index + 1 < chars.len() {
                    match chars[index + 1] {
                        'x' | 'X' => (16, index + 2),
                        'b' | 'B' => (2, index + 2),
                        'o' | 'O' => (8, index + 2),
                        _ => (10_u32, index),
                    }
                } else {
                    (10_u32, index)
                };

                index = start;
                let mut digits = String::new();
                while index < chars.len() {
                    let character = chars[index];
                    if character == '_' {
                        index += 1;
                        continue;
                    }
                    if character.is_digit(radix) {
                        digits.push(character);
                        index += 1;
                        continue;
                    }
                    break;
                }
                while index < chars.len()
                    && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
                {
                    index += 1;
                }
                if let Ok(value) = u128::from_str_radix(&digits, radix) {
                    literals.insert(value);
                }
                continue;
            }

            index += 1;
        }
        literals
    }
}
