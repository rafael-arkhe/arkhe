//! Integration tests for the declarative adversarial corpus.
//!
//! These tests are the acceptance evidence for the corpus: they assert that
//! every adversarial case is detected at *exactly* its target invariant, that
//! every benign control produces *zero* violations, and that removing only the
//! violating condition turns each adversarial case clean (so the tests are
//! falsifiable rather than always-green).
//!
//! Nothing here writes to disk, spawns a process or touches the network: the
//! fixtures are read-only data and the only code exercised is the pure
//! `arkhe-safe-manifold` API.

use std::collections::BTreeSet;

use arkhe_adversarial_corpus::{
    evaluate, invariant_from_id, invariant_ids, load_corpus, render_report, run_case, witnesses,
    CaseKind, CorpusCase, CAPABILITY_CLASSES,
};
use arkhe_safe_manifold::{
    CapabilitySet, HumanConfirmation, Invariant, ManifoldError, SafeManifold, SafeState,
    SuppressionConfig, SystemConfig, SystemState, TrustedArtifact,
};

/// The seven situations the corpus is required to cover.
const REQUIRED_ADVERSARIAL_IDS: [&str; 7] = [
    "cve-2026-82021",
    "cve-2026-53870",
    "cve-2026-82020",
    "issue-20273",
    "issue-38687",
    "skillignore-self-suppression",
    "import-dynamic-bypass",
];

const EXTENSION_INVARIANTS: [Invariant; 4] =
    [Invariant::I17, Invariant::I18, Invariant::I19, Invariant::I20];

fn corpus() -> Vec<CorpusCase> {
    load_corpus().expect("corpus must load")
}

fn run_all() -> Vec<(CorpusCase, arkhe_adversarial_corpus::CaseOutcome)> {
    corpus()
        .into_iter()
        .map(|case| {
            let outcome = run_case(&case)
                .unwrap_or_else(|err| panic!("case {} must run: {err}", case.id));
            (case, outcome)
        })
        .collect()
}

fn adversarial() -> Vec<(CorpusCase, arkhe_adversarial_corpus::CaseOutcome)> {
    run_all()
        .into_iter()
        .filter(|(case, _)| case.kind == CaseKind::Adversarial)
        .collect()
}

fn benign() -> Vec<(CorpusCase, arkhe_adversarial_corpus::CaseOutcome)> {
    run_all()
        .into_iter()
        .filter(|(case, _)| case.kind == CaseKind::Benign)
        .collect()
}

// ── Corpus shape ────────────────────────────────────────────────────────────

#[test]
fn corpus_shape_is_seven_adversarial_and_four_benign() {
    let cases = corpus();
    assert_eq!(cases.len(), 11, "expected 7 adversarial + 4 benign cases");

    let adversarial_count = cases.iter().filter(|c| c.kind == CaseKind::Adversarial).count();
    let benign_count = cases.iter().filter(|c| c.kind == CaseKind::Benign).count();
    assert_eq!(adversarial_count, 7);
    assert_eq!(benign_count, 4);

    let ids: BTreeSet<&str> = cases.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), cases.len(), "case ids must be unique");
    for required in REQUIRED_ADVERSARIAL_IDS {
        assert!(ids.contains(required), "corpus is missing required case {required}");
    }

    for invariant in EXTENSION_INVARIANTS {
        assert!(
            cases
                .iter()
                .any(|c| c.kind == CaseKind::Adversarial && c.target_invariant == invariant.id()),
            "no adversarial case targets {}",
            invariant.id()
        );
        assert!(
            cases
                .iter()
                .any(|c| c.kind == CaseKind::Benign && c.target_invariant == invariant.id()),
            "no benign control for {}",
            invariant.id()
        );
    }
}

#[test]
fn every_case_targets_one_of_the_four_extension_hooks() {
    for case in corpus() {
        let target = case.target().expect("target resolves");
        assert!(
            EXTENSION_INVARIANTS.contains(&target),
            "case {} targets {} which is outside the I-17..I-20 scope",
            case.id,
            case.target_invariant
        );
        assert!(case.rationale.len() > 40, "case {} needs a real rationale", case.id);
        assert!(case.mechanism.len() > 20, "case {} needs a real mechanism", case.id);
        // Every expected witness key must be one of the three real witnesses.
        for key in case.expected.witness.keys() {
            assert!(
                ["undeclared_capabilities", "tampered_trusted_artifacts", "self_suppressed_files"]
                    .contains(&key.as_str()),
                "case {} declares unknown witness {key:?}",
                case.id
            );
        }
    }
}

