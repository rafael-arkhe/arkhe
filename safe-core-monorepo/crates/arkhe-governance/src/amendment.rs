//! The amendment flow: **proposal → quorum → timelock**.
//!
//! Section 9.1 of the Arkhe paper requires *a machine-readable constitution in
//! decidable logic, amendable only through proposal, quorum, and timelock*.
//! Those three steps are implemented here, in that order, as the only path by
//! which a constitutional value can change.
//!
//! The flow is fail-closed at every point:
//!
//! * a proposal that does not reach quorum inside its window is **rejected**,
//!   never applied tacitly (deny-on-timeout);
//! * a proposal whose quorum has become arithmetically unreachable is rejected
//!   rather than left pending;
//! * a proposal is judged by the constitution **it amends**, so it can never
//!   lower its own bar, and once another amendment is applied it is rejected as
//!   stale instead of being applied under rules it was never voted on;
//! * a proposal can never be applied before its timelock elapses, which is what
//!   turns governance capture from an instantaneous act into one that must
//!   survive days of public visibility (§9.1).
//!
//! Every step appends to the store's append-only audit chain.
//!
//! # Values versus mechanisms
//!
//! This module contains no quorum and no duration. It reads the pair that
//! governs an amendment out of the constitution in force ([`Constitution::parameters_for`])
//! and freezes it into the proposal, so that the values a vote was counted
//! against are the values the audit trail records.

use crate::constitution::{
    AmendmentClass, AuditEvent, CanonicalEncoder, Constitution, ConstitutionStore, Council,
    Fraction, GovernanceError, GovernorId, Timelock, DECISION_DOMAIN,
};
use arkhe_core::hash::{blake3_hash, hash_to_hex};
use arkhe_core::ArkheHash;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// Identifier of a proposed amendment.
///
/// Identifiers are assigned by the store in proposal order and never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AmendmentId(u64);

impl AmendmentId {
    /// Wraps a raw identifier.
    pub(crate) const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the raw identifier.
    pub const fn raw(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for AmendmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Why a proposal was rejected without being applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectionReason {
    /// Enough governors voted against that the quorum can no longer be reached
    /// while the window is open.
    QuorumImpossible,
    /// The constitution the proposal was made against has been superseded.
    BaseConstitutionSuperseded,
}

impl RejectionReason {
    /// Stable machine-readable label.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::QuorumImpossible => "quorum-impossible",
            Self::BaseConstitutionSuperseded => "base-constitution-superseded",
        }
    }
}

impl fmt::Display for RejectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Where an amendment is in the proposal → quorum → timelock sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmendmentState {
    /// Open for votes; the quorum has not been reached yet.
    Voting,
    /// Quorum reached; the change must stay publicly visible until `until`.
    Timelocked {
        /// Instant at which the amendment becomes applicable.
        until: DateTime<Utc>,
    },
    /// Applied: the proposed values are in force. Terminal.
    Applied,
    /// Rejected without being applied. Terminal.
    Rejected(RejectionReason),
    /// The voting window closed without quorum (deny-on-timeout). Terminal.
    Expired,
}

impl AmendmentState {
    /// Stable machine-readable label.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Voting => "voting",
            Self::Timelocked { .. } => "timelocked",
            Self::Applied => "applied",
            Self::Rejected(_) => "rejected",
            Self::Expired => "expired",
        }
    }

    /// Returns `true` while the amendment can still make progress.
    pub const fn is_open(&self) -> bool {
        matches!(self, Self::Voting | Self::Timelocked { .. })
    }
}

impl fmt::Display for AmendmentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timelocked { until } => write!(f, "timelocked until {until}"),
            Self::Rejected(reason) => write!(f, "rejected ({reason})"),
            other => f.write_str(other.label()),
        }
    }
}

/// A proposed change to the constitution.
///
/// An amendment is created by [`ConstitutionStore::propose`], which also freezes
/// into it the values it will be judged by:
///
/// * `base` — the canonical hash of the constitution it amends, so the vote is
///   provably a vote about a specific value set;
/// * `quorum`, `timelock`, `council` — read out of that base constitution, so
///   the proposal cannot lower its own bar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Amendment {
    id: AmendmentId,
    class: AmendmentClass,
    base: ArkheHash,
    base_version: u32,
    proposed: Constitution,
    state: AmendmentState,
    proposed_at: DateTime<Utc>,
    voting_deadline: DateTime<Utc>,
    quorum: Fraction,
    timelock: Timelock,
    council: Council,
    quorum_reached_at: Option<DateTime<Utc>>,
    approvals: BTreeSet<GovernorId>,
    rejections: BTreeSet<GovernorId>,
}

impl Amendment {
    /// Returns the identifier.
    pub fn id(&self) -> AmendmentId {
        self.id
    }

    /// Returns the class that selected the governing parameters.
    pub fn class(&self) -> AmendmentClass {
        self.class
    }

    /// Returns the state in the proposal → quorum → timelock sequence.
    pub fn state(&self) -> AmendmentState {
        self.state
    }

    /// Returns `true` while the amendment can still make progress.
    pub fn is_open(&self) -> bool {
        self.state.is_open()
    }

    /// Returns the canonical hash of the constitution this proposal amends.
    pub fn base_hash(&self) -> ArkheHash {
        self.base
    }

    /// Returns the version number the amendment targets.
    pub fn base_version(&self) -> u32 {
        self.base_version
    }

    /// Returns the values the amendment would put into force.
    pub fn proposed(&self) -> &Constitution {
        &self.proposed
    }

    /// Returns the quorum this amendment is judged by.
    ///
    /// Frozen from the base constitution when the proposal was made.
    pub fn quorum(&self) -> Fraction {
        self.quorum
    }

    /// Returns the timelock this amendment is judged by.
    ///
    /// Frozen from the base constitution when the proposal was made.
    pub fn timelock(&self) -> Timelock {
        self.timelock
    }

    /// Returns the council eligible to vote on this amendment.
    ///
    /// Frozen from the base constitution when the proposal was made.
    pub fn council(&self) -> &Council {
        &self.council
    }

    /// Returns the number of seats eligible to vote.
    pub fn eligible(&self) -> u32 {
        self.council.size()
    }

    /// Returns the instant the proposal was made.
    pub fn proposed_at(&self) -> DateTime<Utc> {
        self.proposed_at
    }

