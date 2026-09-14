# ARKHE adversarial corpus — SafeManifold invariants I-17..I-20

Declarative, **executable** adversarial corpus for the four constitutional
invariants added to `packages/arkhe-safe-manifold` (`arkhe-safe-manifold`
v0.8.0): **I-17**, **I-18**, **I-19** and **I-20**.

The corpus has two jobs:

1. **Detect** each of seven reported situations at exactly the invariant that
   gates it (no false negatives), and
2. **Prove zero false positives** on four benign controls — including two
   controls that are deliberately built to trip a *naive* implementation
   (over-declaration in a capability manifest, and an untrusted artifact whose
   hashes diverge).

---

## 1. Objective

`arkhe-safe-manifold` gained four invariants whose predicates are boolean gates
over new state fields:

| Hook | Predicate (real API) | Category | Witness method |
|------|----------------------|----------|----------------|
| **I-17** | `used_capabilities ⊆ declared_capabilities` (`SystemState::check_i17`) | Capability | `SystemState::undeclared_capabilities()` |
| **I-18** | every `trusted` artifact hash-matches (`SystemState::check_i18`) | Integrity | `SystemState::tampered_trusted_artifacts()` |
| **I-19** | the artifact's own config suppresses no file it contains (`SystemState::check_i19`) | Evidence | `SystemState::self_suppressed_files()` |
| **I-20** | `critical_operation ⇒ HumanConfirmation { confirmed: true }` (`SystemState::check_i20`) | HumanOversight | *(no witness vector — boolean gate)* |

This corpus encodes the reported CVEs/issues as **declarative fixtures** and
asserts, against the real API, that each one is caught by the right gate and
repaired (or not) exactly as documented.

### Non-negotiable property: the fixtures are data, not payloads

Every case is a JSON document describing a *state*. There is **no** exploit
code, no command execution, no network access, no process spawning, no disk
writes and no `unsafe` anywhere in this crate (`#![forbid(unsafe_code)]`). The
runner only reads `cases/*.json` and calls the pure `arkhe-safe-manifold` API.

---

## 2. Case → invariant → expected result

`detected` is `SystemState::violations()` on the raw fixture state.
`rejected` is `SafeState::new(..).is_err()`. `neron` is what
`SafeManifold::neron_model` leaves behind. `neutralized` is the state with the
violating condition removed.

### 2.1 Adversarial cases (must be detected)

| Case id | Target | Situation | Expected violations | `SafeState::new` | `neron_model` | Neutralization → clean |
|---------|--------|-----------|--------------------|------------------|---------------|------------------------|
| `cve-2026-82021` | **I-18** | dependency pinned to a mutable branch; sealed artifact bytes change | `["I-18"]` | rejected | repaired (quarantine) | yes |
| `cve-2026-53870` | **I-17** | file created `0o644` (world-readable) using an undeclared `filesystem` capability | `["I-17"]` | rejected | repaired (drops undeclared use) | yes |
| `cve-2026-82020` | **I-17** | write outside declared scope to `~/.hermes/auth.json` using undeclared `filesystem` + `credentials` | `["I-17"]` | rejected | repaired | yes |
| `issue-20273` | **I-18** | bundled skill `SKILL.md` modified after sealing → hash divergence | `["I-18"]` | rejected | repaired (quarantine) | yes |
| `issue-38687` | **I-20** | `skip_confirm=true` on a critical operation, no confirmation registered | `["I-20"]` | rejected | **NOT repaired — stays blocked on I-20** | yes |
| `skillignore-self-suppression` | **I-19** | artifact's own `.skillignore` suppresses `evidence/audit.log`, which the artifact contains | `["I-19"]` | rejected | repaired (drops the config) | yes |
| `import-dynamic-bypass` | **I-17** | dynamic import loads a plugin that spawns a process; `process` class undeclared | `["I-17"]` | rejected | repaired | yes |

### 2.2 Benign controls (negative controls — must stay clean)

| Case id | Target | Situation | Expected violations | `SafeState::new` |
|---------|--------|-----------|--------------------|------------------|
| `benign-i17-complete-manifest` | I-17 | `declared ⊋ used` — over-declaration is *not* a violation | `[]` | accepted |
| `benign-i18-sealed-and-untrusted` | I-18 | trusted artifact intact **+** untrusted artifact with diverging hashes (out of scope) | `[]` | accepted |
| `benign-i19-scoped-suppression` | I-19 | `.skillignore` present but its patterns match no contained file | `[]` | accepted |
| `benign-i20-granted-confirmation` | I-20 | critical operation **with** a granted confirmation, under a stricter compliance config | `[]` | accepted |

Two of these are aimed at specific over-reach:

* `benign-i17-complete-manifest` fails if a gate demanded set **equality**
  instead of the subset relation the predicate states.
* `benign-i18-sealed-and-untrusted` fails if a gate checked hash equality
  without honouring the `trusted` flag — `TrustedArtifact::is_intact()`
  short-circuits on `!trusted`, so untrusted artifacts always satisfy I-18.

### 2.3 Why each report is mapped to that hook

