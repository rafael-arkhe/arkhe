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
- **`arkhe-web3-security`** — creation, destruction, and per-action checks
  are recorded to its `EvidenceBus` under invariant IDs `FI-A01`–`FI-A04`.
- **`arkhe-agi`** — `AAVMManager::spawn_agent_session` wraps a real
  `AgiCoordinator` (safety check → inference call → memory write → history
  update) so every `process()` call is gated by the owning AAVM's *live*
  `AgentPolicy` and `LifecycleState`. See "Execution environment" below.

## Status

25/25 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## Execution environment: policy-gated agent sessions

`spawn_agent_session` is what makes an AAVM an actual execution
environment, not just an identity/lifecycle record:

```rust
let coordinator = manager
    .spawn_agent_session(&vm_id, memory, inference, "session-1", "You are helpful.")
    .await?;

coordinator.process("hello").await // gated by FI-A04, see below
```

Every `process()` call goes through [`session::PolicyVerifier`], which
implements `arkhe_core::SafetyVerifier` — the exact hook `AgiCoordinator`
already calls before every inference request, not a new one invented for
this. It enforces **FI-A04**: an action is allowed only if
`AgentPolicy::allows(action)` holds *and* the VM's `LifecycleState` is
`Running`. Both are read live through a shared `Arc<RwLock<Lifecycle>>` /
`Arc<AgentPolicy>` — so `destroy_vm`/`sweep_expired` called on a VM after a
session was already spawned from it takes effect on that session's *next*
`process()` call, not just on newly-spawned sessions. Proven with an actual
`AgiCoordinator`+`NullEngine` in
`manager.rs::tests::destroying_the_vm_blocks_further_process_calls_on_an_already_spawned_session`.

**What this is not:** in-process, type-system-level policy enforcement —
not OS-level sandboxing. Nothing in this workspace does process isolation,
resource limiting, or syscall filtering yet (`arkhe-tool-sandbox` and
`arkhe-syscall-bridge` are both non-functional placeholders). A
`PolicyVerifier` stops a disallowed `process()` call from *starting*; it
does not constrain what an allowed call can do once inference actually
runs.

FI-A04 is formalized in `proofs/lean/AgentVm/PolicyGate.lean` (not
type-checked in this session — see that directory's `README.md`).

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