    /// Returns the instant the voting window closes, inclusive.
    pub fn voting_deadline(&self) -> DateTime<Utc> {
        self.voting_deadline
    }

    /// Returns the instant the quorum was reached, if it ever was.
    pub fn quorum_reached_at(&self) -> Option<DateTime<Utc>> {
        self.quorum_reached_at
    }

    /// Returns the instant the amendment may be applied, if the quorum was
    /// reached.
    pub fn timelock_until(&self) -> Option<DateTime<Utc>> {
        match self.state {
            AmendmentState::Timelocked { until } => Some(until),
            _ => None,
        }
    }

    /// Returns the distinct governors that approved.
    pub fn approvals(&self) -> u32 {
        self.approvals.len() as u32
    }

    /// Returns the distinct governors that voted against.
    pub fn rejections(&self) -> u32 {
        self.rejections.len() as u32
    }

    /// Returns `true` when `governor` approved.
    pub fn approved_by(&self, governor: &GovernorId) -> bool {
        self.approvals.contains(governor)
    }

    /// Returns `true` when `governor` voted against.
    pub fn rejected_by(&self, governor: &GovernorId) -> bool {
        self.rejections.contains(governor)
    }
}

/// The result of a vote, as counted by the mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoteOutcome {
    /// The amendment voted on.
    pub amendment: AmendmentId,
    /// Distinct approvals after this vote.
    pub approvals: u32,
    /// Distinct rejections after this vote.
    pub rejections: u32,
    /// Seats eligible to vote.
    pub eligible: u32,
    /// The quorum the vote was counted against.
    pub quorum: Fraction,
    /// Whether the quorum is satisfied.
    pub quorum_met: bool,
    /// The state after this vote.
    pub state: AmendmentState,
}

/// A decision that was applied to the constitution.
///
/// [`AppliedAmendment::decision_hash`] is the record a deployment anchors
/// on-chain (ADR-011): it binds the amendment, the votes that carried it, the
/// quorum and timelock it was judged by, the constitution it replaced and the
/// value set it installed. [`AppliedAmendment::new_constitution_hash`] is the
/// canonical hash of that new value set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedAmendment {
    id: AmendmentId,
    class: AmendmentClass,
    base: ArkheHash,
    base_version: u32,
    new_constitution: Constitution,
    decision: ArkheHash,
    approvals: u32,
    eligible: u32,
    quorum: Fraction,
    timelock: Timelock,
    timelock_until: DateTime<Utc>,
    applied_at: DateTime<Utc>,
}

impl AppliedAmendment {
    /// Returns the identifier of the amendment that was applied.
    pub fn id(&self) -> AmendmentId {
        self.id
    }

    /// Returns the class of the amendment that was applied.
    pub fn class(&self) -> AmendmentClass {
        self.class
    }

    /// Returns the canonical hash of the constitution that was superseded.
    pub fn base_constitution_hash(&self) -> ArkheHash {
        self.base
    }

    /// Returns the version number of the constitution that was superseded.
    pub fn base_version(&self) -> u32 {
        self.base_version
    }

    /// Returns the values that are now in force.
    pub fn new_constitution(&self) -> &Constitution {
        &self.new_constitution
    }

    /// Returns the canonical hash of the values now in force.
    pub fn new_constitution_hash(&self) -> ArkheHash {
        self.new_constitution.canonical_hash()
    }

    /// Returns the hash of the decision record, to be anchored on-chain.
    pub fn decision_hash(&self) -> ArkheHash {
        self.decision
    }

    /// Returns the number of approvals that carried the amendment.
    pub fn approvals(&self) -> u32 {
        self.approvals
    }

    /// Returns the number of seats eligible to vote.
    pub fn eligible(&self) -> u32 {
        self.eligible
    }

    /// Returns the quorum the amendment was judged by.
    pub fn quorum(&self) -> Fraction {
        self.quorum
    }

    /// Returns the timelock the amendment was judged by.
    pub fn timelock(&self) -> Timelock {
        self.timelock
    }

    /// Returns the instant from which the amendment became applicable.
    pub fn timelock_until(&self) -> DateTime<Utc> {
        self.timelock_until
    }

    /// Returns the instant the amendment was applied.
    pub fn applied_at(&self) -> DateTime<Utc> {
        self.applied_at
    }
}

// ---------------------------------------------------------------------------
// The flow
// ---------------------------------------------------------------------------

impl ConstitutionStore {
    /// Registers a proposal to replace the constitution in force.
    ///
    /// The proposal is a proposal about the constitution *currently in force*:
    /// its canonical hash is recorded as the base, and the quorum, timelock,
    /// council and voting window that will govern the proposal are read out of
    /// that constitution and frozen into the amendment. The version number of
    /// the proposed values is assigned by the mechanism (`base + 1`), so a
    /// proposal cannot claim a version.
    ///
    /// Returns the identifier of the new amendment.
    pub fn propose(
        &mut self,
        class: AmendmentClass,
        proposed: Constitution,
        at: DateTime<Utc>,
    ) -> Result<AmendmentId, GovernanceError> {
        let base = self.constitution.canonical_hash();
        let base_version = self.constitution.version;
        let (quorum, timelock) = self.constitution.parameters_for(class);
        let council = self.constitution.council.clone();
        let voting_deadline = self.constitution.proposal_window.deadline_from(at);

        let mut proposed = proposed;
        proposed.version = base_version + 1;
        proposed.validate()?;

        let id = AmendmentId::from_raw(self.next_amendment_id);
        self.next_amendment_id += 1;

        let amendment = Amendment {
            id,
            class,
            base,
            base_version,
            proposed,
            state: AmendmentState::Voting,
            proposed_at: at,
            voting_deadline,
            quorum,
            timelock,
            council,
            quorum_reached_at: None,
            approvals: BTreeSet::new(),
            rejections: BTreeSet::new(),
        };

        self.audit.append(
            AuditEvent::Proposed {
                amendment: id,
                class,
                base,
                proposed: amendment.proposed.canonical_hash(),
            },
            at,
        );
        self.amendments.insert(id, amendment);
        Ok(id)
    }