#[test]
fn capability_class_names_match_the_real_witness_vocabulary() {
    // If `CapabilitySet` or the witness vocabulary is renamed upstream, this
    // fails loudly instead of silently invalidating every fixture.
    let mut state = SystemState::safe(SystemConfig::default());
    state.declared_capabilities = CapabilitySet::none();
    state.used_capabilities = CapabilitySet::all();
    assert_eq!(state.undeclared_capabilities(), CAPABILITY_CLASSES.to_vec());

    // Every capability name used by every fixture must resolve.
    for case in corpus() {
        case.patched_state().expect("fixture capability names must resolve");
    }
}

// ── Detection (no false negatives) ──────────────────────────────────────────

#[test]
fn adversarial_cases_detect_exactly_the_target_invariant() {
    let pairs = adversarial();
    assert_eq!(pairs.len(), 7);

    for (case, outcome) in pairs {
        let target = case.target().expect("target resolves");
        let baseline = SystemState::safe(case.system_config());

        // The baseline holds the target; only the fixture's patch flips it.
        assert!(
            target.check(&baseline),
            "case {}: {} must already hold on the unpatched baseline",
            case.id,
            target.id()
        );
        assert!(
            outcome.baseline_violations.is_empty(),
            "case {}: baseline must be clean, got {:?}",
            case.id,
            invariant_ids(&outcome.baseline_violations)
        );

        assert_eq!(
            outcome.detected,
            vec![target],
            "case {} must violate exactly {}, observed {:?}",
            case.id,
            target.id(),
            invariant_ids(&outcome.detected)
        );
        assert_eq!(outcome.violation_count, 1);
        assert_eq!(outcome.violation_count as usize, outcome.detected.len());

        // Exactly one of the four gates is false.
        let false_gates: Vec<&str> = EXTENSION_INVARIANTS
            .iter()
            .zip(outcome.extension_predicates)
            .filter(|(_, holding)| !holding)
            .map(|(invariant, _)| invariant.id())
            .collect();
        assert_eq!(false_gates, vec![target.id()]);

        // The witness of the target is non-empty; the others are empty.
        let state = case.patched_state().expect("state builds");
        match target {
            Invariant::I17 => assert!(!state.undeclared_capabilities().is_empty()),
            Invariant::I18 => assert!(!state.tampered_trusted_artifacts().is_empty()),
            Invariant::I19 => assert!(!state.self_suppressed_files().is_empty()),
            Invariant::I20 => {}
            other => panic!("unexpected target {}", other.id()),
        }
    }
}

#[test]
fn every_adversarial_case_is_rejected_by_safe_state_new() {
    for (case, outcome) in adversarial() {
        assert!(
            outcome.safe_state_rejected,
            "case {}: SafeState::new must reject a state with {} violated",
            case.id,
            case.target_invariant
        );
        let message = outcome.safe_state_error.as_deref().unwrap_or_default();
        assert!(
            message.contains("violates 1 invariants"),
            "case {}: unexpected rejection message {message:?}",
            case.id
        );

        let error = SafeState::new(case.patched_state().expect("state builds"))
            .expect_err("SafeState::new must fail");
        assert!(matches!(error, ManifoldError::InvariantViolation(_)));
        assert_eq!(outcome.violation_count, 1);
    }
}

// ── Repair semantics (neron_model) ──────────────────────────────────────────

