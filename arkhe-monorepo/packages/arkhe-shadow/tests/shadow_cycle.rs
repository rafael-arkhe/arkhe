//! Full Shadow cycle integration test.
//!
//! Mirrors the Block-22 narrative: a structured EVO/state matrix is compressed
//! (SVD), the discarded tail becomes the Shadow, and healing re-admits its
//! energy — the "Catedral cura" end-to-end path.

use arkhe_shadow::{Shadow, ShadowCycle, ShadowHealer};
use nalgebra::DMatrix;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// A rank-`rank` structured matrix the compression should expose a tail for.
fn structured_state(rows: usize, cols: usize, rank: usize, seed: u64) -> DMatrix<f64> {
    let mut rng = StdRng::seed_from_u64(seed);
    let a = DMatrix::from_fn(rows, rank, |_, _| rng.gen::<f64>() - 0.5);
    let b = DMatrix::from_fn(rank, cols, |_, _| rng.gen::<f64>() - 0.5);
    let signal = &a * &b;
    signal.map(|x| x + 0.05 * rng.gen::<f64>())
}

#[test]
fn shadow_cycle_compresses_then_heals() {
    let data = structured_state(60, 50, 4, 42);
    let cycle = arkhe_shadow::ShadowCycle::run(data.clone(), 6, 0.2);

    // Visible subset keeps the principal modes -> smaller than the true rank.
    assert_eq!(cycle.visible.shape(), (60, 50));
    assert!(cycle.shadow.len() > 0, "a noisy matrix must leave a tail");

    // Healing folds energy back in: healed energy > visible energy.
    assert!(
        cycle.total_energy() > cycle.visible_energy(),
        "healing should restore shadow energy"
    );

    // The shadow's strength is a unit quantity we can threshold against.
    assert!(cycle.shadow.strength() >= 0.0 && cycle.shadow.strength() <= 1.0);
}

#[test]
fn standalone_healer_reads_back_origin() {
    // A rank-3 state: cutting at 8 leaves a tail; trace_origin finds its peak.
    let data = structured_state(30, 30, 3, 21);
    let svd = data.clone().svd(true, true);
    let (_, shadow) = Shadow::split(
        &svd.u.expect("u"),
        &svd.v_t.expect("v"),
        &svd.singular_values,
        8,
    );
    let healer = ShadowHealer::new(data, shadow);
    let (sigma, idx) = healer.trace_origin();
    assert!(sigma >= 0.0);
    assert!(idx < healer.shadow.len());
    assert!(!healer.shadow.is_empty());
}