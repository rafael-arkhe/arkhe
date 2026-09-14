# ARKHE Monorepo

Constitutional container runtime for scalable AI systems. This repository
documents and implements the ARKHE operating system: every substrate sealed,
every invariant checked, every action anchored in the time chain.

**Status:** CANONIZED_CLEAN — workspace builds on `rustc 1.94.0`, Lean core
4.33.1, no `sorry`, no placeholders in the pitch-critical crates.

## Pitch-critical packages (2026)

These are the compilable, verified foundations for post-quantum identity and
enforceable AI safety:

| Package | Role |
|---|---|
| [`packages/arkhe-pqc`](packages/arkhe-pqc/README.md) | FIPS 203 ML-KEM + FIPS 204 ML-DSA, pure Rust, `#![deny(unsafe_code)]` |
| [`packages/identity`](packages/identity/README.md) | BIP39 → ML-DSA-65 via HKDF-SHA3-256, `did:arkhe` |
| [`services/safe-core`](services/safe-core/README.md) | gRPC anti-hallucination watchdog (`vibe = broadcast × weak_closure`), `SetRegime`/`DeepSleep` |
| [`packages/arkhe-safe-manifold`](packages/arkhe-safe-manifold/README.md) | Safe manifold (frontier/escape-region) + device↔host consciousness governance (C-01..C-08) |
| [`packages/arkhe-observability`](packages/arkhe-observability/README.md) | Read-only observability bridge + `TpmAnchor` hardware root of trust (NCrypt/CNG) |

## Formal verification

- Lean 4 core in `src/lean/` — kernel 4.33.1, **no Mathlib, no `sorry`**:
  `I511–I523` (field-stability/coherence), `I647–I649` (safe-manifold severity
  monotonicity, mode, conservative tie-break).
- TLA+ spec `ArkheCoherenceLedger` (`packages/arkhe-field-stability/spec/`),
  validated with TLC.
- SHA3-256 append-only `CoherenceLedger` with native Gravity-1 (rejects
  non-monotonic timestamps).

## Build & verify

```bash
cargo build --workspace
cd src/lean/nuclei && lake build
```

> Toolchain note: `packages/arkhe-ia-dtn` / `packages/arkhe-core` are excluded
> from the workspace because `hardy-bpv7 0.6.0` requires rustc ≥ 1.95 (toolchain
> is 1.94). Every other package builds.

## Honesty commitments

This repository never claims artifacts that do not exist here. There is **no**
`RealTpmProvider`/tss-esapi, **no** "unfireable safety kernel"/
`barrier_is_unfireable`, and **no** IBC/Tendermint mesh. Placeholders
(`todo!()`/`PLACEHOLDER`) exist only in `catedral_deps/` / `catedral_os_v304`
adapter code, never in the packages listed above.

## License

Dual MIT / Apache-2.0 (core). See each package's `Cargo.toml`.

---
*"A Catedral é o pensamento do Arquiteto materializado."*