    /// Records a vote on an amendment.
    ///
    /// The quorum is evaluated as an exact fraction: a `4/7` quorum is satisfied
    /// by exactly four approvals out of seven eligible seats and by nothing
    /// less. When an approval reaches the quorum the proposal enters the
    /// timelock, anchored at the instant the quorum was reached — the change
    /// must then stay publicly visible for the full duration before it can be
    /// applied.
    ///
    /// A vote cast after the window closed without quorum expires the proposal
    /// and is refused; a vote that makes the quorum arithmetically unreachable
    /// rejects it.
    pub fn vote(
        &mut self,
        id: AmendmentId,
        voter: GovernorId,
        approve: bool,
        at: DateTime<Utc>,
    ) -> Result<VoteOutcome, GovernanceError> {
        let current_base = self.constitution.canonical_hash();
        let (outcome, events) = match self.amendments.get_mut(&id) {
            Some(amendment) => drive_vote(amendment, &voter, approve, at, &current_base),
            None => {
                return Err(GovernanceError::UnknownAmendment { amendment: id });
            }
        };
        for event in events {
            self.audit.append(event, at);
        }
        outcome
    }

    /// Applies an amendment whose quorum has been reached and whose timelock
    /// has elapsed.
    ///
    /// This is the only path by which constitutional values change. Everything
    /// else is an error:
    ///
    /// * no quorum → [`GovernanceError::QuorumNotMet`];
    /// * quorum but the timelock is still running →
    ///   [`GovernanceError::TimelockNotElapsed`];
    /// * window closed without quorum → [`GovernanceError::ProposalExpired`],
    ///   and the proposal is marked expired so that it can never be applied
    ///   later;
    /// * already applied → [`GovernanceError::AlreadyApplied`];
    /// * already rejected → [`GovernanceError::AlreadyRejected`];
    /// * the base constitution was superseded →
    ///   [`GovernanceError::StaleBaseConstitution`].
    ///
    /// On success the new values are recorded as a hash-addressed version, and
    /// the returned [`AppliedAmendment`] carries both the decision hash (to be
    /// anchored on-chain) and the canonical hash of the new constitution.
    pub fn apply(
        &mut self,
        id: AmendmentId,
        at: DateTime<Utc>,
    ) -> Result<AppliedAmendment, GovernanceError> {
        let current_base = self.constitution.canonical_hash();
        let (outcome, mut events) = match self.amendments.get_mut(&id) {
            Some(amendment) => drive_apply(amendment, at, &current_base),
            None => {
                return Err(GovernanceError::UnknownAmendment { amendment: id });
            }
        };

        match outcome {
            Ok(applied) => {
                self.constitution = applied.new_constitution.clone();
                self.record_version();
                events.push(AuditEvent::Applied {
                    amendment: id,
                    decision: applied.decision,
                    previous_constitution: applied.base,
                    new_constitution: self.canonical_hash(),
                });
                for event in events {
                    self.audit.append(event, at);
                }
                Ok(applied)
            }
            Err(error) => {
                for event in events {
                    self.audit.append(event, at);
                }
                Err(error)
            }
        }
    }

    /// Expires every open proposal whose voting window has closed without
    /// quorum, and returns their identifiers.
    ///
    /// This is the explicit form of deny-on-timeout. It is never *required* —
    /// [`ConstitutionStore::apply`] applies the same rule on its own — but it
    /// lets an operator close out abandoned proposals and record the expiry
    /// without waiting for someone to try to apply them.
    pub fn sweep_expired(&mut self, at: DateTime<Utc>) -> Vec<AmendmentId> {
        let mut expired = Vec::new();
        let mut events = Vec::new();
        for amendment in self.amendments.values_mut() {
            if amendment.state == AmendmentState::Voting && at > amendment.voting_deadline {
                amendment.state = AmendmentState::Expired;
                expired.push(amendment.id);
                events.push(AuditEvent::Expired {
                    amendment: amendment.id,
                });
            }
        }
        for event in events {
            self.audit.append(event, at);
        }
        expired
    }
}

/// Drives one vote through the state machine.
///
/// Split out of [`ConstitutionStore::vote`] so that the audit events it
/// produces can be appended after the mutable borrow of the amendment ends.
fn drive_vote(
    amendment: &mut Amendment,
    voter: &GovernorId,
    approve: bool,
    at: DateTime<Utc>,
    current_base: &ArkheHash,
) -> (Result<VoteOutcome, GovernanceError>, Vec<AuditEvent>) {
    let mut events = Vec::new();
    let id = amendment.id;

    match amendment.state {
        AmendmentState::Applied => {
            return (
                Err(GovernanceError::AlreadyApplied { amendment: id }),
                events,
            );
        }
        AmendmentState::Rejected(reason) => {
            return (
                Err(GovernanceError::AlreadyRejected {
                    amendment: id,
                    reason,
                }),
                events,
            );
        }
        AmendmentState::Expired => {
            return (
                Err(GovernanceError::ProposalExpired {
                    amendment: id,
                    deadline: amendment.voting_deadline,
                }),
                events,
            );
        }
        AmendmentState::Voting | AmendmentState::Timelocked { .. } => {}
    }

    if amendment.base != *current_base {
        let reason = RejectionReason::BaseConstitutionSuperseded;
        amendment.state = AmendmentState::Rejected(reason);
        events.push(AuditEvent::Rejected {
            amendment: id,
            reason,
        });
        return (
            Err(GovernanceError::StaleBaseConstitution {
                amendment: id,
                expected: hash_to_hex(&amendment.base),
                actual: hash_to_hex(current_base),
            }),
            events,
        );
    }

    if amendment.state == AmendmentState::Voting && at > amendment.voting_deadline {
        // Deny-on-timeout: no quorum inside the window is a rejection, not a
        // silent approval and not an indefinite pending state.
        amendment.state = AmendmentState::Expired;
        events.push(AuditEvent::Expired { amendment: id });
        return (
            Err(GovernanceError::ProposalExpired {
                amendment: id,
                deadline: amendment.voting_deadline,
            }),
            events,
        );
    }

    if !amendment.council.contains(voter) {
        return (
            Err(GovernanceError::NotACouncilMember {
                amendment: id,
                governor: voter.clone(),
            }),
            events,
        );
    }

    if amendment.approvals.contains(voter) || amendment.rejections.contains(voter) {
        return (
            Err(GovernanceError::AlreadyVoted {
                amendment: id,
                governor: voter.clone(),
            }),
            events,
        );
    }

    if approve {
        amendment.approvals.insert(voter.clone());
    } else {
        amendment.rejections.insert(voter.clone());
    }

    let approvals = amendment.approvals.len() as u32;
    let rejections = amendment.rejections.len() as u32;
    let eligible = amendment.council.size();
    let quorum_met = amendment.quorum.satisfied_by(approvals, eligible);

    if quorum_met && amendment.state == AmendmentState::Voting {
        amendment.state = AmendmentState::Timelocked {
            until: amendment.timelock.deadline_from(at),
        };
        amendment.quorum_reached_at = Some(at);
    } else if amendment.state == AmendmentState::Voting {
        // Every vote that is not an approval reduces the approvals that are
        // still arithmetically reachable. Once the quorum is out of reach the
        // proposal is rejected rather than left to time out.
        let still_reachable = eligible.saturating_sub(rejections);
        if still_reachable < amendment.quorum.threshold_for(eligible) {
            let reason = RejectionReason::QuorumImpossible;
            amendment.state = AmendmentState::Rejected(reason);
            events.push(AuditEvent::Rejected {
                amendment: id,
                reason,
            });
        }
    }

    events.push(AuditEvent::Voted {
        amendment: id,
        voter: voter.clone(),
        approve,
        approvals,
        rejections,
        eligible,
        quorum_met,
    });

    (
        Ok(VoteOutcome {
            amendment: id,
            approvals,
            rejections,
            eligible,
            quorum: amendment.quorum,
            quorum_met,
            state: amendment.state,
        }),
        events,
    )
}

