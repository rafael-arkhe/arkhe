//! ARKHE adversarial corpus — declarative fixtures for the four constitutional
//! invariants **I‑17..I‑20** added to `arkhe-safe-manifold`.
//!
//! # What this crate is
//!
//! A **data-only** corpus. Each case is a JSON document under `cases/` that
//! describes a `SystemState` patch (capability manifest, trusted artifacts,
//! contained files, artifact-level suppression config, human-confirmation
//! record) plus the expectations that must hold against the *real* public API
//! of `arkhe-safe-manifold`. There is no exploit code anywhere in this crate:
//! no subprocess spawning, no network, no filesystem writes, no `unsafe`.
//!
//! # Invariant hooks under test
//!
//! | Hook  | Predicate                                                        |
//! |-------|------------------------------------------------------------------|
//! | I‑17  | `used_capabilities ⊆ declared_capabilities` (`check_i17`)         |
//! | I‑18  | every `trusted` artifact hash-matches (`check_i18`)               |
//! | I‑19  | the artifact's own config suppresses no contained file (`check_i19`) |
//! | I‑20  | `critical_operation ⇒ confirmed human confirmation` (`check_i20`) |
//!
//! # Two directions of error, both tested
//!
//! * **False negative** — an adversarial fixture that does *not* raise its
//!   target invariant. Tested by [`evaluate`] and by
//!   `tests/corpus.rs::adversarial_cases_detect_exactly_the_target_invariant`.
//! * **False positive** — a benign fixture that raises any invariant. Tested by
//!   `tests/corpus.rs::benign_controls_produce_zero_violations`.
//!
//! # Significance (the corpus must be able to fail)
//!
//! Every adversarial case declares a `neutralization` patch that removes *only*
//! the violating condition. `tests/corpus.rs` asserts that the neutralized
//! state is clean and accepted by [`SafeState::new`]. A test that cannot be made
//! to pass by removing the defect would be worthless; this one can.
//!
//! # Immutability policy
//!
//! The corpus is **immutable by agents**: only a human maintainer edits
//! `cases/*.json`. See `README.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use arkhe_safe_manifold::{
    CapabilitySet, HumanConfirmation, Invariant, SafeManifold, SafeState, SuppressionConfig,
    SystemConfig, SystemState, TrustedArtifact,
};

/// The four capability class names understood by [`CapabilitySet`], in the
/// exact order used by [`SystemState::undeclared_capabilities`].
///
/// This constant is asserted against the real witness vocabulary by
/// `tests/corpus.rs::capability_class_names_match_the_real_witness_vocabulary`,
/// so a rename inside `arkhe-safe-manifold` fails the corpus loudly instead of
/// silently invalidating every fixture.
pub const CAPABILITY_CLASSES: [&str; 4] = ["filesystem", "network", "process", "credentials"];

/// Directory holding the declarative case fixtures shipped with this crate.
pub fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cases")
}

// ========================================================================
// Fixture schema (deserialized from `cases/*.json`)
// ========================================================================

/// Whether a case must be detected (adversarial) or must stay clean (benign).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseKind {
    /// Must raise exactly its `target_invariant`.
    Adversarial,
    /// Must raise no invariant at all (negative control).
    Benign,
}

impl CaseKind {
    /// Lower-case label used in reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Adversarial => "adversarial",
            Self::Benign => "benign",
        }
    }
}

/// A trusted/bundled artifact as written in a fixture.
///
/// Mirrors the public fields of `arkhe_safe_manifold::TrustedArtifact`. A mirror
/// struct is used instead of deserializing the real type directly so that
/// `deny_unknown_fields` catches typos in fixtures, and so that states the
/// convenience constructors normalize away stay expressible — in particular an
/// *untrusted* artifact whose observed hash diverges from its expected hash
/// (I‑18 is out of scope for untrusted artifacts, and the benign I‑18 control
/// depends on being able to express exactly that).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactFixture {
    /// Stable artifact name (e.g. `arkhe-core.so`).
    pub name: String,
    /// Hash recorded when the artifact was sealed as trusted.
    pub expected_hash: String,
    /// Hash recomputed at verification time.
    pub observed_hash: String,
    /// Whether the artifact is marked trusted/bundled.
    pub trusted: bool,
}

