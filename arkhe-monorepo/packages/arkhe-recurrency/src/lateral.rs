//! Lateral recurrency field — the *lateral* level of recurrency.
//!
//! Lateral recurrency is what the paper attributes to the *character* facet of
//! a conscious content: the relational structure of *how* things are perceived
//! (the look of a face, the tone of a color) as opposed to *what* they are
//! (their category identity).
//!
//! Here that is modelled as a field of exemplar prototypes whose activation
//! relaxes through a lateral coupling matrix `L`: perceived similarity is the
//! fixed point of `a = a0 + L·a`, not the raw cosine distance. Because the
//! lateral weights are directional (as in cortex), this relaxation can warp
//! the perceived ranking of a borderline query away from its feedforward
//! identity — while feedforward decoding of *identity* stays intact.
//!
//! Table 2 predicts that **disrupting lateral recurrency changes phenomenal
//! character while preserving identity**; this module makes that falsifiable.

use nalgebra::{DMatrix, DVector};

/// A field of laterally-coupled exemplar prototypes.
#[derive(Clone, Debug)]
pub struct LateralField {
    /// Category prototypes; `centroids[i]` is exemplar `i`.
    pub centroids: Vec<DVector<f64>>,
    /// Lateral coupling `L` between exemplars (directional weights, zero diag).
    pub lateral: DMatrix<f64>,
    /// Number of relaxation iterations applied to a query's activation.
    pub relaxation_steps: usize,
}

impl LateralField {
    /// Build a field with no lateral coupling (pure feedforward).
    pub fn new(centroids: Vec<DVector<f64>>) -> Self {
        let k = centroids.len();
        Self {
            centroids,
            lateral: DMatrix::zeros(k, k),
            relaxation_steps: 1,
        }
    }

    /// Build a field with an explicit lateral coupling matrix.
    ///
    /// `lateral` must be `k×k` with a zero diagonal.
    pub fn with_lateral(
        centroids: Vec<DVector<f64>>,
        lateral: DMatrix<f64>,
        relaxation_steps: usize,
    ) -> Self {
        debug_assert_eq!(lateral.nrows(), centroids.len());
        debug_assert_eq!(lateral.ncols(), centroids.len());
        Self {
            centroids,
            lateral,
            relaxation_steps: relaxation_steps.max(1),
        }
    }

    /// Build a field whose lateral weights are the cosine similarities between
    /// prototypes (symmetric coupling).
    pub fn similarity_coupled(centroids: Vec<DVector<f64>>, strength: f64) -> Self {
        let k = centroids.len();
        let mut lateral = DMatrix::zeros(k, k);
        for i in 0..k {
            for j in 0..k {
                if i != j {
                    lateral[(i, j)] = strength * cosine(&centroids[i], &centroids[j]);
                }
            }
        }
        Self {
            centroids,
            lateral,
            relaxation_steps: 1,
        }
    }

    /// Relax a query's feedforward activation `a0` through the lateral field.
    ///
    /// `a0[i] = cosine(query, centroid[i])`. Returns the normalised relaxed
    /// activation vector after `relaxation_steps` iterations of
    /// `a ← a0 + L·a`.
    pub fn relaxed_activation(&self, query: &DVector<f64>) -> DVector<f64> {
        let a0 = DVector::from_fn(self.centroids.len(), |i, _| {
            cosine(query, &self.centroids[i]).max(0.0)
        });
        let mut a = normalize(&a0);
        for _ in 0..self.relaxation_steps {
            let next = &a0 + &self.lateral * &a;
            a = normalize(&next);
        }
        a
    }

    /// Ranking of exemplars by *perceived* (relaxed) similarity, descending.
    pub fn relaxed_ranking(&self, query: &DVector<f64>) -> Vec<usize> {
        let a = self.relaxed_activation(query);
        let mut idx: Vec<usize> = (0..self.centroids.len()).collect();
        idx.sort_by(|&i, &j| a[j].partial_cmp(&a[i]).unwrap_or(std::cmp::Ordering::Equal));
        idx
    }

    /// Ranking of exemplars by *feedforward* (raw) cosine similarity.
    pub fn feedforward_ranking(&self, query: &DVector<f64>) -> Vec<usize> {
        let a = DVector::from_fn(self.centroids.len(), |i, _| {
            cosine(query, &self.centroids[i]).max(0.0)
        });
        let mut idx: Vec<usize> = (0..self.centroids.len()).collect();
        idx.sort_by(|&i, &j| a[j].partial_cmp(&a[i]).unwrap_or(std::cmp::Ordering::Equal));
        idx
    }