The full justification is stored per case in the fixture's `rationale` field and
is printed by the runner. Summary:

* **`cve-2026-82021` → I-18.** A mutable ref is defined by the failure of
  "the observed bytes of a trusted artifact still equal the sealed bytes",
  which is exactly I-18's predicate. I-17/I-19 describe what a compromised
  dependency may do *after* it is admitted; I-20 governs human oversight of a
  critical operation. The fixture declares a complete manifest, no suppression
  config and no critical operation, so I-18 is the only load-bearing gate.
* **`cve-2026-53870` → I-17.** The report's essential defect is an undeclared
  effect: a persistent world-readable file produced under a manifest that
  authorised only `process`.
* **`cve-2026-82020` → I-17.** Scope escape *is* the capability manifest's
  purpose. I-19 would fit only if a suppression config hid the affected paths
  from audit — none is present, so I-19 holds vacuously.
* **`issue-20273` → I-18.** Overwriting a bundled artifact is literally sealed
  bytes diverging.
* **`issue-38687` → I-20.** A confirmation-bypass flag maps onto the human
  oversight predicate and onto nothing else.
* **`skillignore-self-suppression` → I-19.** The predicate is exactly
  "the artifact's own config suppresses a file the artifact contains".
* **`import-dynamic-bypass` → I-17.** The whole point of the bypass is that
  static analysis never sees the import, so the capability it needs is never
  declared, while the runtime state records the use.

---

## 3. How to run

From `arkhe-monorepo/`:

```bash
# The tests (17 tests; acceptance evidence)
cargo test -p arkhe-adversarial-corpus

# With the per-case report and rationale printed
cargo test -p arkhe-adversarial-corpus -- --nocapture

# The standalone runner: prints the verdict table + per-case detail,
# exits non-zero if any case deviates from its declared expectations
cargo run -p arkhe-adversarial-corpus
```

The tests need no network, no credentials and no writable state.

### What the tests assert

| Test | Property |
|------|----------|
| `corpus_shape_is_seven_adversarial_and_four_benign` | 7 + 4 cases, unique ids, all seven required situations present, I-17..I-20 covered in both directions |
| `adversarial_cases_detect_exactly_the_target_invariant` | `violations() == [target]` exactly; the target held on the unpatched baseline; exactly one of the four gates is false |
| `every_adversarial_case_is_rejected_by_safe_state_new` | `SafeState::new` rejects with `ManifoldError::InvariantViolation` |
| `neron_model_repairs_i17_i18_i19_but_never_i20` | repaired states are accepted; the I-20 state keeps `critical_operation` and stays blocked |
| `removing_the_violating_condition_makes_each_adversarial_case_pass` | **significance**: with only the defect removed, `violations() == []` and `SafeState::new` accepts |
| `benign_controls_produce_zero_violations` | **no false positives**: zero violations, all four gates true, all witnesses empty |
| `witnesses_match_the_expected_vectors` | witness vectors equal the fixture's declared expectations, and are recomputable independently |
| `declared_expectations_hold_for_every_case` | every fixture's `expected` block matches observation (the fixtures are self-checking) |
| `evaluate_is_itself_falsifiable` | the checker itself flags perturbed outcomes (no vacuous assertions) |
| `gates_are_load_bearing_under_the_public_constructors` | each gate flips under `TrustedArtifact::new`/`quarantine`/`untrusted`, `SuppressionConfig::new`, `HumanConfirmation::granted`/`denied`, `CapabilitySet` algebra |
| `i20_denied_confirmation_does_not_satisfy_the_gate` | `confirmed: false` is a denial, not a confirmation |
| `capability_class_names_match_the_real_witness_vocabulary` | the fixture vocabulary is pinned to the real witness ordering |
| `invariant_ids_resolve_against_the_real_enum` | id parsing rejects unknown/lower-case ids |
| `corpus_is_deterministic` | two runs produce identical outcomes |
| `report_lists_every_case_with_its_verdicts` | the runner report covers every case and every summary line |

### Falsifiability (why a green run means something)

A test that always passes is worthless. Two independent mechanisms prevent
that:

1. `removing_the_violating_condition_makes_each_adversarial_case_pass` — each
   adversarial fixture carries a `neutralization` patch that removes *only* the
   violating condition; the suite asserts the neutralized state is clean and
   accepted. A case that could not be made to pass would fail here.
2. Mutation experiment (performed, reproducible by hand): removing the violating
   condition from one representative case per hook (I-17..I-20) turns the suite
   **red** — 9 of 17 tests fail and the runner reports
   `false negatives = 4`, exit code 101. Restoring the fixtures returns the
   suite to green.

---

## 4. Layout

