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
  are recorded to its `EvidenceBus` under invariant IDs `FI-A01`–`FI-A08`.
- **`arkhe-agi`** — `AAVMManager::spawn_agent_session` wraps a real
  `AgiCoordinator` (safety check → inference call → memory write → history
  update) so every `process()` call is gated by the owning AAVM's *live*
  `AgentPolicy` and `LifecycleState`. See "Execution environment" below.

## Status

63/63 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## Capability certificates (FI-A08)

Each AAVM also gets a `arkhe_identity::GdidCertificate` — a **separate**
capability system from `AgentPolicy`, using `arkhe-identity`'s fixed,
4-flag `CapabilityBitmap` (`CONSENSUS`/`INFERENCE`/`GOVERNANCE_VOTE`/`HUBBLE_RELAY`)
rather than `AgentPolicy`'s free-form action strings. Issued and self-signed
at `create_vm` time using the VM's own Ed25519 sub-key, self-verified
immediately (FI-A08), and retrievable via `AAVMManager::capability_certificate(id)`.
`capability_bitmap_from_policy` maps the subset of `AgentPolicy.allowed_capabilities`
strings that have a corresponding GDID-level flag (e.g. `"llm_inference"` →
`INFERENCE`) — anything else in the policy is still enforced at the
`PolicyVerifier`/FI-A04 level, it just has no bit to set here. Two
different vocabularies for two different scopes, not a bug.

Adding this required a real change to `arkhe-identity` itself:
`GdidCertificate::issue` existed only as a private, test-only helper before
this — there was no way to construct a certificate that passed `verify()`
from outside the crate (`payload()`/`GdidCertPayload` are private). Promoted
to a real `pub fn`.

## Execution environment: policy-gated agent sessions

`spawn_agent_session` is what makes an AAVM an actual execution
environment, not just an identity/lifecycle record:

This is the exact pattern used in
`manager.rs::tests::spawned_agent_session_processes_when_llm_inference_is_allowed`
(run for real, not just sketched — see Status above):

