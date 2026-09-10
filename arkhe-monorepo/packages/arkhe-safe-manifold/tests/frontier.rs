//! Property-based tests for frontier dynamics (bloco 1073).
//!
//! Behavioral checks — the crate's `proptest` dev-dependency (already used in
//! `integration.rs`) verifies structural invariants of the real substrate.

use arkhe_safe_manifold::{
    dominant_invariant, sample_interleaved, severity, EscapeRegion, FrontierHistory,
};
use proptest::prelude::*;

fn region_strategy() -> impl Strategy<Value = EscapeRegion> {
    prop_oneof![
        Just(EscapeRegion::Safe),
        Just(EscapeRegion::Warning),
        Just(EscapeRegion::Boundary),
        Just(EscapeRegion::Continuum),
        Just(EscapeRegion::Outside),
    ]
}

proptest! {
    /// Property: dominant of a homogeneous window is that region.
    #[test]
    fn prop_dominant_homogeneous(region in region_strategy(), n in 1usize..50) {
        let h = vec![region; n];
        prop_assert_eq!(dominant_invariant(&h), Some(region));
    }

    /// Property: dominant of an empty window is None.
    #[test]
    fn prop_dominant_empty(_seed in 0usize..1usize) {
        prop_assert_eq!(dominant_invariant(&[] as &[EscapeRegion]), None);
    }

    /// Property: severity is a total monotonic ranking 0..=4.
    #[test]
    fn prop_severity_in_range(region in region_strategy()) {
        let s = severity(region);
        prop_assert!((0..=4).contains(&s));
    }

    /// Property: interleave length == real.len() * (ratio + 1).
    #[test]
    fn prop_interleave_length(
        real in prop::collection::vec(region_strategy(), 1..50),
        imagined in prop::collection::vec(region_strategy(), 1..50),
        ratio in 1usize..5,
    ) {
        let out = sample_interleaved(&real, &imagined, ratio);
        prop_assert_eq!(out.len(), real.len() * (ratio + 1));
    }

    /// Property: interleave preserves real-first ordering (DynaWeb doctrine:
    /// imagined augments, never replaces, real experience).
    #[test]
    fn prop_interleave_real_first(
        real in prop::collection::vec(region_strategy(), 1..20),
        imagined in prop::collection::vec(region_strategy(), 1..20),
        ratio in 1usize..4,
    ) {
        let out = sample_interleaved(&real, &imagined, ratio);
        for (i, r) in real.iter().enumerate() {
            prop_assert_eq!(&out[i * (ratio + 1)], r);
        }
    }

    /// Property: FrontierHistory is bounded (never exceeds capacity).
    #[test]
    fn prop_window_bounded(
        sample in prop::collection::vec(region_strategy(), 0..100),
        capacity in 1usize..20,
    ) {
        let mut w = FrontierHistory::new(capacity);
        for region in &sample {
            w.observe(*region);
        }
        prop_assert!(w.len() <= capacity);
        if sample.is_empty() {
            prop_assert!(w.is_empty());
        }
    }
}