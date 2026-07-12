# AAVM integration: identity, lifecycle, policy, and agent execution

**Scope note:** this describes what `arkhe-agent-vm` actually does today —
not a target architecture. Where a prior document (a "Monorepo Completo"
spec) described AAVM concepts that don't exist yet (`arkhe-cuda-guard`,
`SectorLoop`/`SectorPolicy`, hardware-level `seccomp`/`landlock` execution,
self-modifying agent policies), they're intentionally absent here — see
"Explicitly out of scope" at the bottom for why each one isn't in this
diagram, and [ADR-0001](../decisions/ADR-0001-two-pqc-crates.md) for the
one architectural decision this crate has actually made so far.

## Component map

```mermaid
flowchart TB
    subgraph identity["Identity (existing, pre-session)"]
        GDID["arkhe-identity::Gdid"]
    end
    subgraph pqc["PQC (existing, this session)"]
        PQC["arkhe-crypto-pqc::HybridKeyPair<br/>(fips204/fips203, native ZeroizeOnDrop)"]
    end
    subgraph evidence["Evidence (existing, this session)"]
        EB["arkhe-web3-security::EvidenceBus<br/>(AuditEvidence: InvariantId + InvariantVerdict)"]
    end
    subgraph agi["Agent loop (existing, pre-session)"]
        AGI["arkhe-agi::AgiCoordinator<br/>safety-check -> inference -> memory-write -> history-update"]
    end

    subgraph aavm["arkhe-agent-vm (this session)"]
        MGR["AAVMManager"]
        LC["Lifecycle<br/>Creating -> Running -> Terminating -> Destroyed"]
        POL["AgentPolicy<br/>max_lifetime_secs, allowed_capabilities"]
        PV["PolicyVerifier<br/>implements arkhe_core::SafetyVerifier"]
    end

    MGR -->|generates| PQC
    MGR -->|derives Gdid from pubkey hash| GDID
    MGR -->|owns, Arc-shared| LC
    MGR -->|owns, Arc-shared| POL
    MGR -->|records FI-A01..FI-A03| EB
    MGR -->|spawn_agent_session builds| PV
    PV -->|reads live| LC
    PV -->|reads| POL
    PV -->|records FI-A04| EB
    MGR -->|constructs, PV as SafetyVerifier| AGI
```

## Flow: `create_vm` → `spawn_agent_session` → `process` → `destroy_vm`

```mermaid
sequenceDiagram
    participant Caller
    participant Manager as AAVMManager
    participant PQC as arkhe-crypto-pqc
    participant GDID as arkhe-identity
    participant Bus as EvidenceBus
    participant PV as PolicyVerifier
    participant Coord as AgiCoordinator

    Caller->>Manager: create_vm(policy)
    Manager->>PQC: generate_hybrid_keypair()
    Manager->>GDID: Gdid::from_parts(hash(pubkey))
    Manager->>PQC: sign(CHALLENGE), verify(...)
    Manager->>Bus: store(FI-A01, verdict)
    Manager->>Bus: store(FI-A02, verdict)
    Note over PQC: secret key dropped here<br/>(ZeroizeOnDrop)
    Manager-->>Caller: AavmSummary { id, state: Running, ... }

    Caller->>Manager: spawn_agent_session(id, memory, inference, ...)
    Manager->>PV: new(policy, lifecycle, evidence_bus)
    Manager-->>Caller: AgiCoordinator<PolicyVerifier, M, I>

    Caller->>Coord: process("hello")
    Coord->>PV: verify("llm_inference", input)
    PV->>PV: check lifecycle == Running
    PV->>PV: check policy.allows("llm_inference")
    PV->>Bus: store(FI-A04, verdict)
    alt allowed
        PV-->>Coord: SafetyVerdict::Allowed
        Coord->>Coord: run inference, write memory, update history
        Coord-->>Caller: response text
    else rejected
        PV-->>Coord: SafetyVerdict::Rejected(reason)
        Coord-->>Caller: Err(PermissionDenied)
    end

    Caller->>Manager: destroy_vm(id)
    Manager->>Manager: lifecycle.begin_terminate(), .destroy()
    Manager->>Bus: store(FI-A03, verdict)
    Note over PV: PV's Arc<RwLock<Lifecycle>> is the<br/>SAME object Manager just mutated

    Caller->>Coord: process("still holds the old handle")
    Coord->>PV: verify("llm_inference", input)
    PV->>PV: lifecycle state is now Destroyed
    PV-->>Coord: SafetyVerdict::Rejected("not Running")
    Coord-->>Caller: Err(PermissionDenied)
```