impl ArtifactFixture {
    /// Build the real `TrustedArtifact` from the fixture fields.
    ///
    /// Fields are assigned directly (they are public API) rather than through
    /// `TrustedArtifact::new`/`untrusted`, which cannot represent
    /// untrusted-with-divergent-hash. The constructors themselves are exercised
    /// by `tests/corpus.rs::gates_are_load_bearing_under_the_public_constructors`.
    pub fn to_artifact(&self) -> TrustedArtifact {
        TrustedArtifact {
            name: self.name.clone(),
            expected_hash: self.expected_hash.clone(),
            observed_hash: self.observed_hash.clone(),
            trusted: self.trusted,
        }
    }
}

/// The artifact's own declarative suppression config (e.g. `.skillignore`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuppressionFixture {
    /// Path of the config carrying the rules.
    pub config_path: String,
    /// Path patterns excluded from verification; `*` matches any run.
    pub patterns: Vec<String>,
}

impl SuppressionFixture {
    /// Build the real `SuppressionConfig` through its public constructor.
    pub fn to_config(&self) -> SuppressionConfig {
        SuppressionConfig::new(self.config_path.clone(), self.patterns.clone())
    }
}

/// An explicit human decision record for a critical operation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmationFixture {
    /// Identity of the human operator.
    pub operator: String,
    /// Operation/scope the decision refers to.
    pub scope: String,
    /// Whether confirmation was explicitly granted.
    pub confirmed: bool,
    /// Timestamp of the decision (ISO‑8601).
    pub timestamp: String,
}

impl ConfirmationFixture {
    /// Build the real `HumanConfirmation`.
    ///
    /// Fields are assigned directly so a fixture can express a *denial* that
    /// still carries a timestamp; `HumanConfirmation::denied` normalizes the
    /// timestamp to the empty string.
    pub fn to_confirmation(&self) -> HumanConfirmation {
        HumanConfirmation {
            operator: self.operator.clone(),
            scope: self.scope.clone(),
            confirmed: self.confirmed,
            timestamp: self.timestamp.clone(),
        }
    }
}

/// Declarative override of the config-gated `SystemConfig` thresholds.
///
/// The four invariants under test (I‑17..I‑20) are **not** config-gated, so
/// this patch exists only so a fixture can state the ambient compliance regime
/// it is evaluated in. It is applied *before* `SystemState::safe(config)`, which
/// is what seeds the numeric fields from the config.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigPatch {
    /// I‑10 gate: require SBOM verification.
    #[serde(default)]
    pub require_sbom_verification: Option<bool>,
    /// I‑11 gate: require model hash validation.
    #[serde(default)]
    pub require_model_hash_validation: Option<bool>,
    /// I‑15 gate: enable bias detection.
    #[serde(default)]
    pub enable_bias_detection: Option<bool>,
    /// I‑15 threshold.
    #[serde(default)]
    pub max_bias_threshold: Option<f64>,
    /// I‑16 gate: require explainability.
    #[serde(default)]
    pub require_explainability: Option<bool>,
    /// I‑16 threshold.
    #[serde(default)]
    pub min_explainability_threshold: Option<f64>,
}

