use approx::assert_relative_eq;
use concept_pca::pca_concept_coverage;
use nalgebra::DMatrix;

/// THEOREM: embeddings idênticos → matriz centralizada zero.
#[test]
fn test_pca_degenerate_identical_embeddings() {
    let e = DMatrix::from_row_slice(3, 2, &[1.0, 0.0, 1.0, 0.0, 1.0, 0.0]);
    let concepts: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
    let r = pca_concept_coverage(&e, &concepts, 0.95).unwrap();
    assert_eq!(r.n_components, 0);
    assert_relative_eq!(r.coverage, 0.0, epsilon = 1e-12);
}

/// THEOREM: 5 pontos colineares → rank 1.
#[test]
fn test_pca_collinear_points_rank1() {
    let e = DMatrix::from_row_slice(5, 3, &[
        1.0, 0.01, 0.01,
        2.0, 0.02, 0.02,
        3.0, 0.03, 0.03,
        4.0, 0.04, 0.04,
        5.0, 0.05, 0.05,
    ]);
    let concepts: Vec<String> = (0..5).map(|i| format!("c{}", i)).collect();
    let r = pca_concept_coverage(&e, &concepts, 0.95).unwrap();
    assert_eq!(r.n_components, 1);
    assert_relative_eq!(r.coverage, 1.0, epsilon = 1e-9);
}

/// THEOREM: 2 idênticos + 1 ortogonal. X^T X = diag(4/3, 0).
#[test]
fn test_pca_high_similarity_cluster() {
    let e = DMatrix::from_row_slice(3, 2, &[
        1.0, 0.0,
        1.0, 0.0,
        0.0, 1.0,
    ]);
    let concepts: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
    let r = pca_concept_coverage(&e, &concepts, 0.95).unwrap();

    assert_eq!(r.n_components, 1);
    assert_relative_eq!(r.coverage, 1.0, epsilon = 1e-9);
    assert_relative_eq!(r.concept_similarities[0].1, 0.5, epsilon = 1e-12);
    assert_relative_eq!(r.concept_similarities[1].1, 0.5, epsilon = 1e-12);
    assert_relative_eq!(r.concept_similarities[2].1, 0.0, epsilon = 1e-12);
}

/// THEOREM: 4 pontos em R³. Autovalores de X^T X = {1/16, 1, 1}.
#[test]
fn test_pca_orthogonal_concepts_3d() {
    let e = DMatrix::from_row_slice(4, 3, &[
        1.0, 0.0, 0.0,
        0.0, 1.0, 0.0,
        0.0, 0.0, 1.0,
        0.5, 0.5, 0.5,
    ]);
    let concepts: Vec<String> = vec!["x".into(), "y".into(), "z".into(), "m".into()];
    let r = pca_concept_coverage(&e, &concepts, 0.95).unwrap();

    assert_eq!(r.n_components, 2);
    assert_relative_eq!(r.coverage, 32.0 / 33.0, epsilon = 1e-9);
}

/// THEOREM: similaridades no intervalo [0,1].
#[test]
fn test_pca_similarity_range() {
    let e = DMatrix::from_row_slice(4, 3, &[
        1.0, 0.0, 0.0,
        0.0, 1.0, 0.0,
        0.0, 0.0, 1.0,
        1.0, 1.0, 1.0,
    ]);
    let concepts: Vec<String> = (0..4).map(|i| format!("c{}", i)).collect();
    let r = pca_concept_coverage(&e, &concepts, 0.95).unwrap();
    for (_, sim) in &r.concept_similarities {
        assert!(*sim >= 0.0 && *sim <= 1.0);
    }
}