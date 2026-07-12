# arkhe-web3-security

Web3 security invariants (`FI-W01`..`FI-W20`), mapped to the OWASP Smart
Contract Top 10 2026 — see `../../../web3-security-architecture.md` at the
repo root for the full architecture doc, including its "Status de
Implementação" section.

## Status

80/80 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## Module map

| Module | What it does | Verified how |
|---|---|---|
| `contracts::reentrancy` | Runtime `ReentrancyGuard` + Checks-Effects-Interactions ordering check (SC08) | `cargo test` |
| `analyzers::reentrancy` | Trace-based reentrancy-cycle detector (complements the runtime guard) | `cargo test` |
| `wallets::signature` | Nonce-monotonicity replay protection (FI-W07) | `cargo test` |
| `wallets::eip712` | Domain/nonce separation (FI-W06) — uses BLAKE3 as an explicit stand-in for keccak256, not spec-faithful on the hash primitive | `cargo test` |
| `wallets::hybrid_wallet` / `wallets::policy` | Policy-aware wallet (`HybridOnly`/`PqcOnly`/`StrictPqc`) wrapping `arkhe-crypto-pqc` | `cargo test` |
| `agents::*` | Async Recon→Hunting→Validation→GapFilling audit pipeline + evidence bus | `cargo test` |
| `web3_adapter` | `before_transaction`/`after_execution` hooks producing `Web3Evidence` (`Blake3Hash`) | `cargo test` |
| `verify::kani_harness` (feature-gated, `#[cfg(kani)]`) | Kani proofs for `ReentrancyGuard` | **Not run** — no `cargo-kani`/matching nightly toolchain here; see the file's own doc comment for the exact verification trail (real `kani` API confirmed via git dependency, blocked on Kani's pinned `nightly-2025-04-03`) |
| `proofs/lean/` | Lean 4 formalization of SC08 + 2 more invariants | **Not type-checked** — no `lean`/`lake` here; see `proofs/lean/README.md` for exactly what's formalized, what isn't, and how to verify it yourself |
| `contracts/` (Solidity) | `PQCVerifier.sol` (ECDSA + off-chain oracle attestation, no fabricated on-chain ML-DSA precompile) + `ERC20Vulnerable` reentrancy exploit test | **Not compiled/run** — no `forge`/`solc` here |

## Two coexisting PQC backends

`wallets::hybrid_wallet` depends on `arkhe-crypto-pqc` specifically. A
second, independent PQC implementation, `arkhe-pqc-core`, also exists in
this workspace but is not wired into this crate — see
`../../docs/decisions/ADR-0001-two-pqc-crates.md` for why both exist.
