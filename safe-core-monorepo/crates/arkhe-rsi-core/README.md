# arkhe-rsi-core

FI-050 — foundational, storage-agnostic types for a recursive
self-improvement (RSI) loop: `Artifact`, `EvaluationResult`,
`CheckpointStatus`, and the hash-chained `IterationRecord`. No policy
(approval, rollback, verification) lives here — see `arkhe-rsi` for that.

## Status

3/3 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## What's actually here

- **`Artifact`** — `id` is BLAKE3 of `content`, so identical content always
  produces the same id (content-addressed, deduplicating, verifiable
  independent of any counter or timestamp).
- **`IterationRecord`** — an immutable record of one RSI iteration:
  `baseline_id`/`candidate_id`/`applied_id` (content ids, not full
  `Artifact`s — the record never needed the content itself, only its id,
  which is also what makes rollback possible without reconstructing the
  original artifact), `final_score`, `was_applied` (reflects only whether
  something was actually applied — **not** gated by score; that decision
  belongs to `arkhe-rsi::Verifier`, not this type), `checkpoint_status`,
  and a `previous_hash`/`hash` pair forming a BLAKE3 hash chain.
  `verify_chain` checks one link; `arkhe-rsi::Verifier` checks the whole
  history. `with_rollback` exists because `rollback_id` participates in
  the hash — setting it after `new()` requires recomputing, or the record
  would carry a hash that doesn't match its own content.
- **`EvaluationResult`** — a score plus an open `metrics` map (see
  `arkhe-rsi::CargoTestEvaluator` for a real evaluator: 1.0 if the
  candidate compiles and its tests pass, 0.5 if it compiles but tests
  fail, 0.0 if it doesn't compile).

## What's explicitly not here

- **No approval, rollback, or verification policy.** This crate only
  defines what a record *is*; `arkhe-rsi` defines what makes a *sequence*
  of records valid and what to do when one shouldn't have been applied.
- **No storage backend.** `IterationRecord` is `Serialize`/`Deserialize`
  (`serde`) but persisting it is `arkhe-rsi::RegistryBackend`'s job.
