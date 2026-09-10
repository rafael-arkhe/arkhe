//! Frontier dynamics for the ARKHE‑χ SafeManifold — bloco 1073.
//!
//! Implements the extracted doctrine from DynaWeb (arXiv:2601.22149):
//! **interleaving of real trajectories with imagined (model-generated)
//! trajectories**. Here the substrate is real: [`EscapeRegion`]
//! (`escape_region.rs:12`) is the macro-state of the safe envelope, and the
//! dominant (modal) region is the operant defect level.
//!
//! Honesty note: this is **construction on verified substrate** — it extends
//! the crate's public surface without modifying `escape_region.rs` /
//! `safe_manifold.rs` (all referenced hashes remain unchanged, bloco 1072).

use std::collections::{HashMap, VecDeque};

use crate::escape_region::EscapeRegion;

/// Monotonic severity rank of an escape region (0 = safest).
///
/// Kept as a free function (not `Ord` on the enum) to avoid touching the
/// anchored `EscapeRegion` type.
pub fn severity(region: EscapeRegion) -> u8 {
    match region {
        EscapeRegion::Safe => 0,
        EscapeRegion::Warning => 1,
        EscapeRegion::Boundary => 2,
        EscapeRegion::Continuum => 3,
        EscapeRegion::Outside => 4,
    }
}

/// Dominant (modal) escape region over a history window.
///
/// Returns the most frequent region. Ties favour the **more severe** region
/// (conservative: an ambiguous window conserves the worst defect signal).
/// An empty history yields `None`.
pub fn dominant_invariant(history: &[EscapeRegion]) -> Option<EscapeRegion> {
    if history.is_empty() {
        return None;
    }
    let mut counts: HashMap<u8, usize> = HashMap::new();
    let mut by_rank: HashMap<u8, EscapeRegion> = HashMap::new();
    for &region in history {
        let rank = severity(region);
        *counts.entry(rank).or_insert(0) += 1;
        by_rank.entry(rank).or_insert(region);
    }
    counts
        .into_iter()
        .max_by(|(ra, ca), (rb, cb)| ca.cmp(cb).then_with(|| ra.cmp(rb)))
        .map(|(rank, _)| by_rank[&rank])
}

/// Sliding window of escape-region observations (bounded, append-only).
#[derive(Debug, Clone)]
pub struct FrontierHistory {
    window: VecDeque<EscapeRegion>,
    capacity: usize,
}

impl FrontierHistory {
    /// Creates a window with a bounded capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(capacity),
            capacity: capacity.max(1),
        }
    }

    /// Appends an observation (drops the oldest if at capacity).
    pub fn observe(&mut self, region: EscapeRegion) {
        if self.window.len() == self.capacity {
            self.window.pop_front();
        }
        self.window.push_back(region);
    }

    /// Current dominant region of the window.
    pub fn dominant(&self) -> Option<EscapeRegion> {
        dominant_invariant(&self.window.iter().copied().collect::<Vec<_>>())
    }

    /// True if the most recent observation changed the modal region.
    ///
    /// Compares the window's mode with the mode of the window **without its
    /// newest element** — a semantics that actually detects modal drift
    /// (the phantom `front()`-comparison criticised in bloco 1072 is avoided).
    pub fn dominant_changed(&self) -> bool {
        if self.window.len() < 2 {
            return false;
        }
        let mut prev = self.window.clone();
        prev.pop_back();
        dominant_invariant(&prev.into_iter().collect::<Vec<_>>())
            != dominant_invariant(&self.window.iter().copied().collect::<Vec<_>>())
    }

    /// Number of observations retained.
    pub fn len(&self) -> usize {
        self.window.len()
    }

    /// Window is empty.
    pub fn is_empty(&self) -> bool {
        self.window.is_empty()
    }
}

