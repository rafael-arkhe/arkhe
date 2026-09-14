# arkhe-identity

Post-quantum identity derivation for ARKHE — **BIP39 → ML-DSA-65 via
HKDF-SHA3-256**, with a self-sovereign `did:arkhe` DID format.

## How it works

1. A BIP39 mnemonic expands to a 64-byte seed (standard BIP39).
2. `HKDF-SHA3-256(seed, info = "arkhe-v1-<agent_id>-mldsa65")` derives a 32-byte
   ML-DSA-65 seed.
3. `libcrux-ml-dsa` (FIPS 204) turns that seed into an ML-DSA-65 keypair.
4. The DID fingerprint binds the **public key** (not just the seed): `SHA3-256(
   "arkhe-did-v1" + agent_id + pk)[0..16]`.

Consequence: two agents sharing one seed derive different keys and therefore
different DIDs — a DID uniquely identifies one keypair + one agent role.

## Public API

- `derive_master_identity(seed: &[u8; 64], agent_id: &str) -> MasterIdentity`
- `ArkheMnemonic::new()` / from phrase — mnemonic generation & parse
- `build_did`, `generate_did_document`, `resolve_did` (`std` feature)
- Feature `flock`: qdrant-backed DID resolver bridge; `qdrant-resolver`:
  gRPC resolver.
- `no_std` build available (`#![no_std]`, alloc only).

## Guarantees

- `#![deny(unsafe_code)]`.
- Secret key wiped via `zeroize` immediately after derivation.
- KAT (known-answer) tests and `miri` zeroization tests in `tests/`.

## Example

```rust
use arkhe_identity::derive_master_identity;

let seed = [7u8; 64];
let id = derive_master_identity(&seed, "agent-1").expect("derive");
assert!(id.fingerprint.len() == 32); // 16 bytes, hex-encoded
```

## License

MIT OR Apache-2.0