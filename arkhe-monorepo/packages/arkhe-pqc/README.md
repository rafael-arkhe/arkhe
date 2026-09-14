# arkhe-pqc

Quantum-safe cryptography layer for the ARKHE runtime — **FIPS 203 ML-KEM** +
**FIPS 204 ML-DSA** in pure Rust, built on the RustCrypto implementations
(`ml-kem 0.3.2`, `ml-dsa 0.1.1`).

> **Naming honesty:** this crate exists because the audit docs refer to
> `dcrypt` / `pqc-std` / `libQ`, which do not exist on crates.io. The canonical
> FIPS 203/204 implementations are RustCrypto's `ml-kem` and `ml-dsa`, and this
> crate is the ARKHE layer around them.

## Modules

| Module | What it provides |
|---|---|
| `kem` | `generate_kem_keypair`, `quantum_encapsulate`, `quantum_decapsulate` |
| `sign` | `quantum_sign`, `quantum_verify`, `QuantumSignature`, `QuantumSigner` |
| `auth_kem` | authenticated KEM: `auth_encapsulate` / `auth_decapsulate`, `PqcIdentity`, `PqcKeyMaterial` |
| `handshake` | three-step PQC handshake: `initiator_hello`, `responder_ack`, `confirm_ss`, `verify_confirm` |
| `wallet` | `PqcWallet`, `WalletEntry`, `KeyStatus`, `identity_fingerprint` |

## Guarantees

- `#![deny(unsafe_code)]` — zero unsafe in this crate.
- Dependencies only: `ml-kem`, `ml-dsa`, `sha2`.
- Post-quantum defaults for ARKHE identity, handshake and wallet flows.

## Example

```rust
use arkhe_pqc::{generate_kem_keypair, quantum_encapsulate, quantum_decapsulate};

let (sk, pk) = generate_kem_keypair();
let (ct, ss) = quantum_encapsulate(&pk);
assert_eq!(quantum_decapsulate(&ct, &sk).unwrap(), ss);
```

## License

MIT OR Apache-2.0