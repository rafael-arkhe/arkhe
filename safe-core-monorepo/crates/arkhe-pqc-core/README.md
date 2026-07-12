# arkhe-pqc-core

Hybrid Ed25519 + ML-DSA-65 signatures and ML-KEM-1024 KEM, via
`pqcrypto-dilithium` / `pqcrypto-mlkem` (PQClean C bindings — this
environment does have a working C toolchain, confirmed by building this
crate).

Deliberately separate from [`arkhe-crypto-pqc`](../arkhe-crypto-pqc) (pure-Rust
`fips204`/`fips203`), not a replacement — see
[ADR-0001](../../docs/decisions/ADR-0001-two-pqc-crates.md).

## Status

7/7 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## Known limitation: no zeroization on the ML-DSA/ML-KEM secret keys

Unlike `arkhe-crypto-pqc`, this crate's PQC secret key types —
`pqcrypto_dilithium::dilithium3::SecretKey` and
`pqcrypto_mlkem::mlkem1024::SecretKey` — **do not implement `Zeroize`**, and
their trait (`pqcrypto_traits::{sign,kem}::SecretKey`) only exposes an
*immutable* `as_bytes(&self) -> &[u8]`. There is no way to zero this memory
through the public API. Confirmed directly: deriving
`#[derive(Zeroize, ZeroizeOnDrop)]` on a wrapper containing either type
fails to compile (`the trait bound ... SecretKey: Zeroize is not
satisfied`). No `Drop` impl papers over this — see
[`src/hybrid.rs`](src/hybrid.rs) and [`src/kem.rs`](src/kem.rs)'s doc
comments for the full verification trail. If secret-key zeroization matters
for your use case, prefer `arkhe-crypto-pqc`, whose `fips204`/`fips203`
secret key types derive `ZeroizeOnDrop` natively.

## Naming pitfall this crate avoids

`pqcrypto-dilithium`'s NIST-level naming (`dilithium2`/`3`/`5`) doesn't
match FIPS 204's final naming (`ML-DSA-44`/`65`/`87`) — `dilithium3` is
ML-DSA-65 (3309-byte signatures, confirmed and cross-checked against
`arkhe-crypto-pqc`'s `fips204::ml_dsa_65::SIG_LEN`), **not** `dilithium5`
(which is ML-DSA-87, 4627 bytes). An earlier draft of this crate used
`dilithium5` while labeling it "MLDSA65" — this crate uses `dilithium3`
throughout and stores signatures as the real `DetachedSignature` type
rather than a hand-maintained size constant, so the mismatch can't recur
silently.
