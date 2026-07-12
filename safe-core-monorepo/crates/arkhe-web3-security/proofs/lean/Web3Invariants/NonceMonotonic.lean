/-!
# FI-W07 — Non-replay of signatures via a strictly monotonic nonce.

Formal counterpart to `wallets::signature::NonceTracker` in the Rust
implementation (`crates/arkhe-web3-security/src/wallets/signature.rs`). See
`../README.md` for verification status — not type-checked in this session.
-/

namespace Web3Invariants

/-- `last = none` means no nonce has been consumed yet for this account.
    `consumeNonce last n` mirrors `NonceTracker::consume`: accepted only if
    `n` is strictly greater than the last nonce seen. -/
def consumeNonce (last : Option Nat) (n : Nat) : Bool :=
  match last with
  | none => true
  | some last => decide (n > last)

/-- Mirrors the Rust unit test `replayed_nonce_is_rejected`: reusing the
    exact same nonce again is always rejected. -/
theorem replayed_nonce_is_rejected (last : Nat) :
    consumeNonce (some last) last = false := by
  simp only [consumeNonce, decide_eq_false_iff_not]
  omega

/-- Mirrors the Rust unit test `out_of_order_nonce_is_rejected`: any nonce
    not strictly greater than the last one is rejected. -/
theorem out_of_order_nonce_is_rejected (last n : Nat) (h : n ≤ last) :
    consumeNonce (some last) n = false := by
  simp only [consumeNonce, decide_eq_false_iff_not]
  omega

/-- Mirrors the Rust unit test `strictly_increasing_nonces_hold`: any
    strictly increasing nonce is accepted. -/
theorem increasing_nonce_is_accepted (last n : Nat) (h : n > last) :
    consumeNonce (some last) n = true := by
  simp only [consumeNonce, decide_eq_true_iff]
  omega

/-- The very first nonce for a fresh account is always accepted, regardless
    of its value — mirrors `NonceTracker`'s `HashMap::get` returning `None`
    for an account not seen before. -/
theorem first_nonce_is_always_accepted (n : Nat) :
    consumeNonce none n = true := by
  simp [consumeNonce]

end Web3Invariants