/// Drives one application attempt through the state machine.
///
/// Split out of [`ConstitutionStore::apply`] so that the audit events it
/// produces can be appended after the mutable borrow of the amendment ends.
fn drive_apply(
    amendment: &mut Amendment,
    at: DateTime<Utc>,
    current_base: &ArkheHash,
) -> (Result<AppliedAmendment, GovernanceError>, Vec<AuditEvent>) {
    let mut events = Vec::new();
    let id = amendment.id;

    match amendment.state {
        AmendmentState::Applied => {
            return (
                Err(GovernanceError::AlreadyApplied { amendment: id }),
                events,
            );
        }
        AmendmentState::Rejected(reason) => {
            return (
                Err(GovernanceError::AlreadyRejected {
                    amendment: id,
                    reason,
                }),
                events,
            );
        }
        AmendmentState::Expired => {
            return (
                Err(GovernanceError::ProposalExpired {
                    amendment: id,
                    deadline: amendment.voting_deadline,
                }),
                events,
            );
        }
        AmendmentState::Voting | AmendmentState::Timelocked { .. } => {}
    }

    if amendment.base != *current_base {
        let reason = RejectionReason::BaseConstitutionSuperseded;
        amendment.state = AmendmentState::Rejected(reason);
        events.push(AuditEvent::Rejected {
            amendment: id,
            reason,
        });
        return (
            Err(GovernanceError::StaleBaseConstitution {
                amendment: id,
                expected: hash_to_hex(&amendment.base),
                actual: hash_to_hex(current_base),
            }),
            events,
        );
    }

    let approvals = amendment.approvals.len() as u32;
    let eligible = amendment.council.size();

    let unlock_at = match amendment.state {
        AmendmentState::Timelocked { until } => until,
        _ => {
            if at > amendment.voting_deadline {
                // The window closed without quorum: deny-on-timeout.
                amendment.state = AmendmentState::Expired;
                events.push(AuditEvent::Expired { amendment: id });
                return (
                    Err(GovernanceError::ProposalExpired {
                        amendment: id,
                        deadline: amendment.voting_deadline,
                    }),
                    events,
                );
            }
            // The window is still open, so the proposal is simply short of
            // votes and may still get them.
            return (
                Err(GovernanceError::QuorumNotMet {
                    amendment: id,
                    approvals,
                    eligible,
                    required: amendment.quorum,
                }),
                events,
            );
        }
    };

    if !amendment.quorum.satisfied_by(approvals, eligible) {
        return (
            Err(GovernanceError::QuorumNotMet {
                amendment: id,
                approvals,
                eligible,
                required: amendment.quorum,
            }),
            events,
        );
    }

    if at < unlock_at {
        return (
            Err(GovernanceError::TimelockNotElapsed {
                amendment: id,
                unlock_at,
                now: at,
            }),
            events,
        );
    }

    let new_constitution = amendment.proposed.clone();
    let new_hash = new_constitution.canonical_hash();
    let decision = decision_hash(amendment, &new_hash, at);
    amendment.state = AmendmentState::Applied;

    (
        Ok(AppliedAmendment {
            id,
            class: amendment.class,
            base: amendment.base,
            base_version: amendment.base_version,
            new_constitution,
            decision,
            approvals,
            eligible,
            quorum: amendment.quorum,
            timelock: amendment.timelock,
            timelock_until: unlock_at,
            applied_at: at,
        }),
        events,
    )
}

