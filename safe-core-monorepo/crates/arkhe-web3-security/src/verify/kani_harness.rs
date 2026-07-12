//! Kani model-checking harnesses for `contracts::reentrancy` (SC08).
//!
//! These prove properties over *symbolic* inputs (`kani::any()`), not just
//! the handful of example cases already covered by the unit tests in
//! `contracts::reentrancy::tests`.
//!
//! **Verification status (honest disclosure):** the `kani` crate isn't on
//! crates.io under real semver (only placeholder `0.0.0`/`0.0.1` — the
//! `kani = "0.55.0"` dev-dependency in the sibling `safe-core-crypto` crate
//! doesn't actually resolve, confirmed by trying it). Its real source lives
//! at `github.com/model-checking/kani`, tag `kani-0.61.0`; temporarily
//! adding that as a git dependency (see the `Cargo.toml` comment above
//! `[lints.rust]` for the exact snippet — it's not committed there
//! permanently because `cargo publish` refuses any git dependency, even
//! optional/feature-gated ones) confirmed the real crate resolves and pulls
//! in real `kani`/`kani_macros`/`kani_core` — but that crate itself
//! requires the exact nightly Kani was built against
//! (`nightly-2025-04-03` with `rustc-dev`+`llvm-tools`, per its own
//! `rust-toolchain.toml`) to type-check, which isn't installed here. So:
//! the API surface used below (`#[kani::proof]`, `kani::any()`,
//! `#[derive(kani::Arbitrary)]`, `kani::assume`) is confirmed real, but
//! these harnesses have **not** been fully type-checked or model-checked in
//! this session. To verify for real:
//!
//! ```text
//! rustup toolchain install nightly-2025-04-03 --component rustc-dev,llvm-tools,rust-src
//! # add the temporary git dependency from Cargo.toml's comment, then:
//! cargo +nightly-2025-04-03 rustc -p arkhe-web3-security --lib -- --cfg kani   # type-check
//! cargo install --locked kani-verifier && cargo kani setup                     # then, to actually model-check:
//! cargo +nightly-2025-04-03 kani
//! ```

use crate::contracts::reentrancy::{check_effects_interactions_order, Op, ReentrancyGuard};

/// Bounded number of legitimate enter/exit cycles to perform before probing
/// the guard with a reentrant call. Kept small so Kani can explore it
/// exhaustively rather than needing an unbounded loop.
#[derive(kani::Arbitrary)]
struct GuardScenario {
    prior_cycles: u8,
}

/// FI-W04: no matter how many prior *legitimate* (sequential) enter/exit
/// cycles a guard has been through, a nested `enter()` call made while it is
/// still locked must always be rejected. This is the actual reentrancy
/// vector (a callback re-entering mid-call) — proving it only for one
/// hand-picked starting state (as a `by simp` tautology would) doesn't cover
/// this; Kani explores every `prior_cycles` value up to the bound below.
#[kani::proof]
fn prove_reentrancy_guard_rejects_nested_enter_after_any_prior_cycles() {
    let scenario: GuardScenario = kani::any();
    kani::assume(scenario.prior_cycles <= 3);

    let mut guard = ReentrancyGuard::new();
    for _ in 0..scenario.prior_cycles {
        guard.enter().expect("guard is unlocked between completed cycles");
        guard.exit();
    }

    // Guard is unlocked here regardless of `prior_cycles`.
    guard.enter().expect("guard is unlocked immediately before this call");
    // A nested reentrant call — the guard is now locked — must be rejected.
    assert!(guard.enter().is_err());
}

/// `execute_guarded` must release the lock once the guarded closure
/// completes, so a legitimate *sequential* follow-up call still succeeds
/// (the guard must not falsely stay locked forever after a clean exit).
#[kani::proof]
fn prove_execute_guarded_releases_lock_after_completion() {
    let mut guard = ReentrancyGuard::new();
    let result = guard.execute_guarded(|| 7u8);
    assert_eq!(result, Ok(7));
    assert!(guard.enter().is_ok());
}

/// Formal equivalence check between `check_effects_interactions_order` and
/// its specification ("violates iff some `Effect` follows some earlier
/// `ExternalCall`"), over every 3-element sequence of `Op`s — not just the
/// two example sequences the unit tests cover.
#[kani::proof]
fn prove_cei_check_matches_spec_for_all_three_op_sequences() {
    let flags: [bool; 3] = kani::any();
    let ops: Vec<Op> = flags
        .iter()
        .map(|&is_external_call| if is_external_call { Op::ExternalCall } else { Op::Effect })
        .collect();

    let spec_violates = ops.iter().enumerate().any(|(i, op)| {
        matches!(op, Op::Effect) && ops[..i].iter().any(|earlier| matches!(earlier, Op::ExternalCall))
    });

    let impl_violates = !check_effects_interactions_order(&ops).holds();
    assert_eq!(spec_violates, impl_violates);
}
