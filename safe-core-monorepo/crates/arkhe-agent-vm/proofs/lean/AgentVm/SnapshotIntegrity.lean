import AgentVm.PolicyGate

/-!
# FI-A05 — Snapshot integrity.

Formal counterpart to `AavmSnapshot` in `../../src/snapshot.rs`. See
`../README.md` for verification status.

**Honest scope note:** this does **not** prove anything about BLAKE3
itself — collision resistance is a cryptographic assumption about a
concrete algorithm, not something derivable inside Lean's logic. `hashFn`
below is an *opaque, uninterpreted* function; the tamper-detection theorem
takes injectivity of `hashFn` as an explicit hypothesis rather than an
axiom, so the theorem's actual content is "IF the hash function is
injective over the fields it's applied to, THEN tampering is detected" —
not a claim that BLAKE3 satisfies that hypothesis (a claim this file makes
no attempt to substantiate).
-/

namespace AgentVm

/-- Mirrors the fields `AavmSnapshot::payload_bytes` serializes in the Rust
    implementation — not the literal byte encoding, which is irrelevant to
    the properties proven here. -/
structure SnapshotFields where
  id : String
  state : LifecycleState
  ageSecs : Nat
  maxLifetimeSecs : Nat
  allowedCapabilities : List String
  takenAt : Nat
  deriving DecidableEq, Repr

/-- Opaque stand-in for BLAKE3 — see the module doc comment. -/
opaque hashFn : SnapshotFields → Nat

structure Snapshot where
  fields : SnapshotFields
  contentHash : Nat
  deriving DecidableEq, Repr

/-- Mirrors `AavmSnapshot::capture`. -/
def capture (fields : SnapshotFields) : Snapshot :=
  { fields := fields, contentHash := hashFn fields }

/-- Mirrors `AavmSnapshot::verify_integrity`. -/
def Snapshot.verifyIntegrity (s : Snapshot) : Prop :=
  hashFn s.fields = s.contentHash

/-- A freshly captured snapshot always passes its own integrity check —
    true by construction (`contentHash` is defined as `hashFn fields`),
    regardless of what `hashFn` actually computes. -/
theorem fresh_capture_passes_integrity (fields : SnapshotFields) :
    (capture fields).verifyIntegrity := rfl

/-- Capturing has no hidden dependency on mutable state: the same fields
    always produce the same snapshot. -/
theorem capture_is_deterministic (fields : SnapshotFields) :
    capture fields = capture fields := rfl

/-- Tamper detection: replacing a captured snapshot's fields with different
    ones, while keeping the *original* `contentHash`, fails the integrity
    check — **given** `hashFn` is injective (stated as a hypothesis, not
    assumed true of BLAKE3 by this file). This is the actual content of
    "verify_integrity catches tampering": a corrupted/substituted snapshot
    doesn't pass unless it happens to collide with the original hash. -/
theorem tamper_is_detected_when_hash_is_injective
    (original tampered : SnapshotFields)
    (hInjective : ∀ a b, hashFn a = hashFn b → a = b)
    (hDiff : original ≠ tampered) :
    ¬ (Snapshot.mk tampered (capture original).contentHash).verifyIntegrity := by
  intro hIntegrity
  exact hDiff (hInjective original tampered hIntegrity.symm)

end AgentVm
