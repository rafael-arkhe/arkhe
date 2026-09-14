//! Cobertura conceitual via PCA sobre embeddings.

use nalgebra::DMatrix;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptCoverage {
    pub explained_variance_ratio: f64,
    pub n_components: usize,
    pub coverage: f64,
    pub concept_similarities: Vec<(String, f64)>,
}

pub fn pca_concept_coverage(
    embeddings: &DMatrix<f64>,
    concepts: &[String],
    variance_threshold: f64,
) -> Result<ConceptCoverage, String> {
    if embeddings.nrows() != concepts.len() {
        return Err(format!(
            "Mismatch: {} embeddings vs {} conceitos",
            embeddings.nrows(),
            concepts.len()
        ));
    }
    if embeddings.nrows() == 0 {
        return Err("Embeddings vazios".into());
    }

    // Centralizar por linha (média por característica = média amostral).
    // `row_mean()` já retorna a linha 1×D; subtrair diretamente de cada linha.
    let mean_row = embeddings.row_mean();
    let mut centered = embeddings.clone();
    for mut row in centered.row_iter_mut() {
        row -= &mean_row;
    }

    let svd = centered.svd(true, true);
    let sv = &svd.singular_values;

    let total_var: f64 = sv.iter().map(|s| s * s).sum();
    if total_var < 1e-12 {
        return Ok(ConceptCoverage {
            explained_variance_ratio: 0.0,
            n_components: 0,
            coverage: 0.0,
            concept_similarities: concepts.iter().map(|c| (c.clone(), 0.0)).collect(),
        });
    }

    let mut cumulative = 0.0;
    let mut n_components = 0;
    for s in sv.iter() {
        cumulative += s * s;
        n_components += 1;
        if cumulative / total_var >= variance_threshold {
            break;
        }
    }

    let coverage = cumulative / total_var;

    // Similaridades cosseno sobre embeddings originais
    let mut concept_similarities = Vec::with_capacity(concepts.len());
    for i in 0..concepts.len() {
        let vi = embeddings.row(i);
        let ni = vi.norm();
        let mut sum_sim = 0.0;
        let mut count = 0;
        for j in 0..concepts.len() {
            if i == j {
                continue;
            }
            let vj = embeddings.row(j);
            let nj = vj.norm();
            if ni > 1e-12 && nj > 1e-12 {
                let dot = (vi * vj.transpose())[(0, 0)];
                sum_sim += (dot / (ni * nj)).max(0.0);
                count += 1;
            }
        }
        let avg = if count > 0 { sum_sim / count as f64 } else { 0.0 };
        concept_similarities.push((concepts[i].clone(), avg));
    }

    Ok(ConceptCoverage {
        explained_variance_ratio: coverage,
        n_components,
        coverage,
        concept_similarities,
    })
}