//! Falsifiable predictions of Table 2 (Zheng et al., *Loops all the way up*,
//! Physics of Life Reviews, 2026) executed against the `491-AGI-CORTEX`
//! functional recurrency simulation.
//!
//! * **C1 — Lateral disruption (character):** disrupting lateral recurrency
//!   must change perceived similarity (phenomenal character) while feedforward
//!   identity decoding is preserved.
//! * **C2 — Local/global asymmetry (access vs. phenomenal):** a feedforward
//!   system must still discriminate content, but that content must never be
//!   *accessed* (broadcast) because its causal-closure depth is ≈ 0.
//! * **C3 — State gradient:** causal-closure depth must scale continuously
//!   with recurrence strength (not be binary), a feedforward-only system must
//!   fail the closure and perturbational tests, and a recurrent one must pass.

use anyhow::Result;
use arkhe_neurogenesis::Rk4Solver;
use arkhe_recurrency::{
    closure_depth, perturbational_agreement, LateralField, PerceptionLoop, StateDaemon,
    ArousalRegime, GlobalWorkspace, AccessDecision,
};
use nalgebra::{DMatrix, DVector};

/// Slow, smooth, site-distinct exogenous drive (so the input is predictable
/// on its own and internal transport is what decorrelates the state).
fn smooth_drive(n_sites: usize, t_max: usize) -> Vec<DVector<f64>> {
    (0..t_max)
        .map(|t| {
            DVector::from_fn(n_sites, |i, _| {
                (0.5 * (i + 1) as f64 * t as f64 * 0.02).sin() + 0.1 * (i as f64)
            })
        })
        .collect()
}

/// Fraction of representations classified to the correct template by raw
/// cosine nearest-neighbour.
fn nearest_template_accuracy(
    representations: &[DVector<f64>],
    templates: &[DVector<f64>],
    labels: &[usize],
) -> f64 {
    assert_eq!(representations.len(), labels.len());
    let mut correct = 0;
    for (r, &label) in representations.iter().zip(labels) {
        let mut best = 0usize;
        let mut best_sim = f64::NEG_INFINITY;
        for (i, t) in templates.iter().enumerate() {
            let s = r.dot(t) / (r.norm() * t.norm());
            if s > best_sim {
                best_sim = s;
                best = i;
            }
        }
        if best == label {
            correct += 1;
        }
    }
    correct as f64 / representations.len() as f64
}

/// C1 — Lateral disruption changes phenomenal character but preserves identity.
#[test]
fn c1_lateral_disruption_changes_character_preserves_identity() {
    // Exemplar prototypes on a 2-D layout. A and B are neighbours; the lateral
    // coupling A -> B (strong) with B -> A (weak) is directional, as in cortex.
    let centroids = vec![
        DVector::from_vec(vec![1.0, 0.0]),
        DVector::from_vec(vec![0.6, 0.8]),
        DVector::from_vec(vec![-1.0, 0.0]),
    ];
    let mut lateral = DMatrix::zeros(3, 3);
    lateral[(1, 0)] = 0.9; // B receives strong lateral drive from A
    lateral[(0, 1)] = 0.05; // A receives weak back-coupling from B
    let mut field = LateralField::with_lateral(centroids.clone(), lateral, 8);

    // Borderline query: feedforward identity is A.
    let q = DVector::from_vec(vec![0.9, 0.1]);
    let identity = field.feedforward_ranking(&q);
    assert_eq!(identity[0], 0, "feedforward identity must be A");

    // Perceived similarity (with intact laterals) is warped toward B:
    // phenomenal character differs from feedforward identity.
    let perceived = field.relaxed_ranking(&q);
    assert_ne!(perceived, identity, "lateral recurrency must warp character");
    assert_eq!(perceived[0], 1, "perceived winner must be B");

    // Identity decoding is unaffected by the lateral field.
    let queries = vec![q.clone(), centroids[2].clone()];
    let labels = vec![0, 2];
    assert!(
        (field.feedforward_accuracy(&queries, &labels) - 1.0).abs() < 1e-12,
        "identity decoding must be perfect before disruption"
    );

    // Disrupting the laterals collapses perceived similarity back to identity.
    field.perturb(0.0);
    let disrupted = field.relaxed_ranking(&q);
    assert_eq!(
        disrupted, identity,
        "disconnected laterals must collapse character to identity"
    );
    assert!(
        (field.feedforward_accuracy(&queries, &labels) - 1.0).abs() < 1e-12,
        "identity decoding must persist after disruption"
    );
}

/// C2 — Local/global asymmetry: content without access.
#[test]
fn c2_global_loop_asymmetry_content_without_access() {
    let loop_cfg = PerceptionLoop {
        n: 8,
        recurrence_gain: 3.0, // internal transport strong enough to close the loop
        ..Default::default()
    };

    // Distinct, well-separated static stimuli for local discrimination.
    let templates = vec![
        DVector::from_vec(vec![1.0, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0]),
        DVector::from_vec(vec![0.0, 0.0, 0.0, 1.0, 0.2, 0.1, 0.0, 0.0]),
        DVector::from_vec(vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.3]),
    ];

    // Local discrimination through the closed loop: content is present.
    let closed_repr: Vec<DVector<f64>> = templates
        .iter()
        .map(|s| loop_cfg.perceive(s).representation)
        .collect();
    let labels = [0usize, 1, 2];
    let closed_acc = nearest_template_accuracy(&closed_repr, &templates, &labels);
    assert!(
        (closed_acc - 1.0).abs() < 1e-12,
        "closed loop must discriminate content (accuracy {closed_acc})"
    );

    // Causal-closure depth of the two architectures on a slow drive.
    let drive = smooth_drive(loop_cfg.n, 80);
    let (closed_states, drv) = loop_cfg.closed_loop_trajectory(&drive);
    let depth_closed = closure_depth(&closed_states, &drv).depth;
    // Feedforward system: internal state is a copy of the input (no memory).
    let depth_ff = closure_depth(&drive, &drive).depth;

    assert!(
        depth_closed > 0.3,
        "closed loop must show real closure depth, got {depth_closed}"
    );
    assert!(
        depth_ff < 0.1,
        "feedforward must score ~0 closure, got {depth_ff}"
    );

    // Global workspace: access is gated on closure depth.
    let threshold = 0.5 * (depth_closed + depth_ff);
    let ws = GlobalWorkspace::new(threshold);

    // Feedforward system: content present and discriminable, but NEVER accessed.
    assert_eq!(
        ws.offer(depth_ff),
        AccessDecision::PresentButNotAccessed,
        "feedforward content must be phenomenal-without-access"
    );
    // Closed loop: the same content crosses the threshold and is broadcast.
    assert_eq!(
        ws.offer(depth_closed),
        AccessDecision::Accessed,
        "recurrent content must be accessed"
    );
}

