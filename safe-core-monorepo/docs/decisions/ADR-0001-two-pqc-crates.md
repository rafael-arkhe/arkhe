# ADR-0001: Two independent PQC crates (`arkhe-crypto-pqc`, `arkhe-pqc-core`)

**Status:** Accepted
**Date:** 2026-07-11

## Context

`arkhe-crypto-pqc` (hybrid Ed25519 + ML-DSA-65 signing, ML-KEM-1024 KEM) was
built first, using the pure-Rust `fips204`/`fips203` crates, as part of
fixing the findings in `ARKHE-CATHEDRAL-AUDITORIA-v1.0-2026-07-11` — 10/10
tests pass, `arkhe-web3-security`'s `wallets::hybrid_wallet` depends on it.

A separate document then proposed the same functionality using
`pqcrypto-dilithium`/`pqcrypto-mlkem` (PQClean C bindings) instead. Rather
than pick one and discard the other, the explicit choice made was to build
`arkhe-pqc-core` as a second, independent crate.

## Decision

Both crates stay in the workspace, independently maintained:

| | `arkhe-crypto-pqc` | `arkhe-pqc-core` |
|---|---|---|
| Backend | `fips204`/`fips203` (pure Rust) | `pqcrypto-dilithium`/`pqcrypto-mlkem` (PQClean C bindings) |
| Toolchain needed | None beyond `rustc` | A working C compiler (confirmed present in this environment) |
| Secret-key zeroization | Native — `PrivateKey`/`DecapsKey` derive `ZeroizeOnDrop` | **None available** — `SecretKey` types don't implement `Zeroize`, and their trait only exposes an immutable `as_bytes()`; confirmed by compiling the derive in isolation |
| Wired into `arkhe-web3-security` | Yes (`wallets::hybrid_wallet`) | No |
| Tests | 10/10 | 7/7 |

## Consequences

- Anything that needs secret-key zeroization guarantees should use
  `arkhe-crypto-pqc`, not `arkhe-pqc-core` — this is a real, load-bearing
  difference, not a style preference. `arkhe-pqc-core`'s README states this
  plainly.
- The two crates will independently need to track FIPS 204/203 and any
  future NIST guidance; a change to one's public API is not expected to
  require a matching change to the other, since they're not meant to share
  a common trait/interface at this time.
- If a common interface is wanted later (e.g. to let callers swap backends),
  that's a new decision, not implied by this one.
- Cross-check performed as a side effect of maintaining two implementations:
  both independently agree that ML-DSA-65 signatures are 3309 bytes
  (`fips204::ml_dsa_65::SIG_LEN` and
  `pqcrypto_dilithium::dilithium3::signature_bytes()`) — a real, if small,
  piece of evidence that neither implementation has the parameter set wrong.