/// A declarative patch applied on top of `SystemState::safe(config)`.
///
/// Every field is optional: `None` (or an absent key) leaves the baseline value
/// untouched. `suppression` and `human_confirmation` are only ever *set* — no
/// fixture needs to clear them back to `None`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatePatch {
    /// Capability classes declared in the manifest (I‑17).
    #[serde(default)]
    pub declared_capabilities: Option<Vec<String>>,
    /// Capability classes actually used at runtime (I‑17).
    #[serde(default)]
    pub used_capabilities: Option<Vec<String>>,
    /// Bundled/trusted artifacts sealed by hash (I‑18).
    #[serde(default)]
    pub trusted_artifacts: Option<Vec<ArtifactFixture>>,
    /// Files the artifact itself contains (I‑19).
    #[serde(default)]
    pub artifact_files: Option<Vec<String>>,
    /// The artifact's own suppression config (I‑19).
    #[serde(default)]
    pub suppression: Option<SuppressionFixture>,
    /// Whether the pending operation is critical (I‑20).
    #[serde(default)]
    pub critical_operation: Option<bool>,
    /// Registered human confirmation for the pending operation (I‑20).
    #[serde(default)]
    pub human_confirmation: Option<ConfirmationFixture>,
}

/// The expectations a fixture declares for itself.
///
/// Every field is required so a fixture cannot silently skip an assertion.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expectations {
    /// Invariant ids (e.g. `"I-18"`) expected from `SystemState::violations()`.
    pub violations: Vec<String>,
    /// Exact witness vectors, keyed by witness method name. All three witness
    /// keys must be present so no case silently skips witness verification.
    pub witness: BTreeMap<String, Vec<String>>,
    /// Must `SafeState::new` reject the raw (unrepaired) state?
    pub safe_state_rejected: bool,
    /// Must the `neron_model` output be free of violations?
    pub neron_repairs: bool,
    /// Invariant ids expected to survive `neron_model`.
    pub neron_residual_violations: Vec<String>,
    /// Must `SafeState::new` accept the `neron_model` output?
    pub neron_output_accepted: bool,
}

/// One declarative corpus case.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusCase {
    /// Stable case id; must equal the fixture file stem.
    pub id: String,
    /// The single invariant this case is mapped to (`"I-17"`..`"I-20"`).
    pub target_invariant: String,
    /// Adversarial (must be detected) or benign (must stay clean).
    pub kind: CaseKind,
    /// One-line summary.
    pub title: String,
    /// The mechanism the case exercises.
    pub mechanism: String,
    /// Why this hook is the right mapping for the underlying report/CVE.
    pub rationale: String,
    /// Free-form traceability data (CVE id, CWE, path, mode, ...).
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    /// Ambient config regime (applied before `SystemState::safe`).
    #[serde(default)]
    pub config: Option<ConfigPatch>,
    /// The state patch that produces the case.
    pub state: StatePatch,
    /// Adversarial cases only: the patch that removes *only* the violation.
    #[serde(default)]
    pub neutralization: Option<StatePatch>,
    /// What must be observed.
    pub expected: Expectations,
}

impl CorpusCase {
    /// Resolve `target_invariant` against the real `Invariant` enum.
    pub fn target(&self) -> Result<Invariant, String> {
        invariant_from_id(&self.target_invariant)
    }

    /// The ambient system config for this case.
    pub fn system_config(&self) -> SystemConfig {
        let mut config = SystemConfig::default();
        if let Some(patch) = &self.config {
            if let Some(v) = patch.require_sbom_verification {
                config.require_sbom_verification = v;
            }
            if let Some(v) = patch.require_model_hash_validation {
                config.require_model_hash_validation = v;
            }
            if let Some(v) = patch.enable_bias_detection {
                config.enable_bias_detection = v;
            }
            if let Some(v) = patch.max_bias_threshold {
                config.max_bias_threshold = v;
            }
            if let Some(v) = patch.require_explainability {
                config.require_explainability = v;
            }
            if let Some(v) = patch.min_explainability_threshold {
                config.min_explainability_threshold = v;
            }
        }
        config
    }

    /// `SystemState::safe(config)` with this case's patch applied.
    pub fn patched_state(&self) -> Result<SystemState, String> {
        let mut state = SystemState::safe(self.system_config());
        apply_patch(&mut state, &self.state)?;
        Ok(state)
    }

