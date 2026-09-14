//! FI-120 — canonicalize, hash, then check a record against every
//! registered rule.

use arkhe_core::hash::{blake3_hash, hash_to_hex};
use arkhe_core::ArkheHash;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// A record that can be geometrically verified.
///
/// Implemented by `arkhe-agi`'s `TurnRecord`, which is why this trait lives
/// in a crate `arkhe-agi` depends on and not the other way round: the
/// verifier knows how to read a record, never what a turn is.
///
/// The three accessors are the minimum needed by the rules this workspace
/// actually has — an empty-response check (needs [`response`](VerifiableRecord::response))
/// and a redaction check (needs both halves) — plus
/// [`attested_by`](VerifiableRecord::attested_by), which feeds the
/// canonical form so that an attested turn and an unattested one with the
/// same text hash differently.
///
/// [`attested_by`](VerifiableRecord::attested_by) defaults to `None`, so a
/// record type with no attestation concept only implements the other two.
pub trait VerifiableRecord {
    /// What the user asked.
    fn user_input(&self) -> &str;

    /// What the agent answered.
    fn response(&self) -> &str;

    /// The verified identity attesting this record, if any — typically
    /// `"<display name> (<orcid iD>)"`.
    fn attested_by(&self) -> Option<&str> {
        None
    }
}

/// One invariant a record must satisfy to be admitted.
///
/// `Err` carries the human-readable reason the record was rejected; the
/// verifier attaches the rule's own name, so a rule never has to name
/// itself and cannot misname itself.
///
/// Rules run in registration order and the first failure short-circuits,
/// so a cheap structural check registered first keeps later ones from
/// working on garbage.
///
/// ```
/// use arkhe_geometric_verifier::{VerifiableRecord, VerificationRule};
///
/// struct NonEmptyResponse;
///
/// impl VerificationRule for NonEmptyResponse {
///     fn check(&self, record: &dyn VerifiableRecord) -> Result<(), String> {
///         if record.response().trim().is_empty() {
///             Err("the response is empty".to_string())
///         } else {
///             Ok(())
///         }
///     }
/// }
/// ```
pub trait VerificationRule: Send + Sync {
    /// `Ok(())` if `record` satisfies this rule, `Err(reason)` otherwise.
    fn check(&self, record: &dyn VerifiableRecord) -> Result<(), String>;
}

/// Where a record sits, as a BLAKE3 coordinate.
///
/// Two records with identical canonical bytes land on the same coordinate;
/// any difference — including attestation, which is part of the canonical
/// form — moves it. This is the "coordinate" half of the geometric
/// verification: equality of positions is equality of content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GeometricCoordinate(ArkheHash);

impl GeometricCoordinate {
    /// The coordinate of `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        Self(blake3_hash(bytes))
    }

    /// The raw 32-byte digest.
    pub fn hash(&self) -> &ArkheHash {
        &self.0
    }

    /// The digest as 64 lowercase hex characters.
    pub fn as_hex(&self) -> String {
        hash_to_hex(&self.0)
    }
}

impl fmt::Display for GeometricCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_hex())
    }
}

/// What [`GeometricVerifier::verify`] hands back for an admitted record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedArtifact {
    /// The record's deterministic encoding — see
    /// [`GeometricVerifier::verify`] for the exact layout. This is what
    /// gets appended to the evidence chain, so the chain's tamper-evidence
    /// covers the record's content and not just its hash.
    pub canonical_bytes: Vec<u8>,

    /// BLAKE3 of [`canonical_bytes`](VerifiedArtifact::canonical_bytes).
    pub coordinate: GeometricCoordinate,

    /// Names of the rules that passed, in the order they ran. An empty
    /// vector means no rules are registered — an empty rule set accepts
    /// everything, and says so rather than pretending otherwise.
    pub checks: Vec<String>,
}