```rust
use std::sync::Arc;
use arkhe_agent_vm::{AAVMManager, AgentPolicy};
use arkhe_core::InMemoryAgentMemory;
use arkhe_inference::{ModelId, NullEngine}; // or a real backend: MistralRsEngine, etc.
use arkhe_web3_security::agents::EvidenceBus;

let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));

// AgiCoordinator's SafetyVerifier check always asks about the fixed action
// "llm_inference" (see arkhe-agi/src/coordinator.rs) — the policy must
// allow exactly that string, or every process() call will be rejected.
let policy = AgentPolicy {
    max_lifetime_secs: 3600,
    allowed_capabilities: vec!["llm_inference".to_string()],
};
let vm = manager.create_vm(policy).await?; // FI-A01 + FI-A02 checked and recorded here

let coordinator = manager
    .spawn_agent_session(
        &vm.id,
        Arc::new(InMemoryAgentMemory::new()),
        Arc::new(NullEngine::new(ModelId::new("test", "null"))), // swap for a real InferenceEngine in production
        "session-1",
        "You are helpful.",
    )
    .await?;

let response = coordinator.process("hello").await?; // gated by FI-A04, see below

manager.destroy_vm(&vm.id).await?; // FI-A03 recorded; next process() call on `coordinator` now fails
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

FI-A04 is formalized in `proofs/lean/AgentVm/PolicyGate.lean` and **is
type-checked** — `lake build`, 5/5 jobs, exit 0 (`../docs/verification/lake-build-agent-vm-2026-07-11.txt`).
The first version failed to build (`simp` didn't fully evaluate a concrete
`BEq` comparison); see that directory's `README.md` for the fix.

## Snapshots (FI-A05)

`AAVMManager::snapshot(id)` captures a hash-verifiable record of a VM's
current public state (`gdid`, `LifecycleState`, `AgentPolicy`, timestamp),
records the integrity verdict to `EvidenceBus`, and returns an
`AavmSnapshot`. Scoped deliberately narrow — see `src/snapshot.rs`'s doc
comment: this is a state hash, not a resumable process checkpoint, since
nothing in this crate (or the `AgiCoordinator` it spawns) persists an
agent's conversation history anywhere the manager can reach.
`Snapshot::verify_integrity()` is the "restoration" check in this scoped
version: it confirms a snapshot hasn't been tampered with since capture,
not that it can revive a live agent. Formalized (determinism +
tamper-detection-given-hash-injectivity) in
`proofs/lean/AgentVm/SnapshotIntegrity.lean`, type-checked.

## Fail-closed faulting (FI-023) and execution deadlines (FI-055)

`Lifecycle` gained a fifth state, `SafeClosed`, and an optional `deadline:
Option<u64>` — both **additive**: no existing variant, transition, or field
was removed or renamed, so every pre-existing test and both Lean proofs
(`PolicyGate.lean`, `SnapshotIntegrity.lean`) kept passing unmodified.

- **`Lifecycle::fault()`** (FI-023) forces a transition to `SafeClosed` from
  *any* non-terminal state (`Creating`, `Running`, or `Terminating`),
  deliberately skipping the normal `Terminating` step — a fault is an
  emergency stop, not a graceful shutdown. `SafeClosed` can only go on to
  `Destroyed`; it can never return to `Running`. `AAVMManager::fault_vm(id)`
  wraps this and records **FI-A09**; a faulted VM's `PolicyVerifier` rejects
  every subsequent `process()` call, same live-enforcement guarantee as
  `destroy_vm` (`faulted_vm_rejects_further_agent_actions`).
- **`Lifecycle::set_deadline`/`is_past_deadline`** (FI-055) add an optional
  absolute-timestamp deadline. `AAVMManager::set_deadline(id, Some(t))` sets
  it; `AAVMManager::sweep_timed_out()` finds every `Running` VM whose
  deadline is in the past, calls `fault_vm` on each (recording FI-A09), and
  additionally records a summary **FI-A10** verdict. A VM with no deadline
  set never times out (`no_deadline_set_never_times_out`).

This is deliberately **not** the redesign a since-superseded planning
document proposed: that version replaced the state machine outright and
added `AAVMManager::tick(&mut self)`, which conflicts with the real
`AAVMManager`'s `&self`-only, `Arc<RwLock<..>>`-based interior-mutability
design (every other method — `create_vm`, `destroy_vm`, `sweep_expired` —
takes `&self`). `sweep_timed_out(&self)` matches that existing pattern
instead of introducing a `&mut self` outlier.

## Plan kinds: execution vs. service (FI-032)

`plan.rs` distinguishes two kinds of `arkhe_reasoning::Plan` (the
`PlanKind` enum itself lives in `arkhe-reasoning`, which stays
dependency-free — see that crate's docs):

- **`PlanKind::Execution`** — must terminate. `evaluate_plan` first
  validates the plan is acyclic (FI-031), then checks the owning VM's
  `Lifecycle::is_past_deadline()` (FI-055): past deadline is
  `PlanOutcome::Failed(PlanFailure::DeadlineExceeded)`; otherwise
  `PlanOutcome::Done`. A `Lifecycle` with no deadline set never times out
  (matching `Lifecycle::is_past_deadline`'s own semantics), so an
  execution plan on such a VM is `Done` as soon as it validates — this is
  exactly `execution_plan_past_its_deadline_is_failed` /
  `execution_plan_with_no_deadline_is_done` in `plan.rs`'s tests.
- **`PlanKind::Service`** — persistent, no deadline enforcement.
  `evaluate_plan` returns `PlanOutcome::Running` once validated;
  `evaluate_service_health` is a thin wrapper over
  `arkhe_health_check::HealthChecker::readiness` for ongoing monitoring —
  no new health-check machinery invented here.

Building this required fixing `arkhe-health-check` itself, which had never
been a workspace member before (added this pass): it referenced
`serde_json::Value` without declaring `serde_json` as a dependency
(`E0433`), and had a dead `checks.iter().map(|c| c.status).max()` call that
doesn't compile (`HealthStatus` has no `Ord`) — its result was never even
read; the function already recomputes `status` via an explicit `any()`
chain below it. Fixed by adding the missing dependency and deleting the
dead line, not by inventing an `Ord` semantics nothing actually needed.

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
  **FI-A07** records evidence for every individual `Lifecycle` transition
  (not just the higher-level FI-A01–FI-A03 summaries) — `create_vm`'s
  `Creating -> Running` step previously produced no evidence at all;
  `destroy_vm` now records two FI-A07 entries (`Running -> Terminating`,
  `Terminating -> Destroyed`) alongside its existing combined FI-A03
  verdict, rather than replacing it.

## Logging

`create_vm`/`destroy_vm`/`snapshot` and `PolicyVerifier::verify` emit
`tracing` events (`info` for normal lifecycle events, `warn` for rejections
and failures) — structured execution logs at AAVM task boundaries, wired
into the same `tracing` facade the rest of the workspace already uses (no
new logging framework introduced).

## Secret custody

`AAVMManager` does **not** retain a VM's signing key for its lifetime. It's
generated, used once (to self-sign a fixed challenge and verify it — FI-A01,
proof the VM holds the key matching its own GDID), and dropped — zeroized
via `arkhe-crypto-pqc`'s native `ZeroizeOnDrop` — before `create_vm`
returns. Only the public `HybridVerifyingKey` is kept. If a use case needs
the manager to sign on a VM's behalf later in its life, that's a distinct
design decision this crate doesn't make.
