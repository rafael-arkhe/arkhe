# AgentVm — Lean 4 formalization

Formal counterparts to FI-A04 (`session::PolicyVerifier::verify`) and
FI-A05 (`snapshot::AavmSnapshot`) in `../src/`. Second Lake project in this
repo, mirroring the structure of `../../arkhe-web3-security/proofs/lean/`
(the first one).

## What's formalized

- `AgentVm/PolicyGate.lean` (FI-A04): "an agent action is only ever allowed
  if `AgentPolicy::allows(action)` holds **and** the VM's `Lifecycle` is
  `Running`" — proved as four theorems covering all four `LifecycleState`
  values, plus one mirroring the Rust test
  `restrictive_policy_grants_no_capabilities`.
- `AgentVm/SnapshotIntegrity.lean` (FI-A05): determinism of snapshot capture,
  and tamper detection — **given** the hash function is injective (an
  explicit hypothesis, not a claim proven about BLAKE3 itself; see that
  file's doc comment for why this scope boundary is deliberate).

## What's not formalized

FI-A01 (identity self-attestation) and FI-A02 (policy well-formedness) —
both are Rust-level checks in `manager.rs::create_vm` with their own unit
tests, not formalized here. FI-A03 (graceful termination) is exactly the
`Lifecycle` transition graph already covered by
`arkhe-web3-security/proofs/lean/Web3Invariants/Reentrancy.lean`'s general
approach, but hasn't itself been separately formalized for this crate's
specific `Creating -> Running -> Terminating -> Destroyed` graph. FI-A06
does not exist — no definition of "RVM"/coherence domains was available to
formalize against.

## Verification status

**Type-checked for real — after fixing a genuine bug.** The first version
of `PolicyGate.lean` used `simp [policyGate]` for all four `LifecycleState`
theorems, copying the pattern from `Web3Invariants/Reentrancy.lean`. Once a
real Lean toolchain was available, `lake build` failed with `unsolved
goals` on all four — `simp` alone didn't fully evaluate the derived `BEq`
comparison on a concrete `LifecycleState` constructor. Fixed by replacing
`by simp [policyGate]` with plain `rfl` on all four: since the state
argument is always a concrete constructor at the call site, `state ==
LifecycleState.running` and the subsequent `Bool.and` both reduce by
computation alone, no tactic search needed. Rebuilt clean: exit code 0,
"Build completed successfully (5 jobs)" (4 for `PolicyGate.lean` alone at
the time of that fix; a 5th job, `SnapshotIntegrity.lean`, was added and
verified afterward — see its own theorem for the one genuine build failure
it hit and how it was fixed). Log:
`../../../docs/verification/lake-build-agent-vm-2026-07-11.txt` (run from
`safe-core-monorepo/`). This is exactly the outcome the original "not
verified" disclosure was for — the proof had a real bug, now fixed and
confirmed, not silently assumed correct either before or after. To
reproduce:

```sh
curl https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh -sSf | sh
cd proofs/lean
lake build
```
