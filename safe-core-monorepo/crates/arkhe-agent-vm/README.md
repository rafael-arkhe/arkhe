# arkhe-agent-vm

Arkhe Agent VM (AAVM): lifecycle-managed, policy-constrained agent
identities, integrating three already-verified crates rather than
reinventing any of them:

- **`arkhe-identity`** — each AAVM gets a [`Gdid`] derived from a truncated
  BLAKE3 hash of its ML-DSA-65 public key (tagged `Gdid::VERSION_ML_DSA_65`).
- **`arkhe-crypto-pqc`** — each AAVM's identity is a hybrid Ed25519 +
  ML-DSA-65 keypair (see [ADR-0001](../../docs/decisions/ADR-0001-two-pqc-crates.md)
  for why this crate specifically, not `arkhe-pqc-core`: real secret-key
  zeroization matters here, since `create_vm` briefly holds live secret
  material).
- **`arkhe-web3-security`** — creation and destruction checks are recorded
  to its `EvidenceBus` under invariant IDs `FI-A01`–`FI-A03`.

## Status

17/17 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## What each module actually does

- **`lifecycle.rs`** — a real state machine (`Creating -> Running ->
  Terminating -> Destroyed`) that *rejects* invalid transitions. An earlier
  sketch of this had unconditional setters with no validation at all —
  calling `destroy()` twice would silently "succeed." This version returns
  `Err(InvalidTransition)` instead; see the module's tests for the specific
  invalid transitions it catches (skipping `Running`, destroying twice,
  starting after destroy).
- **`policy.rs`** — `AgentPolicy` replaces an earlier undefined "Daoloop"
  placeholder with concrete fields (`max_lifetime_secs`,
  `allowed_capabilities`) and a real `is_expired()` check.
- **`manager.rs`** — `AAVMManager::create_vm`/`destroy_vm`/`sweep_expired`.
  `sweep_expired` exists because the compiler caught a real bug during
  development: `AgentPolicy` was stored on each VM's handle but never
  actually read anywhere (`warning: field 'policy' is never read`) — the
  expiry check it enables was built but not wired in. `sweep_expired`
  closes that gap: it terminates any AAVM whose own policy says it's
  outlived `max_lifetime_secs`, regardless of what else asked it to stop.

## Secret custody

`AAVMManager` does **not** retain a VM's signing key for its lifetime. It's
generated, used once (to self-sign a fixed challenge and verify it — FI-A01,
proof the VM holds the key matching its own GDID), and dropped — zeroized
via `arkhe-crypto-pqc`'s native `ZeroizeOnDrop` — before `create_vm`
returns. Only the public `HybridVerifyingKey` is kept. If a use case needs
the manager to sign on a VM's behalf later in its life, that's a distinct
design decision this crate doesn't make.
