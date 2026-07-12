# arkhe-rsi

FI-050 — self-limited recursive self-improvement: a candidate change to the
system can only become active through a pipeline that (1) validates and
evaluates it against the real toolchain, (2) requires human quorum
approval with single-vote veto, (3) verifies the resulting history against
hard structural invariants before persisting it, and (4) can always be
rolled back to the last stable checkpoint — automatically, if a verifier
upstream of this crate detects an excessive score drop. No step is
optional or bypassable from inside this crate: `Registry::append` is the
only way to extend the history, and it always runs through `Verifier`
first.

## Status

38/38 tests pass (35 unit + 3 integration in `tests/composition.rs`).
Verified: `../docs/verification/README.md` (run from `safe-core-monorepo/`).

## What's actually here

- **`validator.rs` / `evaluator.rs`** — `RustClippyValidator` (static:
  `cargo check` + `cargo clippy`, both with `--message-format=json` for
  structured diagnostics instead of scraping human-formatted text) and
  `CargoTestEvaluator` (dynamic: real `cargo build` + `cargo test`, scored
  1.0/0.5/0.0). Both copy `manifest_dir` into a disposable
  `tempfile::TempDir` before writing the candidate's content and running
  any command — `manifest_dir` (the real repository) is never touched,
  confirmed by `evaluate_never_mutates_the_original_manifest_dir` /
  `validate_never_mutates_the_original_manifest_dir` running a build that
  deliberately fails to compile and checking the original file
  afterward. **This runs untrusted candidate code with the same privileges
  as the host process** — no sandboxing. See the "Sandboxing" section
  below.
- **`approval.rs`** — `ApprovalWorkflow`: a candidate is approved once
  `quorum` distinct reviewers vote `Approve`; a single `Reject` vetoes it
  immediately regardless of how many approvals exist. Deliberately
  separate from `Verifier`: approval is a human policy decision, not a
  structural invariant of the record itself.
- **`verifier.rs`** — `Verifier::push` enforces, on every new
  `IterationRecord`: the hash chain never breaks; a `RolledBack` record
  always carries a `rollback_id`; the same candidate artifact is never
  recorded twice; the final score never drops by more than
  `max_score_drop` versus the previous iteration (a rollback record is
  exempt from this — it exists specifically to remediate a score drop, so
  the check that would block it would be self-defeating). On any error,
  `push` leaves the verifier's state completely unchanged.
- **`registry.rs`** — `Registry::append` is the single write path: it runs
  the candidate record through `Verifier` first, and only persists via
  `RegistryBackend` (`InMemoryRegistryBackend` or `SledRegistryBackend`,
  `sled_backend.rs`, disk-backed) if that succeeds. `Registry::restore`
  replays a backend's full history back through a fresh `Verifier`,
  so a backend that already has bad data on disk is caught, not trusted.
- **`checkpoint_store.rs`** — tracks every `Validated` record with an
  applied artifact as a rollback target, in the order they occurred.
- **`rollback.rs`** — `RollbackManager::rollback` is not a special
  mechanism: it appends a new `IterationRecord` (status `RolledBack`,
  `rollback_id` pointing at the reverted checkpoint) through the exact
  same `Registry::append` path as any other iteration. It rolls back to
  the last stable checkpoint *excluding* the current tip — if the current
  tip is itself the problem (discovered after the fact), rolling back
  "to the last stable checkpoint" without excluding it would be a no-op.
  The rollback record's score is copied from the target checkpoint's own
  record, not reset to `0.0` — it's returning to something already
  known-good, not a new failure.

## Sandboxing

`CargoTestEvaluator` runs `cargo build`/`cargo test` on candidate code
directly on the host, with the full privileges of the process running
this crate — a malicious candidate's `build.rs` or `#[test]` body can do
anything the host process can (filesystem, network, process spawning).
Isolation here means "won't corrupt `manifest_dir`," not "can't do
anything else."

`wasm_sandbox.rs` (new) closes part of that gap: `WasmSandboxEvaluator`
compiles the candidate for `wasm32-wasip1` and runs its tests inside a
`wasmtime` WASI sandbox with no preopened directories, no network, and no
inherited environment — confirmed, not just configured, by
`candidate_code_cannot_read_the_host_filesystem`, which asserts a
candidate's own attempt to read a file in its sandboxed working directory
actually fails.

**Real, reproduced limitation, disclosed rather than hidden:** wasmtime
also supports metering execution in "fuel" specifically so a runaway
candidate (`loop {}` in a `#[test]`, which hangs `CargoTestEvaluator`
forever) can be interrupted. This evaluator configures a fuel budget and
reports consumption, but **actually letting that budget run out crashes
the entire host process** on this development machine (Windows, wasmtime
28.0.1) with `STATUS_STACK_BUFFER_OVERRUN` — inside wasmtime's own trap-
unwind helper, before any Rust error handling in this crate ever runs.
Confirmed by direct reproduction (not hypothesized), and confirmed to
persist with a dedicated 64MiB-stack thread and in `--release` — ruling
out both "not enough stack" and "debug-build-only" as the cause. See
`wasm_sandbox.rs`'s module doc comment for the full account. Consequence:
**do not rely on fuel exhaustion for hard interruption on Windows with
this wasmtime version** — there is deliberately no test here that
exercises actual exhaustion, since doing so reliably aborts the whole test
process. A real fix would need out-of-process isolation (running the
sandboxed `wasmtime` execution in a genuinely separate child process, so
a crash there is an observable abnormal exit code, not a shared-process
abort) — not attempted in this pass.

## What's explicitly not here

- **No `Selector`.** Ranking candidates by criteria beyond "compiles /
  tests pass" doesn't exist — `Evaluator` deliberately only answers
  unambiguous yes/no questions.
- **No automatic triggering of `RollbackManager::rollback`.** This crate
  provides the mechanism; deciding *when* to call it (e.g., in response to
  a `ScoreDropExceeded` from a live loop, not just from `Verifier::push`
  rejecting a bad append) belongs to a caller.
