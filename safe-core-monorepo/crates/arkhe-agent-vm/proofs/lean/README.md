# AgentVm — Lean 4 formalization

Formal counterpart to FI-A04 (`session::PolicyVerifier::verify` in `../src/session.rs`).
Second Lake project in this repo, mirroring the structure of
`../../arkhe-web3-security/proofs/lean/` (the first one).

## What's formalized

`AgentVm/PolicyGate.lean`: the statement "an agent action is only ever
allowed if `AgentPolicy::allows(action)` holds **and** the VM's `Lifecycle`
is `Running`" — proved as four theorems covering all four `LifecycleState`
values (one showing the gate reduces to the policy check when `Running`,
three showing it's unconditionally `false` for every other state), plus one
mirroring the Rust test `restrictive_policy_grants_no_capabilities`.

## What's not formalized

FI-A01 (identity self-attestation) and FI-A02 (policy well-formedness) —
both are Rust-level checks in `manager.rs::create_vm` with their own unit
tests, not formalized here. FI-A03 (graceful termination) is exactly the
`Lifecycle` transition graph already covered by
`arkhe-web3-security/proofs/lean/Web3Invariants/Reentrancy.lean`'s general
approach, but hasn't itself been separately formalized for this crate's
specific `Creating -> Running -> Terminating -> Destroyed` graph.

## Verification status — not type-checked

Same caveat as the web3-security proofs: no `lean`/`lake` toolchain in the
session that wrote this. Written against Lean 4 core syntax only, reusing
the exact tactics (`simp` on `Bool` equations with concrete constructors
substituted) already used in `Web3Invariants/Reentrancy.lean`, which is the
lowest-risk pattern established so far in this repo — but "written
carefully" is not "verified." To check:

```sh
cd proofs/lean
lake build
```
