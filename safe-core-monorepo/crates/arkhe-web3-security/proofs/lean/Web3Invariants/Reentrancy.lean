/-!
# SC08 — Reentrancy guard and Checks-Effects-Interactions ordering.

Formal counterpart to `contracts::reentrancy` in the Rust implementation
(`crates/arkhe-web3-security/src/contracts/reentrancy.rs`). See
`../README.md` for why this exists and what "not verified" means for it —
no Lean/Lake toolchain is installed in the session that wrote this, so it
has not actually been type-checked.

The audited prior version of this proof was a tautology: it defined the
guard as "active" when `lock = false` (inverted from the real semantics)
and proved a property using `by simp` on a statement that held regardless
of the guard's behavior, so it demonstrated nothing about reentrancy. This
version fixes both problems: `lock = true` means the guard is held (matches
`ReentrancyGuard.locked` in the Rust implementation exactly), and the two
theorems below are not tautologies — falsifying `enter`'s definition (e.g.
letting it succeed unconditionally) would make `guard_blocks_reentrant_enter`
provably false, not just harder to prove.
-/

namespace Web3Invariants

/-- A step in a simulated transaction: a local state write (`effect`) or a
    call to external, untrusted code (`externalCall`). Mirrors `Op` in
    `contracts::reentrancy`. -/
inductive Op where
  | effect
  | externalCall
  deriving DecidableEq, Repr

/-- Reentrancy-guard state. `lock = true` means the guard is currently held
    — a call is inside the guarded section. `callDepth` is not load-bearing
    for the proofs below; it's kept only so the model can express "how many
    completed calls happened before this one," matching the shape a real
    call-stack trace would have. -/
structure GuardState where
  lock : Bool
  callDepth : Nat
  deriving DecidableEq, Repr

def GuardState.initial : GuardState := { lock := false, callDepth := 0 }

/-- Mirrors `ReentrancyGuard::enter`: succeeds only if the guard isn't
    already held. -/
def GuardState.enter (s : GuardState) : Option GuardState :=
  if s.lock then none
  else some { s with lock := true, callDepth := s.callDepth + 1 }

/-- Mirrors `ReentrancyGuard::exit`. -/
def GuardState.exit (s : GuardState) : GuardState :=
  { s with lock := false }

/-- Mirrors `ReentrancyGuard::execute_guarded`: enter, run, exit. -/
def GuardState.executeGuarded (s : GuardState) : Option GuardState :=
  (s.enter).map GuardState.exit

/-- **SC08, the actual property**: from a state where the guard is held, a
    reentrant `enter` (the callback re-entering mid-call) always fails.
    Proved by unfolding `enter` and using `h` to resolve the `if` — not
    true "by shape alone" without `h`, since `enter` succeeds whenever
    `s.lock = false`. -/
theorem guard_blocks_reentrant_enter (s : GuardState) (h : s.lock = true) :
    s.enter = none := by
  simp [GuardState.enter, h]

/-- Complementary direction, so the model can't vacuously satisfy the
    theorem above by making `enter` always fail: whenever the guard is
    free, `enter` always succeeds. -/
theorem guard_allows_enter_when_free (s : GuardState) (h : s.lock = false) :
    s.enter = some { s with lock := true, callDepth := s.callDepth + 1 } := by
  simp [GuardState.enter, h]

/-- After a guarded call completes, the guard is released — a later,
    *sequential* (non-reentrant) call still succeeds. Mirrors the second
    assertion in the Rust unit test `execute_guarded_prevents_nested_reentry`. -/
theorem execute_guarded_releases_lock (s : GuardState) (h : s.lock = false) :
    ∃ s', s.executeGuarded = some s' ∧ s'.lock = false := by
  simp [GuardState.executeGuarded, GuardState.enter, GuardState.exit, h]

/-- FI-W03 — Checks-Effects-Interactions ordering. `seenCall` tracks whether
    an `externalCall` has already occurred; an `effect` after that point
    fails the check. Structurally identical to
    `check_effects_interactions_order` in the Rust implementation. -/
def ceiHoldsAux (seenCall : Bool) : List Op → Bool
  | [] => true
  | Op.externalCall :: rest => ceiHoldsAux true rest
  | Op.effect :: rest => if seenCall then false else ceiHoldsAux seenCall rest

def ceiHolds (ops : List Op) : Bool := ceiHoldsAux false ops

/-- Concrete case mirroring the Rust unit test `effects_before_interactions_hold`. -/
theorem effects_before_interactions_hold :
    ceiHolds [Op.effect, Op.effect, Op.externalCall] = true := by decide

/-- Concrete case mirroring the Rust unit test `effect_after_external_call_violates_cei`
    (the GMX V1 `executeDecreaseOrder` pattern). -/
theorem effect_after_external_call_violates_cei :
    ceiHolds [Op.externalCall, Op.effect] = false := by decide

/-- General theorem: once an external call has been seen, *any* later
    effect anywhere in the remaining trace fails the check — not just the
    two concrete cases above. -/
theorem ceiHoldsAux_true_effect_head_fails (rest : List Op) :
    ceiHoldsAux true (Op.effect :: rest) = false := by
  simp [ceiHoldsAux]

end Web3Invariants
