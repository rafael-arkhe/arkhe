//! Canonical projection functions ("Abel-Jacobi map" metaphor).
//!
//! Thin wrappers over [`SafeManifold`] methods for 16D projection.

use crate::safe_manifold::{SafeManifold, ManifoldPoint, ManifoldProfile};
use crate::invariants::{SystemState, SystemConfig};

/// Project a state onto the 16D SafeManifold.
pub fn embed_state(state: &SystemState, config: &SystemConfig) -> ManifoldPoint {
    let manifold = SafeManifold::with_config(config.clone());
    manifold.embed_state(state)
}

/// True if the point is not on the Theta boundary.
pub fn is_within_manifold(point: &ManifoldPoint) -> bool {
    !point.on_theta
}

/// Normalized 16D safety-distance score.
pub fn observer_defect(ideal: &SystemState, actual: &SystemState, config: &SystemConfig) -> f64 {
    let manifold = SafeManifold::with_config(config.clone());
    manifold.compute_observer_defect(ideal, actual)
}

/// Detect projection collision (expected many-to-one property).
pub fn collision_detected(s1: &SystemState, s2: &SystemState, config: &SystemConfig) -> bool {
    let manifold = SafeManifold::with_config(config.clone());
    manifold.collision_detected(s1, s2)
}

/// Profile equality (Torelli metaphor, 5D).
pub fn torelli_equivalence(p1: &ManifoldProfile, p2: &ManifoldProfile) -> bool {
    p1 == p2
}