/// Computes the hash of the decision record.
///
/// The hash binds the amendment, the class it was judged as, the constitution
/// it replaced, the values it installs, the quorum and timelock it was judged
/// by, every governor that approved, and the instant it was applied. Two
/// decisions with the same content produce the same hash; changing any of it
/// changes the hash.
fn decision_hash(amendment: &Amendment, new_hash: &ArkheHash, at: DateTime<Utc>) -> ArkheHash {
    let mut encoder = CanonicalEncoder::new(DECISION_DOMAIN);
    encoder.text("amendment");
    encoder.u64(amendment.id.raw());
    encoder.text("class");
    encoder.text(amendment.class.label());
    encoder.text("base-constitution");
    encoder.hash(&amendment.base);
    encoder.text("base-version");
    encoder.u32(amendment.base_version);
    encoder.text("new-constitution");
    encoder.hash(new_hash);
    encoder.text("quorum");
    encoder.u32(amendment.quorum.numerator);
    encoder.u32(amendment.quorum.denominator);
    encoder.text("eligible");
    encoder.u32(amendment.council.size());
    encoder.text("timelock-seconds");
    encoder.i64(amendment.timelock.seconds);
    encoder.text("approvals");
    for governor in &amendment.approvals {
        encoder.text(governor.as_str());
    }
    encoder.text("applied-at-micros");
    encoder.i64(at.timestamp_micros());
    blake3_hash(&encoder.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constitution::AuditLog;
    use crate::paper_defaults::paper_defaults;
    use chrono::{Duration, TimeZone};

    fn day(offset: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0)
            .single()
            .expect("the test start instant is valid")
            + Duration::days(offset)
    }

    fn store() -> ConstitutionStore {
        ConstitutionStore::new(paper_defaults(), day(0)).expect("the §4.4 defaults are valid")
    }

    fn seats() -> Vec<GovernorId> {
        paper_defaults().council.members().to_vec()
    }

    /// Values that differ from the defaults, so that applying them is
    /// observable.
    fn amended_values() -> Constitution {
        let mut next = paper_defaults();
        next.kill_switch_quorum = Fraction::new(2, 7);
        next
    }

    fn approve(
        store: &mut ConstitutionStore,
        id: AmendmentId,
        voters: &[GovernorId],
        at: DateTime<Utc>,
    ) {
        for voter in voters {
            store
                .vote(id, voter.clone(), true, at)
                .expect("the vote is accepted");
        }
    }

    // -- quorum, as an exact fraction --------------------------------------

    #[test]
    fn exact_four_sevenths_satisfies_the_common_quorum() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");

        for voter in council.iter().take(3) {
            let outcome = store
                .vote(id, voter.clone(), true, day(0))
                .expect("vote accepted");
            assert!(
                !outcome.quorum_met,
                "three of seven approvals is below four sevenths"
            );
        }
        assert_eq!(
            store.amendment(id).expect("known").state(),
            AmendmentState::Voting
        );

        let outcome = store
            .vote(id, council[3].clone(), true, day(0))
            .expect("vote accepted");
        assert_eq!(outcome.approvals, 4);
        assert_eq!(outcome.eligible, 7);
        assert_eq!(outcome.quorum, Fraction::new(4, 7));
        assert!(
            outcome.quorum_met,
            "exactly four of seven satisfies four sevenths"
        );
        assert!(matches!(
            store.amendment(id).expect("known").state(),
            AmendmentState::Timelocked { .. }
        ));
    }

    #[test]
    fn four_sevenths_minus_one_does_not_satisfy_the_common_quorum() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        approve(&mut store, id, &council[..3], day(0));

        // One vote short of the quorum leaves the proposal open and the
        // constitution untouched, however long it is given.
        let short = store.apply(id, day(1));
        assert!(
            matches!(
                short,
                Err(GovernanceError::QuorumNotMet {
                    approvals: 3,
                    eligible: 7,
                    required,
                    ..
                }) if required == Fraction::new(4, 7)
            ),
            "three of seven must not carry a change, got {short:?}"
        );
        assert_eq!(
            store.amendment(id).expect("known").state(),
            AmendmentState::Voting
        );
        assert_eq!(store.history().len(), 1);

        // One more approval is exactly the quorum, and then only the timelock
        // remains in the way.
        approve(&mut store, id, &council[3..4], day(0));
        assert!(matches!(
            store.apply(id, day(1)),
            Err(GovernanceError::TimelockNotElapsed { .. })
        ));
    }

    #[test]
    fn exact_five_sevenths_satisfies_the_critical_quorum() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Critical, amended_values(), day(0))
            .expect("proposal accepted");

        for voter in council.iter().take(4) {
            let outcome = store
                .vote(id, voter.clone(), true, day(0))
                .expect("vote accepted");
            assert!(
                !outcome.quorum_met,
                "four of seven approvals is below five sevenths for a critical change"
            );
        }

        let outcome = store
            .vote(id, council[4].clone(), true, day(0))
            .expect("vote accepted");
        assert_eq!(outcome.quorum, Fraction::new(5, 7));
        assert!(
            outcome.quorum_met,
            "exactly five of seven satisfies five sevenths"
        );
    }

    #[test]
    fn five_sevenths_minus_one_does_not_satisfy_the_critical_quorum() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Critical, amended_values(), day(0))
            .expect("proposal accepted");
        approve(&mut store, id, &council[..4], day(0));

        assert!(
            matches!(
                store.apply(id, day(1)),
                Err(GovernanceError::QuorumNotMet { .. })
            ),
            "four of seven must not carry a critical change"
        );
        assert_eq!(
            store.amendment(id).expect("known").state(),
            AmendmentState::Voting,
            "a proposal short of quorum inside its window stays open"
        );
    }

    #[test]
    fn the_quorum_boundary_is_inclusive_and_exactly_computed() {
        let four_sevenths = Fraction::new(4, 7);
        assert_eq!(four_sevenths.threshold_for(7), 4);
        assert!(four_sevenths.satisfied_by(4, 7));
        assert!(!four_sevenths.satisfied_by(3, 7));
        assert_eq!(four_sevenths.to_string(), "4/7");

        let five_sevenths = Fraction::new(5, 7);
        assert_eq!(five_sevenths.threshold_for(7), 5);
        assert!(five_sevenths.satisfied_by(5, 7));
        assert!(!five_sevenths.satisfied_by(4, 7));

        // `satisfied_by` and `threshold_for` must agree for every council size,
        // which is what an exact-fraction quorum means. A `f64` would not.
        for eligible in 1..=40_u32 {
            for numerator in 1..=7_u32 {
                let quorum = Fraction::new(numerator, 7);
                let threshold = quorum.threshold_for(eligible);
                assert!(threshold <= eligible);
                assert!(quorum.satisfied_by(threshold, eligible));
                if threshold > 0 {
                    assert!(!quorum.satisfied_by(threshold - 1, eligible));
                }
            }
        }

        // Invalid fractions are never satisfied (fail-closed).
        assert!(!Fraction::new(4, 0).satisfied_by(4, 7));
        assert!(!Fraction::new(0, 7).satisfied_by(7, 7));
        assert_eq!(Fraction::new(4, 0).threshold_for(7), u32::MAX);
    }

    #[test]
    fn a_quorum_of_four_sevenths_is_not_a_quorum_of_half() {
        // These two are one vote apart on a seven-seat council; exact-integer
        // arithmetic keeps them apart.
        let four_sevenths = Fraction::new(4, 7);
        let one_half = Fraction::new(1, 2);
        assert_eq!(four_sevenths.threshold_for(7), 4);
        assert_eq!(one_half.threshold_for(7), 4);
        assert!(four_sevenths.satisfied_by(4, 7) && one_half.satisfied_by(4, 7));
        // ... but not on a council where the fractions differ.
        assert_eq!(four_sevenths.threshold_for(6), 4);
        assert_eq!(one_half.threshold_for(6), 3);
        assert!(!four_sevenths.satisfied_by(3, 6));
    }

    // -- timelock ----------------------------------------------------------

    #[test]
    fn the_common_timelock_blocks_application_before_seven_days() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        approve(&mut store, id, &council[..4], day(0));

        assert_eq!(
            store.amendment(id).expect("known").timelock(),
            Timelock::from_days(7)
        );
        assert_eq!(store.amendment(id).expect("known").timelock_until(), Some(day(7)));

        let early = store.apply(id, day(6));
        assert!(
            matches!(early, Err(GovernanceError::TimelockNotElapsed { .. })),
            "applying inside the timelock must fail, got {early:?}"
        );

        let almost = store.apply(id, day(7) - Duration::seconds(1));
        assert!(
            matches!(almost, Err(GovernanceError::TimelockNotElapsed { .. })),
            "one second early must still fail, got {almost:?}"
        );

        let applied = store.apply(id, day(7)).expect("applies at the boundary");
        assert_eq!(applied.approvals(), 4);
        assert_eq!(applied.timelock_until(), day(7));
        assert_eq!(applied.applied_at(), day(7));
    }

    #[test]
    fn the_critical_timelock_requires_the_longer_delay() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Critical, amended_values(), day(0))
            .expect("proposal accepted");

        approve(&mut store, id, &council[..4], day(0));
        assert!(
            matches!(
                store.apply(id, day(1)),
                Err(GovernanceError::QuorumNotMet { .. })
            ),
            "a critical change needs five sevenths, not four"
        );

        approve(&mut store, id, &council[4..5], day(0));
        assert_eq!(
            store.amendment(id).expect("known").timelock(),
            Timelock::from_days(14)
        );

        let early = store.apply(id, day(13));
        assert!(
            matches!(early, Err(GovernanceError::TimelockNotElapsed { .. })),
            "a critical change must survive fourteen days, got {early:?}"
        );
        assert!(store.apply(id, day(14)).is_ok());
    }

    #[test]
    fn the_timelock_is_anchored_at_the_quorum_not_at_the_proposal() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");

        // Quorum only on the third day: the change must stay visible for the
        // full timelock *from that moment*.
        approve(&mut store, id, &council[..4], day(3));

        assert_eq!(
            store.amendment(id).expect("known").quorum_reached_at(),
            Some(day(3))
        );
        assert!(matches!(
            store.apply(id, day(9)),
            Err(GovernanceError::TimelockNotElapsed { .. })
        ));
        assert!(store.apply(id, day(10)).is_ok());
    }

    // -- deny-on-timeout ---------------------------------------------------

    #[test]
    fn deny_on_timeout_rejects_a_proposal_and_never_applies_it() {
        let mut store = store();
        let council = seats();
        let before = store.canonical_hash();
        let versions = store.history().len();

        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        approve(&mut store, id, &council[..3], day(0));

        // Inside the window the proposal is merely short of votes.
        assert!(matches!(
            store.apply(id, day(1)),
            Err(GovernanceError::QuorumNotMet { .. })
        ));
        assert_eq!(
            store.amendment(id).expect("known").state(),
            AmendmentState::Voting
        );

        // One day past the window the same call is a rejection.
        let expired = store.apply(id, day(8));
        assert!(
            matches!(expired, Err(GovernanceError::ProposalExpired { .. })),
            "an expired proposal must not apply, got {expired:?}"
        );
        assert_eq!(
            store.amendment(id).expect("known").state(),
            AmendmentState::Expired
        );

        // It stays rejected forever: it can neither be applied nor revived by
        // a late vote.
        assert!(matches!(
            store.apply(id, day(400)),
            Err(GovernanceError::ProposalExpired { .. })
        ));
        assert!(matches!(
            store.vote(id, council[3].clone(), true, day(400)),
            Err(GovernanceError::ProposalExpired { .. })
        ));

        assert_eq!(
            store.canonical_hash(),
            before,
            "the constitution must be untouched"
        );
        assert_eq!(
            store.history().len(),
            versions,
            "no version may have been recorded"
        );
        assert!(store.verify().is_ok());
    }

    #[test]
    fn deny_on_timeout_expires_a_proposal_that_nobody_tried_to_apply() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        approve(&mut store, id, &council[..3], day(0));

        assert!(store.sweep_expired(day(1)).is_empty(), "the window is open");
        assert_eq!(store.sweep_expired(day(8)), vec![id]);
        assert_eq!(
            store.amendment(id).expect("known").state(),
            AmendmentState::Expired
        );
        assert!(matches!(
            store.apply(id, day(9)),
            Err(GovernanceError::ProposalExpired { .. })
        ));

        // The expiry is on the record.
        let last = store
            .audit_log()
            .entries()
            .last()
            .expect("the trail is not empty");
        assert_eq!(last.event, AuditEvent::Expired { amendment: id });
        assert!(store.audit_log().verify().is_ok());
    }

    #[test]
    fn a_proposal_that_reached_quorum_in_time_still_applies_after_the_window() {
        // The window governs *voting*; once the quorum is in, only the timelock
        // stands between the change and application.
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        approve(&mut store, id, &council[..4], day(0));

        assert!(store.sweep_expired(day(400)).is_empty());
        assert!(store.apply(id, day(400)).is_ok());
    }

    // -- finality ----------------------------------------------------------

    #[test]
    fn a_proposal_cannot_be_applied_twice() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        approve(&mut store, id, &council[..4], day(0));
        store.apply(id, day(7)).expect("applied");

        assert!(matches!(
            store.apply(id, day(8)),
            Err(GovernanceError::AlreadyApplied { .. })
        ));
        assert!(matches!(
            store.vote(id, council[0].clone(), true, day(8)),
            Err(GovernanceError::AlreadyApplied { .. })
        ));
        assert_eq!(store.history().len(), 2);
    }

    #[test]
    fn a_proposal_whose_quorum_became_impossible_is_rejected() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");

        for voter in council.iter().take(4) {
            let outcome = store
                .vote(id, voter.clone(), false, day(0))
                .expect("vote accepted");
            assert!(!outcome.quorum_met);
        }

        // Four rejections leave only three possible approvals, below the
        // quorum of four: no point waiting for the window to close.
        assert_eq!(
            store.amendment(id).expect("known").state(),
            AmendmentState::Rejected(RejectionReason::QuorumImpossible)
        );
        assert!(matches!(
            store.vote(id, council[4].clone(), true, day(1)),
            Err(GovernanceError::AlreadyRejected { .. })
        ));
        assert!(matches!(
            store.apply(id, day(1)),
            Err(GovernanceError::AlreadyRejected { .. })
        ));
        assert!(store.verify().is_ok());
    }

    #[test]
    fn a_governor_votes_once_and_only_if_seated() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");

        store
            .vote(id, council[0].clone(), true, day(0))
            .expect("first vote accepted");
        assert!(matches!(
            store.vote(id, council[0].clone(), true, day(0)),
            Err(GovernanceError::AlreadyVoted { .. })
        ));
        assert!(matches!(
            store.vote(id, council[0].clone(), false, day(0)),
            Err(GovernanceError::AlreadyVoted { .. })
        ));
        assert!(matches!(
            store.vote(id, GovernorId::new("outsider"), true, day(0)),
            Err(GovernanceError::NotACouncilMember { .. })
        ));
        assert_eq!(store.amendment(id).expect("known").approvals(), 1);
    }

    #[test]
    fn an_unknown_amendment_is_a_typed_error() {
        let mut store = store();
        let unknown = AmendmentId::from_raw(999);
        assert!(matches!(
            store.vote(unknown, seats()[0].clone(), true, day(0)),
            Err(GovernanceError::UnknownAmendment { .. })
        ));
        assert!(matches!(
            store.apply(unknown, day(0)),
            Err(GovernanceError::UnknownAmendment { .. })
        ));
    }

    #[test]
    fn a_proposal_against_a_superseded_constitution_is_rejected() {
        let mut store = store();
        let council = seats();
        let first = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        let second = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");

        approve(&mut store, first, &council[..4], day(0));
        approve(&mut store, second, &council[..4], day(6));
        store.apply(first, day(7)).expect("the first applies");

        let stale = store.apply(second, day(30));
        assert!(
            matches!(stale, Err(GovernanceError::StaleBaseConstitution { .. })),
            "a proposal against a superseded constitution must be refused, got {stale:?}"
        );
        assert_eq!(
            store.amendment(second).expect("known").state(),
            AmendmentState::Rejected(RejectionReason::BaseConstitutionSuperseded)
        );
        assert_eq!(
            store.history().len(),
            2,
            "only the first amendment may have been applied"
        );
        assert!(store.verify().is_ok());
    }

    // -- values versus mechanisms -----------------------------------------

    #[test]
    fn a_proposal_is_judged_by_the_constitution_it_amends() {
        // A proposal that would relax both the quorum and the timelock must not
        // be able to use the relaxed values to get itself passed.
        let mut store = store();
        let council = seats();
        let mut relaxed = paper_defaults();
        relaxed.amendment_quorum = Fraction::new(1, 7);
        relaxed.amendment_timelock = Timelock::from_seconds(60);

        let id = store
            .propose(AmendmentClass::Common, relaxed, day(0))
            .expect("proposal accepted");

        let amendment = store.amendment(id).expect("known");
        assert_eq!(
            amendment.quorum(),
            Fraction::new(4, 7),
            "the quorum in force governs the proposal"
        );
        assert_eq!(
            amendment.timelock(),
            Timelock::from_days(7),
            "the timelock in force governs the proposal"
        );
        assert_eq!(amendment.eligible(), 7);

        approve(&mut store, id, &council[..1], day(0));
        assert_eq!(
            store.amendment(id).expect("known").state(),
            AmendmentState::Voting,
            "one approval is not the quorum that governs this proposal"
        );
        assert!(matches!(
            store.apply(id, day(1)),
            Err(GovernanceError::QuorumNotMet { .. })
        ));

        // With the governing quorum met, the governing timelock still applies.
        approve(&mut store, id, &council[1..4], day(0));
        assert!(matches!(
            store.apply(id, day(1)),
            Err(GovernanceError::TimelockNotElapsed { .. })
        ));
        let applied = store.apply(id, day(7)).expect("applies under the old rules");
        assert_eq!(applied.new_constitution().amendment_quorum, Fraction::new(1, 7));
    }

    #[test]
    fn a_completely_different_parameter_set_drives_the_flow() {
        // Three governors, a four-fifths quorum, a one-day timelock and a
        // two-day window: nothing here is a §4.4 value, and the mechanism
        // neither knows nor cares.
        let mut constitution = paper_defaults();
        constitution.amendment_quorum = Fraction::new(4, 5);
        constitution.amendment_timelock = Timelock::from_days(1);
        constitution.proposal_window = Timelock::from_days(2);
        constitution.council = Council::new([
            GovernorId::new("g1"),
            GovernorId::new("g2"),
            GovernorId::new("g3"),
        ]);
        let council = constitution.council.members().to_vec();

        let mut next = paper_defaults();
        next.council = constitution.council.clone();
        let mut store =
            ConstitutionStore::new(constitution, day(0)).expect("the custom values are valid");

        let id = store
            .propose(AmendmentClass::Common, next, day(0))
            .expect("proposal accepted");
        assert_eq!(store.amendment(id).expect("known").quorum(), Fraction::new(4, 5));
        assert_eq!(store.amendment(id).expect("known").eligible(), 3);

        approve(&mut store, id, &council[..2], day(0));
        assert_eq!(
            store.amendment(id).expect("known").state(),
            AmendmentState::Voting,
            "two of three is below four fifths"
        );
        approve(&mut store, id, &council[2..3], day(0));

        assert!(matches!(
            store.apply(id, day(0)),
            Err(GovernanceError::TimelockNotElapsed { .. })
        ));
        assert!(store.apply(id, day(1)).is_ok());
        assert_eq!(store.history().len(), 2);
    }

    #[test]
    fn the_kill_switch_quorum_is_a_governed_value() {
        // The crate implements no kill-switch mechanism; P-S1 is represented as
        // a governed value, carried in the constitution and in its hash.
        let reference = paper_defaults();
        let mut variant = paper_defaults();
        variant.kill_switch_quorum = Fraction::new(6, 7);

        assert_eq!(reference.kill_switch_quorum, Fraction::new(4, 7));
        assert_ne!(variant.canonical_hash(), reference.canonical_hash());
        assert!(variant.validate().is_ok());
    }

    #[test]
    fn the_version_is_assigned_by_the_mechanism() {
        let mut store = store();
        let mut proposed = amended_values();
        proposed.version = 99;

        let id = store
            .propose(AmendmentClass::Common, proposed, day(0))
            .expect("proposal accepted");
        assert_eq!(
            store.amendment(id).expect("known").proposed().version,
            2,
            "a proposal can only ever produce the next version"
        );
    }

    #[test]
    fn an_invalid_proposal_is_refused_at_proposal_time() {
        let mut store = store();
        let mut broken = amended_values();
        broken.amendment_quorum = Fraction::new(0, 7);

        assert!(matches!(
            store.propose(AmendmentClass::Common, broken, day(0)),
            Err(GovernanceError::InvalidParameter { .. })
        ));
    }

    // -- decisions and the audit trail ------------------------------------

    #[test]
    fn applying_produces_a_decision_hash_and_a_new_constitution_hash() {
        let mut store = store();
        let council = seats();
        let previous = store.canonical_hash();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        approve(&mut store, id, &council[..4], day(0));

        let applied = store.apply(id, day(7)).expect("applied");

        assert_eq!(applied.base_constitution_hash(), previous);
        assert_ne!(
            applied.base_constitution_hash(),
            applied.new_constitution_hash()
        );
        assert_eq!(applied.new_constitution_hash(), store.canonical_hash());
        assert_eq!(applied.id(), id);
        assert_eq!(applied.class(), AmendmentClass::Common);
        assert_eq!(applied.approvals(), 4);
        assert_eq!(applied.eligible(), 7);
        assert_eq!(applied.quorum(), Fraction::new(4, 7));
        assert_eq!(applied.timelock(), Timelock::from_days(7));
        assert_eq!(store.history().len(), 2);
        assert_eq!(store.history()[1].hash(), applied.new_constitution_hash());
        assert_eq!(store.history()[1].version(), 2);
        assert_eq!(store.history()[1].constitution(), applied.new_constitution());
        assert!(store.verify().is_ok());
    }

    #[test]
    fn the_decision_hash_is_deterministic_and_binds_the_votes() {
        let run = |voters: usize| {
            let mut store = store();
            let council = seats();
            let id = store
                .propose(AmendmentClass::Common, amended_values(), day(0))
                .expect("proposal accepted");
            approve(&mut store, id, &council[..voters], day(0));
            store.apply(id, day(7)).expect("applied").decision_hash()
        };

        assert_eq!(run(4), run(4), "the same decision hashes the same");
        assert_ne!(
            run(4),
            run(5),
            "a different set of approvals is a different decision"
        );
    }

    #[test]
    fn the_audit_trail_records_the_whole_lifecycle() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        approve(&mut store, id, &council[..4], day(0));
        let applied = store.apply(id, day(7)).expect("applied");

        let entries = store.audit_log().entries();
        assert_eq!(
            entries.len(),
            7,
            "sealed + proposed + four votes + applied"
        );
        assert!(matches!(entries[0].event, AuditEvent::Sealed { .. }));
        match &entries[1].event {
            AuditEvent::Proposed {
                amendment,
                class,
                base,
                proposed,
            } => {
                assert_eq!(*amendment, id);
                assert_eq!(*class, AmendmentClass::Common);
                assert_eq!(*base, applied.base_constitution_hash());
                assert_eq!(*proposed, applied.new_constitution_hash());
            }
            other => panic!("expected a proposal, got {other:?}"),
        }
        assert!(entries[2..6]
            .iter()
            .all(|entry| matches!(entry.event, AuditEvent::Voted { .. })));
        match &entries[6].event {
            AuditEvent::Applied {
                amendment,
                decision,
                previous_constitution,
                new_constitution,
            } => {
                assert_eq!(*amendment, id);
                assert_eq!(*decision, applied.decision_hash());
                assert_eq!(*previous_constitution, applied.base_constitution_hash());
                assert_eq!(*new_constitution, applied.new_constitution_hash());
            }
            other => panic!("expected an application, got {other:?}"),
        }
        assert!(entries[6].previous_hash.is_some());
        assert_eq!(entries[6].sequence, 6);
        assert!(store.audit_log().verify().is_ok());
        assert!(store.verify().is_ok());
    }

    #[test]
    fn replayed_votes_leave_no_record() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        let after_proposal = store.audit_log().len();

        store
            .vote(id, council[0].clone(), true, day(0))
            .expect("first vote accepted");
        let after_vote = store.audit_log().len();
        assert_eq!(after_vote, after_proposal + 1);

        // A refused vote changes nothing and records nothing: the trail shows
        // exactly what the mechanism did.
        assert!(store.vote(id, council[0].clone(), true, day(0)).is_err());
        assert_eq!(store.audit_log().len(), after_vote);
        assert!(store.audit_log().verify().is_ok());
    }

    #[test]
    fn the_audit_trail_is_append_only_across_a_whole_lifecycle() {
        let mut store = store();
        let council = seats();
        let rejected = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        let applied = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        approve(&mut store, applied, &council[..4], day(0));
        store.apply(applied, day(7)).expect("applied");
        store.sweep_expired(day(8));

        let log = store.audit_log();
        assert!(log.verify().is_ok());
        assert!(log.len() > 8);

        // Each entry commits to its predecessor, and the sequence is dense.
        let entries = log.entries();
        for (index, entry) in entries.iter().enumerate() {
            assert_eq!(entry.sequence, index as u64);
            assert_eq!(
                entry.previous_hash,
                if index == 0 {
                    None
                } else {
                    Some(entries[index - 1].current_hash)
                }
            );
        }

        // The rejected proposal really was rejected, and the applied one really
        // was applied: no other proposal moved the constitution.
        assert_eq!(
            store.amendment(rejected).expect("known").state(),
            AmendmentState::Expired
        );
        assert_eq!(
            store.amendment(applied).expect("known").state(),
            AmendmentState::Applied
        );
        assert_eq!(store.history().len(), 2);
    }

    #[test]
    fn the_trail_detects_a_forged_apply_record() {
        let mut store = store();
        let council = seats();
        let id = store
            .propose(AmendmentClass::Common, amended_values(), day(0))
            .expect("proposal accepted");
        approve(&mut store, id, &council[..4], day(0));
        store.apply(id, day(7)).expect("applied");

        let mut entries = store.audit_log().entries().to_vec();
        let last = entries.len() - 1;
        entries[last].event = AuditEvent::Expired { amendment: id };

        assert!(AuditLog::from_entries(entries).verify().is_err());
    }
}