#[test]
fn neron_model_repairs_i17_i18_i19_but_never_i20() {
    let pairs = adversarial();
    let mut i20_cases = 0usize;

    for (case, outcome) in &pairs {
        let target = case.target().expect("target resolves");
        let state = case.patched_state().expect("state builds");
        let manifold = SafeManifold::with_config(case.system_config());
        let repaired = manifold.neron_model(&state);

        if target == Invariant::I20 {
            i20_cases += 1;
            assert_eq!(outcome.neron_residual, vec![Invariant::I20]);
            assert!(
                !outcome.neron_output_accepted,
                "case {}: a critical operation without confirmation must stay blocked",
                case.id
            );
            assert_eq!(
                outcome.neron_residual, outcome.detected,
                "case {}: neron_model must be a no-op on I-20",
                case.id
            );
            // Directly on the real API: the repair preserves the critical flag
            // and never fabricates a confirmation.
            assert!(repaired.critical_operation);
            assert!(repaired.human_confirmation.is_none());
            assert!(!repaired.check_i20());
            assert!(SafeState::new(repaired).is_err());
        } else {
            assert!(
                outcome.neron_residual.is_empty(),
                "case {}: neron_model must repair {}",
                case.id,
                target.id()
            );
            assert!(outcome.neron_output_accepted);
            assert!(repaired.check_all(), "case {}: repaired state must be safe", case.id);
            assert!(SafeState::new(repaired).is_ok());
        }
    }

    assert_eq!(i20_cases, 1, "exactly one adversarial case must target I-20");
}

// ── Significance: removing the defect makes the case pass ───────────────────

#[test]
fn removing_the_violating_condition_makes_each_adversarial_case_pass() {
    for (case, outcome) in adversarial() {
        assert!(
            case.neutralization.is_some(),
            "case {} must declare a neutralization patch",
            case.id
        );

        let raw = case.patched_state().expect("state builds");
        let neutralized = case
            .neutralized_state()
            .expect("neutralization applies")
            .expect("neutralization present");

        assert_ne!(raw, neutralized, "case {}: neutralization must change the state", case.id);
        assert!(
            !raw.check_all(),
            "case {}: raw state must be unsafe for this test to be meaningful",
            case.id
        );

        // The very same case, with only the defect removed, is now clean.
        assert_eq!(
            neutralized.violations(),
            Vec::new(),
            "case {}: neutralized state must violate nothing",
            case.id
        );
        assert!(neutralized.check_all());
        assert!(
            SafeState::new(neutralized).is_ok(),
            "case {}: neutralized state must be accepted by SafeState::new",
            case.id
        );

        assert_eq!(outcome.neutralized_violations, Some(Vec::<Invariant>::new()));
        assert_eq!(outcome.neutralized_accepted, Some(true));
    }
}

// ── Negative controls (no false positives) ──────────────────────────────────

#[test]
fn benign_controls_produce_zero_violations() {
    let pairs = benign();
    assert_eq!(pairs.len(), 4);

    for (case, outcome) in pairs {
        assert!(
            outcome.detected.is_empty(),
            "case {}: benign control produced false positives {:?}",
            case.id,
            invariant_ids(&outcome.detected)
        );
        assert_eq!(outcome.violation_count, 0);
        assert_eq!(outcome.extension_predicates, [true; 4]);
        assert!(!outcome.safe_state_rejected);
        assert_eq!(outcome.safe_state_error, None);
        assert!(
            outcome.witnesses.values().all(|values| values.is_empty()),
            "case {}: benign control produced witness {:?}",
            case.id,
            outcome.witnesses
        );

        // Repair must be a no-op on an already-safe state.
        assert!(outcome.neron_residual.is_empty());
        assert!(outcome.neron_output_accepted);

        let state = case.patched_state().expect("state builds");
        assert!(state.check_all(), "case {}: benign state must satisfy all 20", case.id);
        assert!(SafeState::new(state).is_ok());
        assert_eq!(outcome.neutralized_violations, None, "benign cases have no neutralization");
    }
}

#[test]
fn benign_i18_control_exercises_the_trusted_scope_boundary() {
    let case = benign()
        .into_iter()
        .find(|(case, _)| case.id == "benign-i18-sealed-and-untrusted")
        .map(|(case, _)| case)
        .expect("control present");
    let state = case.patched_state().expect("state builds");

    assert_eq!(state.trusted_artifacts.len(), 2);
    let untrusted = state
        .trusted_artifacts
        .iter()
        .find(|artifact| !artifact.trusted)
        .expect("untrusted artifact present");
    // The untrusted artifact really does diverge…
    assert_ne!(untrusted.expected_hash, untrusted.observed_hash);
    // …and is still intact because I-18 only covers trusted artifacts.
    assert!(untrusted.is_intact());
    assert!(state.tampered_trusted_artifacts().is_empty());
}

