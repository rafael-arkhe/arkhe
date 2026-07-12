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

## Verification status

**Type-checked for real.** `lean-toolchain` pins `leanprover/lean4:v4.31.0`.
Elan + that toolchain were installed in-session and `lake build` was run
from a clean `.lake` directory: exit code 0, "Build completed successfully
(6 jobs)" — every file listed above plus the root `Web3Invariants.lean`
compiles, and every `theorem` in them is accepted by the real Lean kernel,
no `sorry`. Log: `../../../docs/verification/lake-build-web3-security-2026-07-11.txt`
(run from `safe-core-monorepo/`). To reproduce:

```sh
curl https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh -sSf | sh
cd proofs/lean
lake build
```

## What was fixed relative to the audited prior version

The previous `SC08` proof was a tautology: it defined the reentrancy guard
as "active" when `lock = false` (backwards from the real Rust
implementation, where `locked: bool` — `true` means held) and proved a
property via `by simp` that held regardless of what the guard actually did.
`Web3Invariants/Reentrancy.lean` fixes the semantics (`lock = true` means
held, matching `ReentrancyGuard.locked` exactly) and proves both directions
(`guard_blocks_reentrant_enter` / `guard_allows_enter_when_free`) so the
model can't vacuously satisfy either theorem.
