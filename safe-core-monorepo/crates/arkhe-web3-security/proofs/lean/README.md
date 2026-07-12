# Web3Invariants — Lean 4 formalization

Formal counterparts to a subset of the `FI-W##`/`SC##` invariants already
implemented and unit-tested in Rust under `../src/`. This is the **first**
Lake project in this repository — there was no existing `lakefile.lean` /
`lakefile.toml` anywhere to inherit conventions from, and none of the other
`.lean` files scattered around the repo (`arkhe-agi-ecosystem/lean/`,
`substrates/substrate_286_bis/lean/`, etc.) are wired into a build either.

## What's formalized here

| File | Invariant | Mirrors (Rust) |
|---|---|---|
| `Web3Invariants/Reentrancy.lean` | SC08 / FI-W03 / FI-W04 | `contracts::reentrancy` |
| `Web3Invariants/NonceMonotonic.lean` | FI-W07 | `wallets::signature::NonceTracker` |
| `Web3Invariants/DomainSeparation.lean` | FI-W06 | `wallets::eip712` |

## What's explicitly **not** formalized

`SC01`, `SC02`, `SC03`, `SC04`, `SC05`, `SC06`, `SC07`, `SC09`, `SC10`, and
every other `FI-W##` invariant beyond the three above (there are 20 total,
implemented in Rust across `analyzers/`, `blockchain/`, `dapps/`,
`contracts/`, `wallets/`) have **no** Lean proof yet. Stated here explicitly
instead of silently omitted — the audit that reviewed the previous version
of this work flagged exactly that omission-without-disclosure pattern, not
incompleteness itself.

## Verification status — read before trusting this

**None of these `.lean` files have been type-checked.** The session that
wrote them has `rustc`/`cargo` installed but no `lean`/`lake` toolchain, and
had no way to run `lake build`. The code was written carefully against
Lean 4 core syntax only (no Mathlib import — kept deliberately minimal to
reduce the chance of a version mismatch), reusing tactics (`simp`, `decide`,
`omega`) that are standard and stable across recent Lean 4 releases, but
**"written carefully" is not the same as "verified."** To actually check it:

```sh
# lean-toolchain pins leanprover/lean4:v4.31.0 (verified as the current
# stable release at https://github.com/leanprover/lean4/releases as of
# 2026-07-11 — this specific version string has not been independently
# re-confirmed since)
cd proofs/lean
lake build
```

If `lake build` fails, that's a real bug in this proof, not a false claim —
please fix the specific tactic/lemma that doesn't resolve rather than
discarding the model. The executable definitions (`GuardState`, `Op`,
`ceiHoldsAux`, `consumeNonce`, `Domain`/`StructuredSig`,
`verifyDomainAndNonce`) are the parts most likely to be exactly right (plain
function/type definitions); the proof tactics are the parts most likely to
need adjustment for the exact Lean 4 release in use.

## What was fixed relative to the audited prior version

The previous `SC08` proof was a tautology: it defined the reentrancy guard
as "active" when `lock = false` (backwards from the real Rust
implementation, where `locked: bool` — `true` means held) and proved a
property via `by simp` that held regardless of what the guard actually did.
`Web3Invariants/Reentrancy.lean` fixes the semantics (`lock = true` means
held, matching `ReentrancyGuard.locked` exactly) and proves both directions
(`guard_blocks_reentrant_enter` / `guard_allows_enter_when_free`) so the
model can't vacuously satisfy either theorem.