// ── Witnesses and declared expectations ─────────────────────────────────────

#[test]
fn witnesses_match_the_expected_vectors() {
    for (case, outcome) in run_all() {
        for key in ["undeclared_capabilities", "tampered_trusted_artifacts", "self_suppressed_files"] {
            assert!(
                case.expected.witness.contains_key(key),
                "case {}: fixture must declare the {key:?} witness (may be empty)",
                case.id
            );
        }
        for (key, expected) in &case.expected.witness {
            let observed = outcome
                .witnesses
                .get(key)
                .unwrap_or_else(|| panic!("case {}: witness {key:?} missing", case.id));
            assert_eq!(observed, expected, "case {}: witness {key:?}", case.id);
        }
        // The witness map is recomputable from the state, independently.
        let recomputed = witnesses(&case.patched_state().expect("state builds"));
        assert_eq!(recomputed, outcome.witnesses, "case {}: witness map", case.id);
    }
}

#[test]
fn declared_expectations_hold_for_every_case() {
    for case in corpus() {
        let outcome = run_case(&case).expect("case runs");
        let mismatches = evaluate(&case, &outcome);
        assert!(
            mismatches.is_empty(),
            "case {} did not match its declared expectations:\n{}",
            case.id,
            mismatches.join("\n")
        );
    }
}

#[test]
fn evaluate_is_itself_falsifiable() {
    // Guard against a vacuous checker: perturbing an observed outcome must be
    // reported as a mismatch. The I-20 case is used because all three
    // ingredients it relies on are non-trivial there (a detection, a rejection
    // and a non-empty neron residual).
    let case = corpus()
        .into_iter()
        .find(|case| case.id == "issue-38687")
        .expect("the I-20 adversarial case exists");
    let good = run_case(&case).expect("case runs");
    assert!(evaluate(&case, &good).is_empty());

    let mut missing_detection = good.clone();
    missing_detection.detected = Vec::new();
    assert!(
        !evaluate(&case, &missing_detection).is_empty(),
        "evaluate must flag a lost detection"
    );

    let mut accepted_raw = good.clone();
    accepted_raw.safe_state_rejected = false;
    assert!(
        !evaluate(&case, &accepted_raw).is_empty(),
        "evaluate must flag an unexpected acceptance"
    );

    let mut repaired = good.clone();
    repaired.neron_residual = Vec::new();
    assert!(
        !evaluate(&case, &repaired).is_empty(),
        "evaluate must flag a claim that neron repaired what it did not"
    );

    let mut wrong_witness = good.clone();
    wrong_witness
        .witnesses
        .insert("undeclared_capabilities".to_string(), vec!["network".to_string()]);
    assert!(
        !evaluate(&case, &wrong_witness).is_empty(),
        "evaluate must flag a witness that does not match"
    );
}

// ── Gate sensitivity under the public constructors ──────────────────────────

