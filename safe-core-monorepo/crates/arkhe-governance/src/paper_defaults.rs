//! The §4.4 default parameter values, as auditable configuration.
//!
//! Section 9.1 of the Arkhe paper requires *strict channel separation whereby
//! values live in the constitution (governed, versioned, hash-addressed) and
//! mechanisms live in code (audited, invariant-checked)*.
//!
//! This module is the **values** channel. It is the single place in the crate
//! where the numbers from §4.4 appear, and it does nothing with them beyond
//! writing them into a [`Constitution`] value. No mechanism reads this module:
//! [`crate::constitution`] and [`crate::amendment`] only ever look at the
//! constitution they are handed, so replacing these values — which is what
//! "None of these values is hard-coded; all are auditable configuration"
//! requires — never touches verification code.
//!
//! The test `constitution::tests::mechanism_contains_no_hardcoded_paper_values`
//! enforces the separation in the other direction, and
//! `paper_defaults_is_the_only_home_of_the_paper_values` fails if the values
//! disappear from here.
//!
//! # Provenance
//!
//! | Value | Source |
//! |---|---|
//! | amendment quorum `4/7` | §4.4, P-G1 |
//! | amendment timelock `7 days` | §4.4, P-G2 |
//! | critical quorum `5/7` | §4.4, P-G3 |
//! | critical timelock `14 days` | §4.4, P-G3 |
//! | kill-switch quorum `4/7` | §4.4, P-S1 |
//!
//! The council has seven seats because `4/7` is a fraction *of an ethics
//! council*; the seat identifiers are placeholders, not paper values.
//!
//! ## The one value §4.4 does not state
//!
//! The paper prescribes no deadline for the *voting* window of a proposal, but
//! the deny-on-timeout rule cannot be implemented without one. Rather than
//! inventing a number, the default is derived from P-G2: the window equals the
//! amendment timelock, so a proposal must survive the same period of public
//! visibility as the change it would make. It is ordinary configuration —
//! [`Constitution::proposal_window`] — and can be set to anything.

use crate::constitution::{Constitution, Council, Fraction, GovernorId, Timelock};

/// Number of seats on the ethics council described by §4.4 (`4/7`).
pub const ETHICS_COUNCIL_SEATS: u32 = 7;

/// Returns the ethics council of §4.4 as seven placeholder seat identifiers.
///
/// Membership is a constitutional value like any other: it is hashed, versioned,
/// and can only change through the amendment flow.
pub fn ethics_council() -> Council {
    Council::new(
        (1..=ETHICS_COUNCIL_SEATS)
            .map(|seat| GovernorId::new(format!("ethics-council-seat-{seat}"))),
    )
}

/// Returns a constitution carrying the §4.4 default parameters.
///
/// This is a starting point, not a constraint: every value can be replaced by
/// building a different [`Constitution`], and `ConstitutionStore::validate`
/// rejects only values that are unusable (a zero quorum numerator, a
/// non-positive timelock, an empty council).
pub fn paper_defaults() -> Constitution {
    Constitution {
        version: 1,
        council: ethics_council(),
        amendment_quorum: Fraction::new(4, 7),
        amendment_timelock: Timelock::from_days(7),
        critical_quorum: Fraction::new(5, 7),
        critical_timelock: Timelock::from_days(14),
        kill_switch_quorum: Fraction::new(4, 7),
        proposal_window: Timelock::from_days(7),
    }
}