/// The one way verification can fail: a registered rule rejected the
/// record.
#[derive(Debug, Error)]
pub enum GeometricError {
    /// `rule` rejected the record, and said why.
    #[error("{rule} rejected the record: {reason}")]
    RuleRejected {
        /// The rejecting rule's type name, as recorded at registration.
        rule: String,
        /// The reason the rule itself supplied.
        reason: String,
    },
}

/// One registered rule, with the label the verifier reports it under.
struct RegisteredRule {
    label: &'static str,
    rule: Box<dyn VerificationRule>,
}

/// Runs `canonicalize -> BLAKE3 -> invariants -> coordinate` over records
/// (FI-120).
///
/// Rules are registered once, at construction, and then applied to every
/// record; there is no per-record configuration. A verifier with no rules
/// registered admits every record — see
/// [`VerifiedArtifact::checks`].
///
/// ```
/// use arkhe_geometric_verifier::{GeometricVerifier, VerifiableRecord, VerificationRule};
///
/// struct Record(&'static str);
/// impl VerifiableRecord for Record {
///     fn user_input(&self) -> &str { self.0 }
///     fn response(&self) -> &str { self.0 }
/// }
///
/// struct NeverEmpty;
/// impl VerificationRule for NeverEmpty {
///     fn check(&self, record: &dyn VerifiableRecord) -> Result<(), String> {
///         if record.response().is_empty() { Err("empty".to_string()) } else { Ok(()) }
///     }
/// }
///
/// let mut verifier = GeometricVerifier::new();
/// verifier.register(NeverEmpty);
///
/// // Same content, same coordinate.
/// let a = verifier.verify(&Record("same")).unwrap();
/// let b = verifier.verify(&Record("same")).unwrap();
/// assert_eq!(a.coordinate, b.coordinate);
///
/// // Any difference moves it.
/// let c = verifier.verify(&Record("different")).unwrap();
/// assert_ne!(a.coordinate, c.coordinate);
///
/// // The rule that ran is on the artifact.
/// assert_eq!(a.checks.len(), 1);
/// assert!(a.checks[0].ends_with("NeverEmpty"));
/// ```
pub struct GeometricVerifier {
    rules: Vec<RegisteredRule>,
}

impl GeometricVerifier {
    /// A verifier with no rules registered. Register some with
    /// [`register`](GeometricVerifier::register) — an unregistered rule
    /// cannot reject anything.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Adds `rule` to the set applied to every verified record.
    ///
    /// Rules run in registration order, and the type name of `R` is what
    /// [`GeometricError::RuleRejected`] reports, so no two rules need to
    /// agree on a shared naming scheme.
    pub fn register<R: VerificationRule + 'static>(&mut self, rule: R) {
        self.rules.push(RegisteredRule {
            label: std::any::type_name::<R>(),
            rule: Box::new(rule),
        });
    }

    /// How many rules are registered.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Canonicalises `record`, hashes it, and runs every registered rule.
    ///
    /// The canonical form is a fixed-order, length-prefixed encoding — 8
    /// big-endian length bytes followed by the UTF-8 bytes — of
    /// [`user_input`](VerifiableRecord::user_input), then
    /// [`response`](VerifiableRecord::response), then a single tag byte
    /// (`1`) and the length-prefixed [`attested_by`](VerifiableRecord::attested_by)
    /// when present, or a `0` tag when absent. Lengths make the encoding
    /// unambiguous — no two different records can produce the same bytes by
    /// shifting a boundary — and the fixed order makes it independent of
    /// field layout, so a record type can gain fields without silently
    /// changing what an already-recorded hash meant.
    ///
    /// Synchronous on purpose: this runs on the turn's critical path in
    /// `AgiCoordinator::process`, where nothing here blocks on I/O.
    pub fn verify<T: VerifiableRecord>(
        &self,
        record: &T,
    ) -> Result<VerifiedArtifact, GeometricError> {
        let canonical_bytes = canonicalize(record);
        let coordinate = GeometricCoordinate::of(&canonical_bytes);

        let mut checks = Vec::with_capacity(self.rules.len());
        for registered in &self.rules {
            if let Err(reason) = registered.rule.check(record) {
                tracing::debug!(
                    rule = registered.label,
                    reason = %reason,
                    "geometric rule rejected a record"
                );
                return Err(GeometricError::RuleRejected {
                    rule: registered.label.to_string(),
                    reason,
                });
            }
            checks.push(registered.label.to_string());
        }

        Ok(VerifiedArtifact {
            canonical_bytes,
            coordinate,
            checks,
        })
    }
}