#[test]
fn gates_are_load_bearing_under_the_public_constructors() {
    let baseline = SystemState::safe(SystemConfig::default());
    assert!(baseline.check_all());
    assert!(baseline.violations().is_empty());
    assert_eq!(baseline.violation_count(), 0);

    // I-17
    let mut state = baseline.clone();
    state.declared_capabilities = CapabilitySet { filesystem: true, ..CapabilitySet::none() };
    state.used_capabilities = CapabilitySet { filesystem: true, network: true, ..CapabilitySet::none() };
    assert!(!state.check_i17());
    assert_eq!(state.undeclared_capabilities(), vec!["network"]);
    state.declared_capabilities = CapabilitySet::all();
    assert!(state.check_i17());
    assert!(CapabilitySet::none().is_subset_of(&CapabilitySet::all()));
    assert!(!CapabilitySet::all().is_subset_of(&CapabilitySet::none()));
    assert_eq!(
        CapabilitySet::all().intersect_with(&CapabilitySet::none()),
        CapabilitySet::none()
    );

    // I-18
    let mut state = baseline.clone();
    state.trusted_artifacts = vec![TrustedArtifact::new("a.so", "hash-1", "hash-2")];
    assert!(!state.check_i18());
    assert_eq!(state.tampered_trusted_artifacts(), vec!["a.so"]);

    let mut quarantined = TrustedArtifact::new("a.so", "hash-1", "hash-2");
    quarantined.quarantine();
    state.trusted_artifacts = vec![quarantined];
    assert!(state.check_i18());

    state.trusted_artifacts = vec![TrustedArtifact::untrusted("b.so", "hash-3")];
    assert!(state.check_i18());

    // I-19
    let suppress_all_logs = SuppressionConfig::new(".skillignore", vec!["*.log".to_string()]);
    assert_eq!(suppress_all_logs.suppresses("evidence/audit.log"), Some("*.log"));
    let mut state = baseline.clone();
    state.artifact_files = vec!["evidence/audit.log".to_string()];
    state.suppression = Some(suppress_all_logs);
    assert!(!state.check_i19());
    assert_eq!(state.self_suppressed_files(), vec!["evidence/audit.log".to_string()]);

    state.suppression = Some(SuppressionConfig::new(".skillignore", vec!["tmp/*".to_string()]));
    assert!(state.check_i19());
    assert!(state.self_suppressed_files().is_empty());

    // I-20
    let mut state = baseline.clone();
    assert!(state.check_i20(), "non-critical operations need no confirmation");
    state.critical_operation = true;
    assert!(!state.check_i20());
    state.human_confirmation = Some(HumanConfirmation::denied("operator@arkhe", "deploy-prod"));
    assert!(!state.check_i20(), "an explicit denial must not satisfy I-20");
    state.human_confirmation = Some(HumanConfirmation::granted(
        "operator@arkhe",
        "deploy-prod",
        "2026-09-13T00:00:00Z",
    ));
    assert!(state.check_i20());
    assert!(state.check_all());
}

#[test]
fn i20_denied_confirmation_does_not_satisfy_the_gate() {
    let case = corpus()
        .into_iter()
        .find(|case| case.id == "issue-38687")
        .expect("the skip_confirm case exists");

    // Build the neutralized (granted) state, then flip the decision to a denial.
    let mut denied = case
        .neutralized_state()
        .expect("neutralization applies")
        .expect("neutralization present");
    assert!(denied.check_i20());
    assert!(SafeState::new(denied.clone()).is_ok());

    denied.human_confirmation = Some(HumanConfirmation::denied("operator@arkhe", "deploy-prod"));
    assert_eq!(denied.violations(), vec![Invariant::I20]);
    assert!(!denied.check_i20());
    assert!(SafeState::new(denied).is_err());
}

#[test]
fn invariant_ids_resolve_against_the_real_enum() {
    for invariant in EXTENSION_INVARIANTS {
        assert_eq!(invariant_from_id(invariant.id()).expect("resolves"), invariant);
    }
    assert!(invariant_from_id("I-21").is_err());
    assert!(invariant_from_id("i-17").is_err());
    assert!(invariant_from_id("").is_err());
}

// ── Determinism and reporting ───────────────────────────────────────────────

#[test]
fn corpus_is_deterministic() {
    let first: Vec<_> = corpus().iter().map(|case| run_case(case).expect("runs")).collect();
    let second: Vec<_> = corpus().iter().map(|case| run_case(case).expect("runs")).collect();
    assert_eq!(first, second);
}

#[test]
fn report_lists_every_case_with_its_verdicts() {
    let outcomes: Vec<_> = run_all().into_iter().map(|(_, outcome)| outcome).collect();
    let report = render_report(&outcomes);
    println!("{report}");

    for outcome in &outcomes {
        assert!(report.contains(&outcome.id), "report is missing case {}", outcome.id);
    }
    assert!(report.contains("cases"), "report must carry a summary");
    for expected_line in [
        "false negatives",
        "false positives",
        "neutralization -> clean",
        "I-20 still blocked by neron",
    ] {
        assert!(report.contains(expected_line), "report is missing line {expected_line:?}");
    }
    // Every case must render as repaired except the single I-20 one:
    // 6 adversarial (I-17 x3, I-18 x2, I-19 x1) + 4 benign = 10.
    assert_eq!(report.matches("blocked(I-20)").count(), 1);
    assert_eq!(report.matches("repaired").count(), 10);
}
