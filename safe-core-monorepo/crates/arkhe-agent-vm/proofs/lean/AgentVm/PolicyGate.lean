/-!
# FI-A04 — Policy gate: an agent action is allowed only if the AAVM's
# policy allows it AND its lifecycle is Running.

Formal counterpart to `PolicyVerifier::verify` in
`../../src/session.rs`. See `../README.md` for verification status — not
type-checked in this session (no Lean/Lake toolchain installed).
-/

namespace AgentVm

/-- Mirrors `LifecycleState` in `../../src/lifecycle.rs`. -/
inductive LifecycleState where
  | creating
  | running
  | terminating
  | destroyed
  deriving DecidableEq, BEq, Repr

/-- Mirrors `AgentPolicy` in `../../src/policy.rs` — only the field this
    proof needs (`max_lifetime_secs` isn't part of the FI-A04 gate itself,
    it's checked separately at creation time as FI-A02). -/
structure AgentPolicy where
  allowedCapabilities : List String
  deriving Repr

/-- Mirrors `AgentPolicy::allows`. -/
def AgentPolicy.allows (p : AgentPolicy) (action : String) : Bool :=
  p.allowedCapabilities.contains action

/-- Mirrors the actual check inside `PolicyVerifier::verify`: **both**
    conditions must hold. -/
def policyGate (state : LifecycleState) (policy : AgentPolicy) (action : String) : Bool :=
  (state == LifecycleState.running) && policy.allows action

/-- When the state is `running`, the gate reduces exactly to the policy
    check — no hidden extra condition. -/
theorem gate_when_running (policy : AgentPolicy) (action : String) :
    policyGate LifecycleState.running policy action = policy.allows action := rfl

/-- The actual content of the "AND Lifecycle is Running" clause: for each of
    the three non-running states, the gate is `false` regardless of what
    the policy allows. A broken implementation that only checked the policy
    (ignoring lifecycle entirely, as a bare `policy.allows action` would)
    could not satisfy these three theorems together with
    `gate_when_running` above — that combination is what pins down that
    lifecycle is actually load-bearing here, not decorative. -/
theorem gate_false_when_creating (policy : AgentPolicy) (action : String) :
    policyGate LifecycleState.creating policy action = false := rfl

theorem gate_false_when_terminating (policy : AgentPolicy) (action : String) :
    policyGate LifecycleState.terminating policy action = false := rfl

theorem gate_false_when_destroyed (policy : AgentPolicy) (action : String) :
    policyGate LifecycleState.destroyed policy action = false := rfl

/-- Mirrors the Rust test `restrictive_policy_grants_no_capabilities`: a
    policy with no allowed capabilities allows nothing, even while running. -/
theorem empty_policy_allows_nothing (action : String) :
    (AgentPolicy.mk []).allows action = false := by
  simp [AgentPolicy.allows]

end AgentVm