impl Default for GeometricVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// The canonical encoding described on [`GeometricVerifier::verify`].
fn canonicalize(record: &dyn VerifiableRecord) -> Vec<u8> {
    let mut out = Vec::new();
    push_field(&mut out, record.user_input().as_bytes());
    push_field(&mut out, record.response().as_bytes());
    match record.attested_by() {
        Some(attested_by) => {
            out.push(1);
            push_field(&mut out, attested_by.as_bytes());
        }
        None => out.push(0),
    }
    out
}

/// 8-byte big-endian length, then the bytes.
fn push_field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Record {
        user_input: String,
        response: String,
        attested_by: Option<String>,
    }

    impl Record {
        fn new(user_input: &str, response: &str) -> Self {
            Self {
                user_input: user_input.to_string(),
                response: response.to_string(),
                attested_by: None,
            }
        }

        fn attested(mut self, who: &str) -> Self {
            self.attested_by = Some(who.to_string());
            self
        }
    }

    impl VerifiableRecord for Record {
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

    /// Rejects an empty response, and counts how many times it ran.
    struct NonEmptyResponse {
        runs: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl VerificationRule for NonEmptyResponse {
        fn check(&self, record: &dyn VerifiableRecord) -> Result<(), String> {
            self.runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if record.response().is_empty() {
                Err("the response is empty".to_string())
            } else {
                Ok(())
            }
        }
    }

    /// Always fails; used to prove that a rejection short-circuits.
    struct AlwaysFails;

    impl VerificationRule for AlwaysFails {
        fn check(&self, _record: &dyn VerifiableRecord) -> Result<(), String> {
            Err("never satisfied".to_string())
        }
    }

    fn counting_rule() -> (
        NonEmptyResponse,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        let runs = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        (
            NonEmptyResponse {
                runs: std::sync::Arc::clone(&runs),
            },
            runs,
        )
    }

    #[test]
    fn empty_invariant_set_always_passes() {
        let verifier = GeometricVerifier::new();
        assert_eq!(verifier.rule_count(), 0);

        let artifact = verifier.verify(&Record::new("q", "a")).unwrap();
        assert!(artifact.checks.is_empty());
    }

    #[test]
    fn a_passing_invariant_is_recorded_as_checked() {
        let (rule, runs) = counting_rule();
        let mut verifier = GeometricVerifier::new();
        verifier.register(rule);

        let artifact = verifier.verify(&Record::new("q", "a")).unwrap();

        assert_eq!(artifact.checks.len(), 1);
        assert!(
            artifact.checks[0].ends_with("NonEmptyResponse"),
            "{:?}",
            artifact.checks
        );
        assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn a_passing_artifact_runs_every_registered_invariant() {
        let (first, first_runs) = counting_rule();
        let (second, second_runs) = counting_rule();
        let mut verifier = GeometricVerifier::new();
        verifier.register(first);
        verifier.register(second);

        let artifact = verifier.verify(&Record::new("q", "a")).unwrap();

        assert_eq!(artifact.checks.len(), 2);
        assert_eq!(first_runs.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(second_runs.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn a_failing_invariant_rejects_the_artifact_and_short_circuits() {
        let (after, after_runs) = counting_rule();
        let mut verifier = GeometricVerifier::new();
        verifier.register(AlwaysFails);
        verifier.register(after);

        let err = verifier.verify(&Record::new("q", "a")).unwrap_err();

        assert!(err.to_string().contains("never satisfied"), "{err}");
        assert!(err.to_string().contains("AlwaysFails"), "{err}");
        assert_eq!(
            after_runs.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "the rule after the failing one must not run"
        );
    }

    #[test]
    fn a_rule_rejection_names_the_rule_that_rejected() {
        let (rule, _runs) = counting_rule();
        let mut verifier = GeometricVerifier::new();
        verifier.register(rule);

        let err = verifier.verify(&Record::new("q", "")).unwrap_err();
        match err {
            GeometricError::RuleRejected { rule, reason } => {
                assert!(rule.ends_with("NonEmptyResponse"), "{rule}");
                assert_eq!(reason, "the response is empty");
            }
        }
    }

    #[test]
    fn identical_canonical_bytes_hash_to_the_same_coordinate() {
        let verifier = GeometricVerifier::new();
        let a = verifier.verify(&Record::new("q", "a")).unwrap();
        let b = verifier.verify(&Record::new("q", "a")).unwrap();

        assert_eq!(a.canonical_bytes, b.canonical_bytes);
        assert_eq!(a.coordinate, b.coordinate);
        assert_eq!(a.coordinate.as_hex(), b.coordinate.as_hex());
    }

    #[test]
    fn differing_canonical_bytes_produce_different_coordinates() {
        let verifier = GeometricVerifier::new();
        let base = verifier.verify(&Record::new("q", "a")).unwrap();

        for other in [
            Record::new("q2", "a"),
            Record::new("q", "a2"),
            Record::new("q", "a").attested("someone (0000-0002-1825-0097)"),
        ] {
            let artifact = verifier.verify(&other).unwrap();
            assert_ne!(
                base.canonical_bytes, artifact.canonical_bytes,
                "canonical bytes must differ"
            );
            assert_ne!(
                base.coordinate, artifact.coordinate,
                "coordinates must differ for different content"
            );
        }
    }

    #[test]
    fn attestation_changes_the_canonical_bytes_of_identical_text() {
        let verifier = GeometricVerifier::new();
        let bare = verifier
            .verify(&Record::new("same input", "same output"))
            .unwrap();
        let attested = verifier
            .verify(&Record::new("same input", "same output").attested("Josiah Carberry"))
            .unwrap();

        assert_ne!(bare.canonical_bytes, attested.canonical_bytes);
        assert_ne!(bare.coordinate, attested.coordinate);
    }

    #[test]
    fn canonical_encoding_is_unambiguous_across_field_boundaries() {
        // Length prefixes mean ("ab", "c") and ("a", "bc") cannot collide,
        // even though a naive concatenation would.
        let verifier = GeometricVerifier::new();
        let left = verifier.verify(&Record::new("ab", "c")).unwrap();
        let right = verifier.verify(&Record::new("a", "bc")).unwrap();

        assert_ne!(left.canonical_bytes, right.canonical_bytes);
        assert_ne!(left.coordinate, right.coordinate);
    }

    #[test]
    fn an_absent_attestation_is_distinguishable_from_an_empty_one() {
        let verifier = GeometricVerifier::new();
        let absent = verifier.verify(&Record::new("q", "a")).unwrap();
        let empty = verifier
            .verify(&Record::new("q", "a").attested(""))
            .unwrap();

        assert_ne!(absent.canonical_bytes, empty.canonical_bytes);
    }

    #[test]
    fn the_coordinate_is_blake3_of_the_canonical_bytes() {
        let verifier = GeometricVerifier::new();
        let artifact = verifier.verify(&Record::new("q", "a")).unwrap();

        assert_eq!(
            artifact.coordinate.hash(),
            &blake3_hash(&artifact.canonical_bytes)
        );
        assert_eq!(artifact.coordinate.as_hex().len(), 64);
        assert_eq!(
            artifact.coordinate.to_string(),
            artifact.coordinate.as_hex()
        );
    }

    #[test]
    fn a_verifier_without_rules_is_the_default() {
        let verifier = GeometricVerifier::default();
        assert_eq!(verifier.rule_count(), 0);
        assert!(verifier.verify(&Record::new("q", "a")).is_ok());
    }
}