    /// The patched state with `neutralization` additionally applied.
    ///
    /// `None` for cases that do not declare a neutralization (the benign
    /// controls, which are already clean by construction).
    pub fn neutralized_state(&self) -> Result<Option<SystemState>, String> {
        match &self.neutralization {
            None => Ok(None),
            Some(patch) => {
                let mut state = self.patched_state()?;
                apply_patch(&mut state, patch)?;
                Ok(Some(state))
            }
        }
    }
}

// ========================================================================
// Fixture -> real-API mapping
// ========================================================================

/// Parse an invariant id such as `"I-17"` into the real enum.
pub fn invariant_from_id(id: &str) -> Result<Invariant, String> {
    Invariant::all()
        .into_iter()
        .find(|invariant| invariant.id() == id)
        .ok_or_else(|| format!("unknown invariant id {id:?}; expected one of I-01..I-20"))
}

/// Parse a list of invariant ids, preserving order.
pub fn invariants_from_ids(ids: &[String]) -> Result<Vec<Invariant>, String> {
    ids.iter().map(|id| invariant_from_id(id)).collect()
}

/// Invariant ids (e.g. `["I-18"]`) for a slice of [`Invariant`] values.
pub fn invariant_ids(invariants: &[Invariant]) -> Vec<&'static str> {
    invariants.iter().map(Invariant::id).collect()
}

/// Build a [`CapabilitySet`] from fixture class names.
///
/// Rejects unknown names so a typo cannot silently produce an empty set (which
/// would turn an adversarial fixture into a no-op).
pub fn capability_set(names: &[String]) -> Result<CapabilitySet, String> {
    let mut set = CapabilitySet::none();
    for name in names {
        match name.as_str() {
            "filesystem" => set.filesystem = true,
            "network" => set.network = true,
            "process" => set.process = true,
            "credentials" => set.credentials = true,
            other => {
                return Err(format!(
                    "unknown capability class {other:?}; known classes: {CAPABILITY_CLASSES:?}"
                ))
            }
        }
    }
    Ok(set)
}

/// Apply a declarative patch to a live [`SystemState`].
pub fn apply_patch(state: &mut SystemState, patch: &StatePatch) -> Result<(), String> {
    if let Some(names) = &patch.declared_capabilities {
        state.declared_capabilities = capability_set(names)?;
    }
    if let Some(names) = &patch.used_capabilities {
        state.used_capabilities = capability_set(names)?;
    }
    if let Some(artifacts) = &patch.trusted_artifacts {
        state.trusted_artifacts = artifacts.iter().map(ArtifactFixture::to_artifact).collect();
    }
    if let Some(files) = &patch.artifact_files {
        state.artifact_files = files.clone();
    }
    if let Some(suppression) = &patch.suppression {
        state.suppression = Some(suppression.to_config());
    }
    if let Some(critical) = patch.critical_operation {
        state.critical_operation = critical;
    }
    if let Some(confirmation) = &patch.human_confirmation {
        state.human_confirmation = Some(confirmation.to_confirmation());
    }
    Ok(())
}

/// Collect every I‑17..I‑20 violation witness of a state.
pub fn witnesses(state: &SystemState) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    out.insert(
        "undeclared_capabilities".to_string(),
        state
            .undeclared_capabilities()
            .into_iter()
            .map(str::to_string)
            .collect(),
    );
    out.insert(
        "tampered_trusted_artifacts".to_string(),
        state
            .tampered_trusted_artifacts()
            .into_iter()
            .map(str::to_string)
            .collect(),
    );
    out.insert("self_suppressed_files".to_string(), state.self_suppressed_files());
    out
}

// ========================================================================
// Loading
// ========================================================================