```
tests/adversarial/
├── Cargo.toml                  crate `arkhe-adversarial-corpus` (workspace member)
├── README.md                   this file
├── cases/                      11 declarative JSON fixtures (data only)
│   ├── cve-2026-82021.json               adversarial  -> I-18
│   ├── cve-2026-53870.json               adversarial  -> I-17
│   ├── cve-2026-82020.json               adversarial  -> I-17
│   ├── issue-20273.json                  adversarial  -> I-18
│   ├── issue-38687.json                  adversarial  -> I-20
│   ├── skillignore-self-suppression.json adversarial  -> I-19
│   ├── import-dynamic-bypass.json        adversarial  -> I-17
│   ├── benign-i17-complete-manifest.json benign       -> I-17
│   ├── benign-i18-sealed-and-untrusted.json benign    -> I-18
│   ├── benign-i19-scoped-suppression.json benign      -> I-19
│   └── benign-i20-granted-confirmation.json benign    -> I-20
├── src/
│   ├── lib.rs                  fixture schema, loader, runner, expectations checker
│   └── main.rs                 standalone runner (bin `arkhe-adversarial-corpus`)
└── tests/
    └── corpus.rs               17 integration tests (acceptance evidence)
```

### Fixture schema

A fixture is JSON with `deny_unknown_fields` on every struct, so a typo is a
load error rather than a silently inert case. The loader additionally enforces
that `id` equals the file stem and that ids are unique.

```jsonc
{
  "id": "…",                        // must equal the file stem
  "target_invariant": "I-17",       // resolved against the real Invariant enum
  "kind": "adversarial" | "benign",
  "title": "…", "mechanism": "…", "rationale": "…",
  "metadata": { },                  // free-form traceability (CVE, CWE, path, mode…)
  "config": { },                    // optional SystemConfig override (applied before SystemState::safe)
  "state": {                        // declarative patch over SystemState::safe(SystemConfig::default())
    "declared_capabilities": ["filesystem", "network", "process", "credentials"],
    "used_capabilities": [ ],
    "trusted_artifacts": [ { "name": "…", "expected_hash": "…", "observed_hash": "…", "trusted": true } ],
    "artifact_files": [ "…" ],
    "suppression": { "config_path": ".skillignore", "patterns": [ "…" ] },
    "critical_operation": false,
    "human_confirmation": { "operator": "…", "scope": "…", "confirmed": true, "timestamp": "…" }
  },
  "neutralization": { },            // adversarial only: removes ONLY the violation
  "expected": {                     // every field required; no silent skips
    "violations": ["I-17"],
    "witness": { "undeclared_capabilities": [], "tampered_trusted_artifacts": [], "self_suppressed_files": [] },
    "safe_state_rejected": true,
    "neron_repairs": true,
    "neron_residual_violations": [],
    "neron_output_accepted": true
  }
}
```

A patch field that is absent (or `null`) leaves the baseline value untouched.
The baseline is `SystemState::safe(SystemConfig::default())`, which satisfies all
of I-01..I-20; every test also asserts the baseline is clean, so a discrepancy
introduced by the baseline can never be mistaken for the case's own effect.

---

## 5. Immutability policy

**The corpus is immutable by agents. Only a human maintainer edits
`cases/*.json`.**

Rationale: the fixtures are the *oracle* for I-17..I-20. An agent that edits a
fixture while trying to make a failing test pass would be moving the target
instead of fixing the defect — the exact failure mode this corpus exists to
catch. In particular:

* An agent **may** add a new `cases/<id>.json` after a new situation is reported
  and reviewed by a human.
* An agent **must not** edit an existing fixture's `state`, `neutralization` or
  `expected` block, weaken an assertion, or delete a case in order to obtain a
  green run.
* Any change to an existing fixture must come with human sign-off and must keep
  the `rationale` field truthful.
* If a shipped predicate changes meaning, the fixture must be updated *by a
  human* together with the invariant's documentation — never silently.

The loader's `id == file stem` and `deny_unknown_fields` rules make accidental
drift noisy; they are not a substitute for the policy above.

---

## 6. Known limitations (honest)

1. **I-17 cannot express permission bits.** `CapabilitySet` has exactly four
   coarse classes (`filesystem`, `network`, `process`, `credentials`). The
   `0o644` mode in `cve-2026-53870` is therefore recorded in `metadata` and the
   gate that actually fires is the undeclared `filesystem` class. A fixture
   cannot distinguish `0o644` from `0o600` through the predicate.
2. **`~/.hermes/auth.json` is a string, never touched.** The path in
   `cve-2026-82020` is metadata. Nothing in the corpus resolves, reads or writes
   that path (or any other path).
3. **Hash values are opaque placeholders.** I-18 only compares for equality, so
   the fixtures use readable stand-ins (`blake3:sealed-…`). The corpus does not
   hash real bytes; it tests the *gate*, not a hash implementation.
4. **`neutralization` is a mechanism, not a remediation.** It exists to prove
   the gate is load-bearing. For `cve-2026-82021` it re-seals an artifact to the
   bytes that were observed — that removes the violating condition but is **not**
   what an operator should do; the real fix is to pin an immutable digest under
   human review. Each fixture states its own `neutralization_semantics`.
5. **Pure `SystemState` level only.** The corpus exercises the invariant gates,
   not the surrounding I/O (loading artifacts from disk, computing real hashes,
   reading real `.skillignore` files). Those layers are outside this task's
   scope.
6. **No Prolog/audit feature coverage.** The corpus uses `arkhe-safe-manifold`
   with its default features; the feature-gated modules are untouched.
