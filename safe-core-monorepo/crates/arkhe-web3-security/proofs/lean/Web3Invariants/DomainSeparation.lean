/-!
# FI-W06 — Structured-signature domain separation.

Formal counterpart to `wallets::eip712` in the Rust implementation
(`crates/arkhe-web3-security/src/wallets/eip712.rs`). Like the Rust module,
this simplifies away the actual EIP-712/keccak256 hashing — it models
`Domain` as a plain record compared structurally, since the property being
checked (domain separation and nonce matching) doesn't depend on the hash
primitive. See `../README.md` for verification status — not type-checked in
this session.
-/

namespace Web3Invariants

structure Domain where
  name : String
  version : String
  chainId : Nat
  verifyingContract : String
  deriving DecidableEq, Repr

structure StructuredSig where
  domain : Domain
  nonce : Nat
  deriving DecidableEq, Repr

/-- Mirrors `verify_domain_and_nonce`: valid only if both the domain and the
    nonce match exactly what's expected. -/
def verifyDomainAndNonce (sig : StructuredSig) (expectedDomain : Domain) (expectedNonce : Nat) : Bool :=
  decide (sig.domain = expectedDomain ∧ sig.nonce = expectedNonce)

/-- Mirrors the Rust unit test `matching_domain_and_nonce_is_valid`. -/
theorem matching_domain_and_nonce_is_valid
    (sig : StructuredSig) (expectedDomain : Domain) (expectedNonce : Nat)
    (hDomain : sig.domain = expectedDomain) (hNonce : sig.nonce = expectedNonce) :
    verifyDomainAndNonce sig expectedDomain expectedNonce = true := by
  simp only [verifyDomainAndNonce, decide_eq_true_iff]
  exact ⟨hDomain, hNonce⟩

/-- Mirrors the Rust unit test `wrong_nonce_is_rejected`. -/
theorem wrong_nonce_is_rejected
    (sig : StructuredSig) (expectedDomain : Domain) (expectedNonce : Nat)
    (hNonceDiff : sig.nonce ≠ expectedNonce) :
    verifyDomainAndNonce sig expectedDomain expectedNonce = false := by
  simp only [verifyDomainAndNonce, decide_eq_false_iff_not]
  intro h
  exact hNonceDiff h.2

/-- Mirrors the Rust unit test `cross_chain_replay_is_rejected`: a signature
    produced against one domain is never valid against a *different* one,
    even if the difference is only `chainId`. General over any two distinct
    domains, not just the chainId case, since the invariant doesn't
    actually depend on which field differs. -/
theorem cross_chain_replay_is_rejected
    (sig : StructuredSig) (domainA domainB : Domain) (expectedNonce : Nat)
    (hSig : sig.domain = domainA) (hDiff : domainA ≠ domainB) :
    verifyDomainAndNonce sig domainB expectedNonce = false := by
  simp only [verifyDomainAndNonce, decide_eq_false_iff_not]
  intro h
  exact hDiff (hSig.symm.trans h.1)

end Web3Invariants