/// Load every `cases/*.json` fixture, sorted by file name.
///
/// Enforces that each fixture's `id` equals its file stem and that ids are
/// unique, so a case cannot be unwittingly shadowed or renamed.
pub fn load_corpus_dir(dir: &Path) -> Result<Vec<CorpusCase>, String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|err| format!("cannot read corpus dir {}: {err}", dir.display()))?;

    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| format!("cannot read entry in {}: {err}", dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut cases: Vec<CorpusCase> = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        let case: CorpusCase = serde_json::from_str(&text)
            .map_err(|err| format!("{}: invalid fixture: {err}", path.display()))?;

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("{}: not a valid file stem", path.display()))?;
        if stem != case.id {
            return Err(format!(
                "{}: fixture id {:?} does not match file stem {stem:?}",
                path.display(),
                case.id
            ));
        }
        if cases.iter().any(|existing| existing.id == case.id) {
            return Err(format!("{}: duplicate case id {:?}", path.display(), case.id));
        }
        // Fail fast on unresolvable ids/patch contents, at load time.
        case.target()?;
        invariants_from_ids(&case.expected.violations)?;
        invariants_from_ids(&case.expected.neron_residual_violations)?;
        case.patched_state()?;
        case.neutralized_state()?;
        cases.push(case);
    }

    if cases.is_empty() {
        return Err(format!("no JSON fixtures found in {}", dir.display()));
    }
    Ok(cases)
}

/// Load the corpus bundled with this crate (`cases/`).
pub fn load_corpus() -> Result<Vec<CorpusCase>, String> {
    load_corpus_dir(&corpus_dir())
}

// ========================================================================
// Running
// ========================================================================

/// Everything observed for one case, straight from the public API.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseOutcome {
    /// Case id.
    pub id: String,
    /// Resolved target invariant.
    pub target: Invariant,
    /// Adversarial or benign.
    pub kind: CaseKind,
    /// Violations of the *unpatched* baseline (must be empty).
    pub baseline_violations: Vec<Invariant>,
    /// `SystemState::violations()` on the patched state.
    pub detected: Vec<Invariant>,
    /// `SystemState::violation_count()` on the patched state.
    pub violation_count: u32,
    /// `[check_i17, check_i18, check_i19, check_i20]` on the patched state.
    pub extension_predicates: [bool; 4],
    /// Witness vectors of the patched state.
    pub witnesses: BTreeMap<String, Vec<String>>,
    /// Did `SafeState::new` reject the patched state?
    pub safe_state_rejected: bool,
    /// The rejection message, when rejected.
    pub safe_state_error: Option<String>,
    /// `neron_model` residual violations.
    pub neron_residual: Vec<Invariant>,
    /// Did `SafeState::new` accept the `neron_model` output?
    pub neron_output_accepted: bool,
    /// `violations()` after applying the neutralization patch.
    pub neutralized_violations: Option<Vec<Invariant>>,
    /// Did `SafeState::new` accept the neutralized state?
    pub neutralized_accepted: Option<bool>,
}

/// Run one case against the real API.
///
/// The manifold is built with [`SafeManifold::with_config`] so that
/// `neron_model` repairs under the same config the state was built with.
pub fn run_case(case: &CorpusCase) -> Result<CaseOutcome, String> {
    let target = case.target()?;
    let config = case.system_config();
    let manifold = SafeManifold::with_config(config.clone());

    let baseline = SystemState::safe(config);
    let baseline_violations = baseline.violations();

    let state = case.patched_state()?;
    let detected = state.violations();
    let violation_count = state.violation_count();
    let extension_predicates = [
        state.check_i17(),
        state.check_i18(),
        state.check_i19(),
        state.check_i20(),
    ];
    let witnesses = witnesses(&state);

    let (safe_state_rejected, safe_state_error) = match SafeState::new(state.clone()) {
        Ok(_) => (false, None),
        Err(err) => (true, Some(err.to_string())),
    };

    let repaired = manifold.neron_model(&state);
    let neron_residual = repaired.violations();
    let neron_output_accepted = SafeState::new(repaired).is_ok();

    let (neutralized_violations, neutralized_accepted) = match case.neutralized_state()? {
        None => (None, None),
        Some(neutralized) => {
            let violations = neutralized.violations();
            let accepted = SafeState::new(neutralized).is_ok();
            (Some(violations), Some(accepted))
        }
    };

    Ok(CaseOutcome {
        id: case.id.clone(),
        target,
        kind: case.kind,
        baseline_violations,
        detected,
        violation_count,
        extension_predicates,
        witnesses,
        safe_state_rejected,
        safe_state_error,
        neron_residual,
        neron_output_accepted,
        neutralized_violations,
        neutralized_accepted,
    })
}