The last two rows are the part that's actually tested end-to-end (not just
diagrammed): `manager.rs::tests::destroying_the_vm_blocks_further_process_calls_on_an_already_spawned_session`
builds a real `AgiCoordinator` + `NullEngine`, calls `process()` once
successfully, destroys the VM, and asserts the *same* coordinator handle's
next `process()` call fails — proving the shared `Arc<RwLock<Lifecycle>>`
actually propagates the state change rather than each side holding its own
snapshot.

## Invariants recorded to `EvidenceBus`

| ID | Checked in | What it means | Formalized |
|---|---|---|---|
| FI-A01 | `create_vm` | The VM's secret key can sign a message its own public key verifies (self-attestation) | Not yet — Rust-only |
| FI-A02 | `create_vm` | `policy.max_lifetime_secs > 0` | Not yet — Rust-only |
| FI-A03 | `destroy_vm` | Lifecycle transitioned `Running -> Terminating -> Destroyed` without error | Overlaps with the general transition-graph approach in `arkhe-web3-security/proofs/lean/Web3Invariants/Reentrancy.lean`, not separately proven for this crate's specific graph |
| FI-A04 | `PolicyVerifier::verify` (every `process()` call) | Action allowed only if `policy.allows(action)` **and** `lifecycle == Running` | Yes — `crates/arkhe-agent-vm/proofs/lean/AgentVm/PolicyGate.lean`, **type-checked** (`lake build`, 5/5 jobs, see `docs/verification/lake-build-agent-vm-2026-07-11.txt`) |
| FI-A05 | `AAVMManager::snapshot` | A captured snapshot passes its own integrity check; a tampered one fails it (given hash injectivity) | Yes — `crates/arkhe-agent-vm/proofs/lean/AgentVm/SnapshotIntegrity.lean`, **type-checked**. Does not prove anything about BLAKE3 itself — see that file's doc comment |

FI-A06 is not listed: no definition of "RVM"/coherence domains was
available in this codebase or session context to formalize against.

## Explicitly out of scope (and why)

- **OS-level sandboxing / hardware execution / `seccomp`/`landlock`** — no
  process isolation, resource limiting, or syscall filtering exists
  anywhere in this workspace yet (`arkhe-tool-sandbox` has no source at
  all; `arkhe-syscall-bridge` is a mapping table with no enforcement).
  `PolicyVerifier` is in-process, type-system-level gating — it stops a
  disallowed `process()` call from starting, not what an allowed call can
  do once inference runs. Building real sandboxing is its own large,
  security-critical project, not an incremental addition to this one.
- **`SectorLoop` / `SectorPolicy`** — searched the whole
  `safe-core-monorepo` workspace for `SectorLoop`, `SectorPolicy`,
  `arkhe-sector`: zero matches. Nothing to integrate with yet.
- **Coherence domains / "RVM" / proof-gated re-partitioning** — no
  definition of "RVM" is available in this codebase or this session's
  context. Not attempted rather than guessed at.
- **Auto-evolution (agents mutating their own policy)** — a real, distinct
  design question (how much can an agent loosen its own constraints, and
  under what check) that deserves its own scoping conversation, not a
  same-pass addition alongside everything else here.