    /// Fraction of queries classified to the correct prototype by raw
    /// (feedforward) nearest-centroid decoding — identity performance, which
    /// must be unaffected by lateral disruption.
    pub fn feedforward_accuracy(&self, queries: &[DVector<f64>], labels: &[usize]) -> f64 {
        assert_eq!(queries.len(), labels.len(), "query/label count mismatch");
        if queries.is_empty() {
            return 1.0;
        }
        let mut correct = 0;
        for (q, &label) in queries.iter().zip(labels) {
            let mut best = 0usize;
            let mut best_sim = f64::NEG_INFINITY;
            for (i, c) in self.centroids.iter().enumerate() {
                let s = cosine(q, c);
                if s > best_sim {
                    best_sim = s;
                    best = i;
                }
            }
            if best == label {
                correct += 1;
            }
        }
        correct as f64 / queries.len() as f64
    }

    /// Disconnect lateral edges, keeping only the strongest
    /// `keep_fraction` of them (ties broken by index).
    ///
    /// `keep_fraction = 1.0` is a no-op; `keep_fraction = 0.0` disconnects all
    /// laterals, collapsing perceived similarity back to feedforward identity.
    pub fn perturb(&mut self, keep_fraction: f64) {
        let k = self.lateral.nrows();
        let mut edges: Vec<(f64, usize, usize)> = Vec::new();
        for i in 0..k {
            for j in 0..k {
                if i != j {
                    edges.push((self.lateral[(i, j)].abs(), i, j));
                }
            }
        }
        edges.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let keep = ((edges.len() as f64) * keep_fraction.clamp(0.0, 1.0)).round() as usize;
        let mut new_lateral = DMatrix::zeros(k, k);
        for (_, i, j) in edges.iter().take(keep) {
            new_lateral[(*i, *j)] = self.lateral[(*i, *j)];
        }
        self.lateral = new_lateral;
    }
}

fn normalize(v: &DVector<f64>) -> DVector<f64> {
    let norm = v.norm();
    if norm > 1e-12 {
        v / norm
    } else {
        v.clone()
    }
}

fn cosine(a: &DVector<f64>, b: &DVector<f64>) -> f64 {
    let na = a.norm();
    let nb = b.norm();
    if na <= 1e-12 || nb <= 1e-12 {
        0.0
    } else {
        (a.dot(b) / (na * nb)).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn centroids_3() -> Vec<DVector<f64>> {
        vec![
            DVector::from_vec(vec![1.0, 0.0]),
            DVector::from_vec(vec![0.6, 0.8]),
            DVector::from_vec(vec![-1.0, 0.0]),
        ]
    }

    #[test]
    fn feedforward_ranking_matches_raw_similarity() {
        let field = LateralField::new(centroids_3());
        // Query closer to A than to B.
        let q = DVector::from_vec(vec![0.9, 0.1]);
        let ranking = field.feedforward_ranking(&q);
        assert_eq!(ranking[0], 0, "A must be the feedforward winner");
        assert_eq!(field.relaxed_ranking(&q), ranking, "no lateral => no warp");
    }

    #[test]
    fn asymmetric_lateral_warps_ranking() {
        // Strong A -> B drive, weak B -> A back-coupling: borderline queries
        // are pulled toward B by the lateral field even though A is closer.
        let centroids = centroids_3();
        let mut lateral = DMatrix::zeros(3, 3);
        lateral[(1, 0)] = 0.9; // B receives strong input from A
        lateral[(0, 1)] = 0.05; // A receives weak input from B
        let field = LateralField::with_lateral(centroids, lateral, 8);
        let q = DVector::from_vec(vec![0.9, 0.1]);
        let ff = field.feedforward_ranking(&q);
        let relaxed = field.relaxed_ranking(&q);
        assert_eq!(ff[0], 0, "feedforward must pick A");
        assert_eq!(relaxed[0], 1, "lateral recurrency must warp the winner to B");
    }

    #[test]
    fn perturb_zero_collapses_warp_to_identity() {
        let centroids = centroids_3();
        let mut lateral = DMatrix::zeros(3, 3);
        lateral[(1, 0)] = 0.9;
        lateral[(0, 1)] = 0.05;
        let mut field = LateralField::with_lateral(centroids, lateral, 8);
        let q = DVector::from_vec(vec![0.9, 0.1]);
        let before = field.relaxed_ranking(&q);
        assert_ne!(before[0], 0, "sanity: lateral warp present before");
        field.perturb(0.0);
        let after = field.relaxed_ranking(&q);
        assert_eq!(after, field.feedforward_ranking(&q), "disrupted laterals must collapse to identity");
    }

    #[test]
    fn feedforward_accuracy_unaffected_by_perturbation() {
        let centroids = centroids_3();
        let mut lateral = DMatrix::zeros(3, 3);
        lateral[(1, 0)] = 0.9;
        lateral[(0, 1)] = 0.05;
        let mut field = LateralField::with_lateral(centroids.clone(), lateral, 8);
        let queries = vec![DVector::from_vec(vec![0.9, 0.1]), centroids[2].clone()];
        let labels = vec![0, 2];
        let before = field.feedforward_accuracy(&queries, &labels);
        field.perturb(0.0);
        let after = field.feedforward_accuracy(&queries, &labels);
        assert!((before - 1.0).abs() < 1e-12);
        assert!((after - before).abs() < 1e-12, "identity decoding must persist");
    }
}