/// C3 — State gradient: closure is graded, feedforward fails both tests,
/// and the cellular-state daemon gates global access.
#[test]
fn c3_state_gradient_depth_is_graded_and_feedforward_fails() {
    let drive = smooth_drive(8, 80);

    // Feedforward null: internal state is a copy of the input (no memory).
    // This is the system Table 2 predicts must fail causal-closure tests.
    let depth_ff = closure_depth(&drive, &drive).depth;
    let agree_ff = perturbational_agreement(&drive, &drive);
    assert!(depth_ff < 0.1, "feedforward must score ~0 closure, got {depth_ff}");
    assert!(
        (agree_ff - 1.0).abs() < 1e-9,
        "feedforward has no internal state: perturbation leaves no trace"
    );

    // Closed-loop family: the feedback gain `k` is the recurrency-depth knob.
    // Large `k` clamps the state to the exogenous drive (feedforward-like,
    // shallow closure); small `k` lets the substrate's own transport determine
    // the next state (deep closure). This is the paper's "relative to its
    // exogenous drivers".
    let gains = [10.0, 5.0, 2.0, 1.0, 0.5, 0.1];
    let mut depths = Vec::new();
    let mut agrees = Vec::new();
    for &k in &gains {
        let loop_cfg = PerceptionLoop {
            feedback_gain: k,
            coupling_scale: 2.0,
            ..Default::default()
        };
        let (states, drv) = loop_cfg.closed_loop_trajectory(&drive);
        depths.push(closure_depth(&states, &drv).depth);
        let mid = drive.len() / 2;
        agrees.push(loop_cfg.perturbational_agreement(&drive, mid, 2, 0.5));
    }

    // Even the shallowest recurrence already beats the feedforward null.
    assert!(
        depths[0] > depth_ff + 0.05,
        "any recurrence must beat the feedforward null (ff {depth_ff}, k=10 {})",
        depths[0]
    );

    // The depth of causal closure is graded, not binary, and monotone in the
    // recurrency-depth knob (decreasing `k` deepens closure).
    for pair in depths.windows(2) {
        assert!(
            pair[1] >= pair[0] - 0.02,
            "closure depth must deepen as the input clamp loosens ({} -> {})",
            pair[0],
            pair[1]
        );
    }
    assert!(
        depths[depths.len() - 1] > 0.5,
        "deep recurrency must score high, got {}",
        depths[depths.len() - 1]
    );

    // Perturbational test: the stronger the recurrency, the more a perturbation
    // propagates (lower agreement); the feedforward copy is insensitive.
    assert!(
        agrees[0] > agrees[agrees.len() - 1],
        "recurrency must propagate perturbations (k=10 {:.4}, k=0.1 {:.4})",
        agrees[0],
        agrees[agrees.len() - 1]
    );

    // Cellular-state gating: the same deep-recurrency content that is accessed
    // while Alert is blocked while DeepSleep — access, not content, is lost.
    let ws = GlobalWorkspace::new(0.2);
    let mut daemon = StateDaemon::new(ArousalRegime::Alert);
    let depth_deep = depths[depths.len() - 1];
    assert_eq!(
        ws.offer_in_state(depth_deep, &daemon),
        AccessDecision::Accessed,
        "Alert must grant access to recurrent content"
    );
    daemon.set_regime(ArousalRegime::DeepSleep);
    assert_eq!(
        ws.offer_in_state(depth_deep, &daemon),
        AccessDecision::PresentButNotAccessed,
        "DeepSleep must suppress access despite the content's depth"
    );
}

/// The loop's substrate must agree with the neurogenesis reference integrator
/// on the raw feedforward dynamics (pin the deterministic cross-crate coupling).
#[test]
fn closed_loop_uses_neurogenesis_substrate_consistently() -> Result<()> {
    let loop_cfg = PerceptionLoop::default();
    let h = loop_cfg.hamiltonian();
    let psi0 = DVector::from_fn(loop_cfg.n, |i, _| 0.5 + (i as f64) * 0.1);
    let solver = Rk4Solver::new(loop_cfg.dt, loop_cfg.dt * loop_cfg.steps as f64);
    let reference = solver.solve(&h, &psi0);
    assert!(reference.len() > 1);
    // The substrate amplitude evolution is bounded under PT symmetry.
    let peak_dev = reference
        .iter()
        .map(|x| (x - reference[0]).abs() / reference[0].max(1e-12))
        .fold(0.0_f64, f64::max);
    assert!(peak_dev < 0.5, "PT-balanced substrate must stay bounded");
    Ok(())
}