/// Run every case, in load order.
pub fn run_corpus(cases: &[CorpusCase]) -> Result<Vec<CaseOutcome>, String> {
    cases.iter().map(run_case).collect()
}

/// Check a fixture's declared expectations against what was observed.
///
/// Returns one message per mismatch; an empty vector means the case passed.
pub fn evaluate(case: &CorpusCase, outcome: &CaseOutcome) -> Vec<String> {
    let mut mismatches: Vec<String> = Vec::new();
    let expected = &case.expected;

    let expected_violations = match invariants_from_ids(&expected.violations) {
        Ok(v) => v,
        Err(err) => return vec![err],
    };
    let expected_residual = match invariants_from_ids(&expected.neron_residual_violations) {
        Ok(v) => v,
        Err(err) => return vec![err],
    };
    let target = match case.target() {
        Ok(t) => t,
        Err(err) => return vec![err],
    };

    if !outcome.baseline_violations.is_empty() {
        mismatches.push(format!(
            "baseline (unpatched SystemState::safe) is not clean: {:?}",
            invariant_ids(&outcome.baseline_violations)
        ));
    }

    if outcome.detected != expected_violations {
        mismatches.push(format!(
            "violations: expected {:?}, observed {:?}",
            expected.violations,
            invariant_ids(&outcome.detected)
        ));
    }

    if outcome.kind == CaseKind::Adversarial {
        if expected_violations != vec![target] {
            mismatches.push(format!(
                "adversarial case {} must declare exactly its target {:?}, declared {:?}",
                case.id,
                target.id(),
                expected.violations
            ));
        }
        if case.neutralization.is_none() {
            mismatches.push(format!(
                "adversarial case {} must declare a neutralization patch",
                case.id
            ));
        }
    }

    for (key, expected_vector) in &expected.witness {
        match outcome.witnesses.get(key) {
            None => mismatches.push(format!("witness {key:?} was never collected")),
            Some(observed) if observed != expected_vector => mismatches.push(format!(
                "witness {key:?}: expected {expected_vector:?}, observed {observed:?}"
            )),
            Some(_) => {}
        }
    }
    for key in ["undeclared_capabilities", "tampered_trusted_artifacts", "self_suppressed_files"] {
        if !expected.witness.contains_key(key) {
            mismatches.push(format!(
                "fixture must declare the {key:?} witness vector (may be empty)"
            ));
        }
    }

    if outcome.safe_state_rejected != expected.safe_state_rejected {
        mismatches.push(format!(
            "SafeState::new rejection: expected {}, observed {} ({:?})",
            expected.safe_state_rejected, outcome.safe_state_rejected, outcome.safe_state_error
        ));
    }

    if outcome.neron_residual != expected_residual {
        mismatches.push(format!(
            "neron_model residual: expected {:?}, observed {:?}",
            expected.neron_residual_violations,
            invariant_ids(&outcome.neron_residual)
        ));
    }

    if expected.neron_repairs != outcome.neron_residual.is_empty() {
        mismatches.push(format!(
            "neron_repairs={} is inconsistent with residual {:?}",
            expected.neron_repairs,
            invariant_ids(&outcome.neron_residual)
        ));
    }

    if outcome.neron_output_accepted != expected.neron_output_accepted {
        mismatches.push(format!(
            "SafeState::new(neron_model(..)): expected {}, observed {}",
            expected.neron_output_accepted, outcome.neron_output_accepted
        ));
    }

    if case.neutralization.is_some() {
        match &outcome.neutralized_violations {
            None => mismatches.push("neutralization declared but never evaluated".to_string()),
            Some(violations) => {
                if !violations.is_empty() {
                    mismatches.push(format!(
                        "neutralized state still violates {:?}",
                        invariant_ids(violations)
                    ));
                }
                if outcome.neutralized_accepted != Some(true) {
                    mismatches.push(format!(
                        "neutralized state not accepted by SafeState::new: {:?}",
                        outcome.neutralized_accepted
                    ));
                }
            }
        }
    }

    mismatches
}

