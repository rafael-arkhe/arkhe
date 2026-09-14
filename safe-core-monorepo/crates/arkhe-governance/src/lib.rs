// crates/arkhe-governance/src/lib.rs
//! Governance of the Arkhe OS.
//!
//! The crate contract is the one stated in the paper's Table 1: *no
//! constitutional change without quorum and timelock*.
//!
//! # Modules
//!
//! * [`constitution`] — the machine-readable constitution: the value types,
//!   their canonical hash-addressing, the version history, and the append-only
//!   governance audit chain.
//! * [`amendment`] — the amendment mechanism: **proposal → quorum → timelock**,
//!   fail-closed.
//! * [`paper_defaults`] — the §4.4 parameter values, as configuration.
//! * [`audit_policy`], [`capability`], [`gdpr`] — pre-existing governance
//!   modules (accountability records, capability tokens, GDPR workflows), now
//!   compiled for the first time.
//!
//! # Values versus mechanisms
//!
//! Paper §9.1 requires a *strict channel separation whereby values live in the
//! constitution (governed, versioned, hash-addressed) and mechanisms live in
//! code (audited, invariant-checked)*.
//!
//! Only [`paper_defaults`] holds parameter values. Nothing in [`constitution`]
//! or [`amendment`] reads it: those modules only ever operate on the
//! [`Constitution`] value they are handed, so replacing every default — which
//! is what *"none of these values is hard-coded; all are auditable
//! configuration"* requires — never touches verification code. The test
//! `constitution::tests::mechanism_contains_no_hardcoded_paper_values` fails if
//! a §4.4 number is reintroduced as a literal into a mechanism module.
//!
//! # Getting started
//!
//! ```
//! use arkhe_governance::{paper_defaults, AmendmentClass, ConstitutionStore};
//! use arkhe_governance::constitution::{Fraction, Timelock};
//!
//! let constitution = paper_defaults();
//! let store = ConstitutionStore::new(constitution, chrono::Utc::now())?;
//!
//! // A different deployment can supply entirely different values.
//! let mut custom = paper_defaults();
//! custom.amendment_quorum = Fraction::new(2, 3);
//! custom.amendment_timelock = Timelock::from_days(1);
//! let _ = AmendmentClass::Common;
//! # Ok::<(), arkhe_governance::GovernanceError>(())
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod amendment;
pub mod audit_policy;
pub mod capability;
pub mod constitution;
pub mod gdpr;
pub mod paper_defaults;

pub use amendment::{
    Amendment, AmendmentId, AmendmentState, AppliedAmendment, RejectionReason, VoteOutcome,
};
pub use constitution::{
    AmendmentClass, AuditEntry, AuditEvent, AuditLog, Constitution, ConstitutionStore,
    ConstitutionVersion, Council, Fraction, GovernanceError, GovernorId, Timelock,
    SECONDS_PER_DAY,
};
pub use paper_defaults::{ethics_council, paper_defaults, ETHICS_COUNCIL_SEATS};
