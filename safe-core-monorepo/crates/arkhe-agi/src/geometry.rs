//! The record FI-120 verifies, and the invariant every admitted turn must
//! satisfy.
//!
//! [`TurnRecord`] is the shape `AgiCoordinator::process` builds once the
//! inference call has returned and any tool output has been appended: the
//! user's question, the agent's answer, and who attested it. It is the only
//! [`VerifiableRecord`] in the workspace — the trait itself lives in
//! `arkhe-geometric-verifier`, which knows how to read a record and nothing
//! about what a turn is.
//!
//! Because `attested_by` is part of the canonical form the verifier hashes,
//! an attested turn and an unattested one with identical text land on
//! different coordinates; that is what makes provenance hashes attributable.
//!
//! [`NonEmptyResponse`] is registered first by `AgiCoordinator::new`,
//! before the redaction rule: it is the cheap structural check, and a turn
//! with an empty answer should be rejected for *that* reason rather than
//! for whatever a later rule would have found inside it.

use arkhe_geometric_verifier::{VerifiableRecord, VerificationRule};

/// One completed conversation turn, in the shape the verifier canonicalises.
///
/// Fields are public and named exactly as the struct literal in
/// `coordinator.rs` spells them — the coordinator builds one per `process()`
/// call and hands it to [`GeometricVerifier::verify`](arkhe_geometric_verifier::GeometricVerifier::verify)
/// with no `.await`, on the turn's critical path, *before* the turn is
/// admitted into the evidence chain.
///
/// [`response`](TurnRecord::response) is the whole assistant reply including
/// any `[tool:<name>] …` block the coordinator appended, so the provenance
/// record covers what the caller actually received, not just what the model
/// said.
#[derive(Debug, Clone)]
pub struct TurnRecord {
    /// What the user asked, verbatim.
    pub user_input: String,

    /// What the agent answered, with tool output already folded in.
    pub response: String,

    /// `"<display name> (<orcid iD>)"` of the verified attestor — set once
    /// `AgiCoordinator::attest_with_orcid` has succeeded, `None` otherwise.
    pub attested_by: Option<String>,
}

impl VerifiableRecord for TurnRecord {
    fn user_input(&self) -> &str {
        &self.user_input
    }

    fn response(&self) -> &str {
        &self.response
    }

    fn attested_by(&self) -> Option<&str> {
        self.attested_by.as_deref()
    }
}

/// Rejects a turn whose response is empty or whitespace-only.
///
/// A model that returned nothing, or nothing but a newline, has not answered
/// the question; admitting such a turn would put a record in the evidence
/// chain and a value in memory for an answer that does not exist, and every
/// later provenance hash would be chained to it.
///
/// Whitespace-only counts as empty: `trim()` is what decides, so `"  \n "`
/// is rejected exactly like `""`. The rejected reason is the same string in
/// both cases — for a consumer, "the response is empty" and "the response
/// has nothing in it" are the same failure.
///
/// Registered by `AgiCoordinator::new` before
/// [`NoUnredactedCpf`](crate::compliance::NoUnredactedCpf), so an empty
/// answer is reported as an empty answer and not as whatever else it also
/// happens to contain.
#[derive(Debug, Clone, Copy)]
pub struct NonEmptyResponse;

impl VerificationRule for NonEmptyResponse {
    fn check(&self, record: &dyn VerifiableRecord) -> Result<(), String> {
        if record.response().trim().is_empty() {
            Err("the response is empty".to_string())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_geometric_verifier::GeometricVerifier;

    fn turn(user_input: &str, response: &str) -> TurnRecord {
        TurnRecord {
            user_input: user_input.to_string(),
            response: response.to_string(),
            attested_by: None,
        }
    }

    #[test]
    fn an_empty_response_is_rejected() {
        let rule = NonEmptyResponse;
        let err = rule.check(&turn("what?", "")).unwrap_err();
        assert_eq!(err, "the response is empty");
    }

    #[test]
    fn a_whitespace_only_response_is_rejected() {
        let rule = NonEmptyResponse;
        assert!(rule.check(&turn("what?", " \n\t ")).is_err());
    }

    #[test]
    fn an_answer_with_content_passes() {
        let rule = NonEmptyResponse;
        assert!(rule.check(&turn("what?", "this")).is_ok());
    }

    #[test]
    fn the_rule_is_what_the_verifier_reports_as_rejecting() {
        let mut verifier = GeometricVerifier::new();
        verifier.register(NonEmptyResponse);

        let err = verifier.verify(&turn("q", "   ")).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("NonEmptyResponse"), "{message}");
        assert!(message.contains("the response is empty"), "{message}");
    }

    #[test]
    fn attestation_moves_the_coordinate() {
        let verifier = GeometricVerifier::new();

        let bare = verifier.verify(&turn("q", "a")).unwrap();
        let attested = verifier
            .verify(&TurnRecord {
                user_input: "q".to_string(),
                response: "a".to_string(),
                attested_by: Some("Josiah Carberry (0000-0002-1825-0097)".to_string()),
            })
            .unwrap();

        assert_ne!(bare.coordinate, attested.coordinate);
    }
}