/// Interleaves real observations with imagined (model-generated) ones.
///
/// Output length: `real.len() * (1 + ratio)`. Each real observation is
/// followed by `ratio` imagined observations, cyclically picked when the
/// imagined buffer is shorter than required. Mirrors the DynaWeb doctrine:
/// imagined experience augments — never replaces — real experience.
pub fn sample_interleaved(
    real: &[EscapeRegion],
    imagined: &[EscapeRegion],
    ratio: usize,
) -> Vec<EscapeRegion> {
    let mut out = Vec::with_capacity(real.len().saturating_mul(ratio.saturating_add(1)));
    for r in real {
        out.push(*r);
        for i in 0..ratio {
            let source = if imagined.is_empty() {
                // No imagined data: the real observation stands alone.
                out.push(*r);
                continue;
            } else {
                imagined[i % imagined.len()]
            };
            out.push(source);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominant_simple_majority() {
        let h = vec![EscapeRegion::Safe, EscapeRegion::Safe, EscapeRegion::Warning];
        assert_eq!(dominant_invariant(&h), Some(EscapeRegion::Safe));
    }

    #[test]
    fn dominant_tie_breaks_towards_severe() {
        let h = vec![EscapeRegion::Safe, EscapeRegion::Warning];
        assert_eq!(dominant_invariant(&h), Some(EscapeRegion::Warning));
    }

    #[test]
    fn dominant_scales_outside() {
        let h = vec![
            EscapeRegion::Boundary,
            EscapeRegion::Continuum,
            EscapeRegion::Continuum,
            EscapeRegion::Outside,
            EscapeRegion::Outside,
            EscapeRegion::Outside,
        ];
        assert_eq!(dominant_invariant(&h), Some(EscapeRegion::Outside));
    }

    #[test]
    fn dominant_empty_is_none() {
        assert_eq!(dominant_invariant(&[]), None);
    }

    #[test]
    fn window_bounded_drops_oldest() {
        let mut w = FrontierHistory::new(3);
        w.observe(EscapeRegion::Safe);
        w.observe(EscapeRegion::Safe);
        w.observe(EscapeRegion::Safe);
        w.observe(EscapeRegion::Warning);
        assert_eq!(w.len(), 3);
        // Window = [Safe, Safe, Warning] → dominant still Safe.
        assert_eq!(w.dominant(), Some(EscapeRegion::Safe));
    }

    #[test]
    fn dominant_changed_detects_modal_drift() {
        let mut w = FrontierHistory::new(4);
        for _ in 0..3 {
            w.observe(EscapeRegion::Safe);
        }
        assert!(!w.dominant_changed());
        w.observe(EscapeRegion::Safe);
        assert!(!w.dominant_changed());
        // Drift window: [Safe, Safe, Warning, Warning]
        let mut w2 = FrontierHistory::new(4);
        w2.observe(EscapeRegion::Safe);
        w2.observe(EscapeRegion::Safe);
        w2.observe(EscapeRegion::Warning);
        assert!(!w2.dominant_changed());
        w2.observe(EscapeRegion::Warning);
        assert!(w2.dominant_changed());
    }

    #[test]
    fn interleave_length_and_order() {
        let real = vec![EscapeRegion::Safe, EscapeRegion::Warning];
        let imagined = vec![EscapeRegion::Boundary];
        let out = sample_interleaved(&real, &imagined, 2);
        assert_eq!(out.len(), real.len() * 3);
        assert_eq!(out[0], EscapeRegion::Safe);
        assert_eq!(out[1], EscapeRegion::Boundary);
        assert_eq!(out[2], EscapeRegion::Boundary);
        assert_eq!(out[3], EscapeRegion::Warning);
    }

    #[test]
    fn interleave_empty_imagined_keeps_real() {
        let real = vec![EscapeRegion::Outside];
        let out = sample_interleaved(&real, &[], 3);
        assert_eq!(out, vec![EscapeRegion::Outside; 4]);
    }

    #[test]
    fn severity_monotonic() {
        assert!(severity(EscapeRegion::Safe) < severity(EscapeRegion::Warning));
        assert!(severity(EscapeRegion::Warning) < severity(EscapeRegion::Boundary));
        assert!(severity(EscapeRegion::Boundary) < severity(EscapeRegion::Continuum));
        assert!(severity(EscapeRegion::Continuum) < severity(EscapeRegion::Outside));
    }
}