/// Render the per-case runner report.
pub fn render_report(outcomes: &[CaseOutcome]) -> String {
    let mut out = String::new();
    out.push_str("ARKHE adversarial corpus - constitutional invariants I-17..I-20 (arkhe-safe-manifold)\n");
    out.push_str(&format!(
        "{:<34} {:<6} {:<12} {:<10} {:<9} {:<16} {:<10} {}\n",
        "case", "target", "kind", "detected", "rejected", "neron_model", "neutralized", "I17-I20"
    ));
    out.push_str(&format!("{}\n", "-".repeat(118)));

    for outcome in outcomes {
        let detected = if outcome.detected.is_empty() {
            "-".to_string()
        } else {
            invariant_ids(&outcome.detected).join(",")
        };
        let rejected = if outcome.safe_state_rejected { "rejected" } else { "accepted" };
        let neron = if outcome.neron_residual.is_empty() {
            "repaired".to_string()
        } else {
            format!("blocked({})", invariant_ids(&outcome.neron_residual).join(","))
        };
        let neutralized = match (&outcome.neutralized_violations, outcome.neutralized_accepted) {
            (None, _) => "-".to_string(),
            (Some(violations), Some(true)) if violations.is_empty() => "clean".to_string(),
            (Some(violations), _) => format!("still({})", invariant_ids(violations).join(",")),
        };
        let predicates = outcome
            .extension_predicates
            .iter()
            .map(|holding| if *holding { 'T' } else { 'F' })
            .collect::<String>();

        out.push_str(&format!(
            "{:<34} {:<6} {:<12} {:<10} {:<9} {:<16} {:<10} {}\n",
            outcome.id,
            outcome.target.id(),
            outcome.kind.as_str(),
            detected,
            rejected,
            neron,
            neutralized,
            predicates
        ));
    }

    let adversarial = outcomes.iter().filter(|o| o.kind == CaseKind::Adversarial).count();
    let benign = outcomes.iter().filter(|o| o.kind == CaseKind::Benign).count();
    let false_negatives = outcomes
        .iter()
        .filter(|o| o.kind == CaseKind::Adversarial && o.detected != vec![o.target])
        .count();
    let false_positives = outcomes
        .iter()
        .filter(|o| o.kind == CaseKind::Benign && !o.detected.is_empty())
        .count();
    let neutralized_clean = outcomes
        .iter()
        .filter(|o| o.neutralized_violations.as_ref().is_some_and(|v| v.is_empty()))
        .count();
    let i20_blocked = outcomes
        .iter()
        .filter(|o| o.target == Invariant::I20 && o.neron_residual == vec![Invariant::I20])
        .count();

    out.push_str(&format!("\n{:<34} {}\n", "cases", outcomes.len()));
    out.push_str(&format!("{:<34} {}\n", "adversarial / benign", format!("{adversarial} / {benign}")));
    out.push_str(&format!("{:<34} {}\n", "false negatives", false_negatives));
    out.push_str(&format!("{:<34} {}\n", "false positives", false_positives));
    out.push_str(&format!(
        "{:<34} {}\n",
        "neutralization -> clean", neutralized_clean
    ));
    out.push_str(&format!("{:<34} {}\n", "I-20 still blocked by neron", i20_blocked));
    out
}
