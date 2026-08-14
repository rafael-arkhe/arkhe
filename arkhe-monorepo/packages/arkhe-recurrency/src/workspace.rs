//! Global workspace — the *global* level of recurrency.
//!
//! Global recurrency is what the paper attributes to the *access* facet:
//! content becomes reportable/globally-broadcast only when the system-wide
//! recurrence crosses a threshold (the front of the workspace). Critically,
//! content can be present *without* access — phenomenal without access — which
//! is the asymmetry Table 2 asks us to make falsifiable.
//!
//! The workspace here is deliberately a dumb broadcast gate: it holds no
//! representation itself, it only decides whether a candidate content — which
//! has already been produced by the local [`crate::cycle::PerceptionLoop`] —
//! is *accessed* (broadcast to downstream consumers) given its measured
//! causal-closure depth. A feedforward-only system produces content with depth
//! ≈ 0 and is therefore never accessed.

//! offer_in_state uses `daemon.regime().allows_global_access()`. The regime
//! enum itself lives in [`crate::state`].

/// Decision of the workspace for an offered content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum AccessDecision {
    /// Closure depth crossed the threshold; content broadcast downstream.
    Accessed,
    /// Content present but its recurrence depth is too shallow; not broadcast.
    PresentButNotAccessed,
}

/// Gated broadcast gate for consciously-accessed content.
#[derive(Clone, Debug, serde::Serialize)]
pub struct GlobalWorkspace {
    /// Minimum causal-closure depth required for access.
    pub closure_threshold: f64,
}

impl GlobalWorkspace {
    /// Create a workspace with the given access threshold.
    pub fn new(closure_threshold: f64) -> Self {
        Self { closure_threshold }
    }

    /// Offer a content produced by the local loop.
    ///
    /// `closure` is the causal-closure depth of the content's representation
    /// (see [`crate::closure_depth`]). Access is granted iff
    /// `closure >= closure_threshold`.
    pub fn offer(&self, closure: f64) -> AccessDecision {
        if closure >= self.closure_threshold {
            AccessDecision::Accessed
        } else {
            AccessDecision::PresentButNotAccessed
        }
    }

    /// Offer a content under a given conscious state.
    ///
    /// A [`StateDaemon`](crate::StateDaemon) in a reduced-consciousness regime
    /// blocks global access even when the content's closure depth crosses the
    /// threshold: cellular recurrency (the conscious state) gates global
    /// recurrency (access).
    pub fn offer_in_state(&self, closure: f64, daemon: &crate::StateDaemon) -> AccessDecision {
        if daemon.regime().allows_global_access() {
            self.offer(closure)
        } else {
            AccessDecision::PresentButNotAccessed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_requires_threshold() {
        let ws = GlobalWorkspace::new(0.3);
        assert_eq!(ws.offer(0.9), AccessDecision::Accessed);
        assert_eq!(ws.offer(0.2), AccessDecision::PresentButNotAccessed);
    }

    #[test]
    fn boundary_is_accessed() {
        let ws = GlobalWorkspace::new(0.5);
        assert_eq!(ws.offer(0.5), AccessDecision::Accessed);
    }
}
