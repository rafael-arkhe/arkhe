# arkhe-reasoning

FI-031 — acyclic execution plans: a plan whose action dependency graph
contains a cycle is rejected before execution, detected via Kahn's algorithm
(topological sort). FI-032 — `PlanKind` distinguishes bounded plans from
persistent ones. FI-037 — causal graphs: acyclic *and* temporally
consistent claims of the form "node N was caused by node C"; see below.

## Status

17/17 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## What's actually here

- **`validator.rs`** — `Action { id, name, depends_on }`, `Plan { actions }`,
  `PlanValidator::validate(&self, plan: &Plan) -> Result<Vec<ActionId>, PlanError>`.
  A plan is acyclic iff Kahn's algorithm can process every action (i.e. the
  returned topological order has the same length as the input). `PlanError`
  distinguishes `Cyclic(Vec<ActionId>)` (the leftover, unprocessable action
  IDs) from `UnknownDependency { action, missing_dependency }` (a dependency
  that doesn't name any action actually in the plan). Pure, synchronous,
  side-effect-free — no `EvidenceBus`/async runtime dependency, so a caller
  wires evidence recording and actual execution around it.
- **`plan_kind.rs`** (FI-032) — `PlanKind::{Execution, Service}`, a plain
  `Copy` enum with no reference to `arkhe-agent-vm::Lifecycle` or
  `arkhe-health-check`, keeping this crate dependency-free. The actual
  outcome evaluation against a live `Lifecycle` (deadline-based for
  `Execution`, health-check-based for `Service`) lives in
  `arkhe-agent-vm::plan`, which depends on this crate — not the other way
  around.

- **`causal_graph.rs`** (FI-037, new — no prior definition of this
  invariant existed anywhere in this codebase; this is a proposed, disclosed
  interpretation, not a recovered spec) — `CausalNode { id, timestamp_secs,
  caused_by }`, `CausalGraph { nodes }`,
  `CausalGraphValidator::validate(&self, graph: &CausalGraph) ->
  Result<Vec<NodeId>, CausalError>`. Reuses `validator.rs`'s Kahn's-algorithm
  acyclicity check (a causal graph must be acyclic — nothing can be its own
  cause, even transitively) but adds a check FI-031's plans have no basis
  for: every node carries a real timestamp, so `CausalGraphValidator`
  additionally rejects any claimed cause timestamped *after* the effect it
  supposedly produced (`CausalError::EffectPrecedesCause`) — a check that
  only makes sense because causal nodes describe things that already
  happened, unlike a plan's not-yet-executed actions. Deliberately generic
  over `NodeId`/timestamps rather than coupled to any specific record type
  (`arkhe-rsi-core::IterationRecord`, `arkhe-evidence::EvidenceRecord`,
  ...) — a caller maps its own already-hash-chained records into
  `CausalNode`s.

## A real bug fixed relative to an earlier draft

An earlier sketch of this validator used
`plan.dependencies.get(&id).unwrap_or(&vec![])` to look up an action's
dependents. This does not compile: `&vec![]` borrows a temporary `Vec` that
is dropped at the end of the statement (`E0716`, "temporary value dropped
while borrowed"). The real implementation instead builds a
`dependents: HashMap<ActionId, Vec<ActionId>>` once, up front, and borrows
against that owned storage via `.get()` — no temporary involved. Confirmed
via `cargo test -p arkhe-reasoning` actually compiling and the 7 unit tests
passing (linear chains, direct + longer cycles, diamond graphs, empty plans,
unknown-dependency rejection, self-dependency-as-cycle).

## What's explicitly not here

- **No plan execution.** This validates and orders a plan; it does not run
  anything.
- **No `EvidenceBus` integration.** A caller that wants FI-031 evidence
  records wraps `PlanValidator::validate` and records the `Ok`/`Err` result
  itself (matching the `AuditEvidence { invariant_id, verdict }` pattern used
  elsewhere in this workspace) — not built into this crate, to keep it
  testable without pulling in `arkhe-web3-security`/`tokio`.
