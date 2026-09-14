//! Análise espectral do DAG de tarefas.
//!
//! Correção CRÍTICA aplicada: `is_fiedler_unique` usa o gap LOCAL (λ₃-λ₂),
//! nunca a distância a λ₁. Isso evita falso-positivo de degenerescência
//! em grafos com gargalo severo mas λ₂ simples.
//!
//! SEGUNDA correção (achado #5): `unique ≠ reliable`. Unicidade matemática
//! (gap λ₃-λ₂ > tol) é necessária mas não suficiente: se λ₂ ≈ ε_mach·‖L‖, o
//! solver não separa o autoespaço de λ₂ do de λ₁=0 e devolve lixo. Por isso a
//! API só expõe `fiedler_vector` quando UNIQUE **e** RELIABLE.

use nalgebra::{DMatrix, DVector, SymmetricEigen};
use crate::error::SpectralError;

const EIGENVALUE_REL_TOL: f64 = 1e-9;
/// Fator empírico: abaixo deste múltiplo de `ε_mach · λ_max`, o Fiedler não é confiável.
/// Calibrado contra o dumbbell K₃-ε-K₃: λ₂ = (2/3)·ε e λ_max ≈ 3, logo
/// a razão λ₂/(ε_mach·λ_max) é 10^6·(ε/1e-9) — **exatamente** 1e6 para ε=1e-9.
/// Com κ=1e6 a fronteira fica em hairline (0.07% acima); κ=3e6 deixa ε=1e-9
/// 3x ABAIXO (não-confiável) e ε=1e-8 3x ACIMA (confiável).
pub const FIEDLER_RELIABILITY_KAPPA: f64 = 3e6;

#[derive(Debug, Clone)]
pub struct SpectralAnalysis {
    pub algebraic_connectivity: f64,
    pub spectral_gap: f64,
    pub lambda_max: f64,
    /// `Some(v)` **somente** se `is_fiedler_unique && is_fiedler_reliable`.
    /// Nos demais casos, retorna `None` — o chamador não pode usar um vetor
    /// potencialmente corrompido.
    pub fiedler_vector: Option<DVector<f64>>,
    pub lambda_2_multiplicity: usize,
    /// Verdadeiro se λ₂ tem multiplicidade 1 no espectro exato (gap λ₃-λ₂ > tol).
    pub is_fiedler_unique: bool,
    /// Verdadeiro se λ₂ é grande o suficiente para o vetor devolvido ser
    /// numericamente estável (λ₂ > κ · ε_mach · λ_max).
    pub is_fiedler_reliable: bool,
    /// Razão λ_max / min(λ₂-λ₁, λ₃-λ₂). Quanto maior, mais mal-condicionada
    /// a computação do vetor. `∞` se algum gap for zero.
    pub fiedler_condition: f64,
    pub n_nodes: usize,
    pub n_edges: usize,
}

pub fn build_symmetrized_laplacian(n_nodes: usize, edges: &[(usize, usize)]) -> DMatrix<f64> {
    let mut adjacency = DMatrix::<f64>::zeros(n_nodes, n_nodes);
    for &(i, j) in edges {
        if i < n_nodes && j < n_nodes && i != j {
            adjacency[(i, j)] = 1.0;
            adjacency[(j, i)] = 1.0;
        }
    }
    let degrees: DVector<f64> = adjacency.row_sum().transpose();
    let d = DMatrix::from_diagonal(&degrees);
    d - adjacency
}

pub fn build_laplacian_from_map(
    nodes: &[String],
    edges: &[(String, String)],
) -> Result<DMatrix<f64>, SpectralError> {
    if nodes.is_empty() {
        return Err(SpectralError::Empty);
    }
    let index: std::collections::HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();

    let mut edge_indices = Vec::with_capacity(edges.len());
    for (from, to) in edges {
        let i = index
            .get(from.as_str())
            .copied()
            .ok_or_else(|| SpectralError::EigenFailed(format!("Nó não encontrado: {}", from)))?;
        let j = index
            .get(to.as_str())
            .copied()
            .ok_or_else(|| SpectralError::EigenFailed(format!("Nó não encontrado: {}", to)))?;
        edge_indices.push((i, j));
    }
    if edge_indices.is_empty() {
        return Err(SpectralError::NoEdges);
    }
    Ok(build_symmetrized_laplacian(nodes.len(), &edge_indices))
}

pub fn analyze_laplacian(lap: &DMatrix<f64>) -> Result<SpectralAnalysis, SpectralError> {
    let n = lap.nrows();
    if n == 0 {
        return Err(SpectralError::Empty);
    }
    if n == 1 {
        return Ok(SpectralAnalysis {
            algebraic_connectivity: 0.0,
            spectral_gap: 0.0,
            lambda_max: 0.0,
            fiedler_vector: Some(DVector::zeros(1)),
            lambda_2_multiplicity: 1,
            is_fiedler_unique: true,
            is_fiedler_reliable: true,
            fiedler_condition: 0.0,
            n_nodes: 1,
            n_edges: 0,
        });
    }

    let eig = SymmetricEigen::new(lap.clone());
    let mut pairs: Vec<(f64, DVector<f64>)> = eig
        .eigenvalues
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, eig.eigenvectors.column(i).into_owned()))
        .collect();
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let evals: Vec<f64> = pairs.iter().map(|(v, _)| *v).collect();
    let lambda_1 = evals[0];
    let lambda_2 = evals.get(1).copied().unwrap_or(lambda_1);
    let lambda_3 = evals.get(2).copied().unwrap_or(lambda_2);
    let lambda_max = *evals.last().unwrap();

    let scale = lambda_max.abs().max(1.0);
    let tol = EIGENVALUE_REL_TOL * scale;

    // --- Unicidade matemática (gap superior) ---
    let spectral_gap = lambda_3 - lambda_2;
    let is_fiedler_unique = spectral_gap > tol;

    // --- Confiabilidade numérica (ambos os gaps) ---
    let eps_mach = f64::EPSILON;
    let numerical_noise = FIEDLER_RELIABILITY_KAPPA * eps_mach * lambda_max.abs().max(1.0);
    let is_fiedler_reliable = lambda_2 > numerical_noise;

    // --- Condicionamento do vetor: min dos dois gaps ---
    let gap_below = lambda_2 - lambda_1;
    let gap_above = spectral_gap;
    let gap_min = gap_below.min(gap_above).abs();
    let fiedler_condition = if gap_min > 0.0 {
        lambda_max.abs() / gap_min
    } else {
        f64::INFINITY
    };

    let lambda_2_multiplicity = if is_fiedler_unique {
        1
    } else {
        evals[1..]
            .iter()
            .take_while(|&&v| (v - lambda_2).abs() < tol)
            .count()
    };

    // Só expõe o vetor se for ÚNICO **e** CONFIÁVEL.
    let fiedler_vector = if is_fiedler_unique && is_fiedler_reliable {
        Some(pairs[1].1.clone())
    } else {
        None
    };

    let n_edges = (lap.trace() / 2.0).round() as usize;
    let _ = scale;

    Ok(SpectralAnalysis {
        algebraic_connectivity: lambda_2,
        spectral_gap,
        lambda_max,
        fiedler_vector,
        lambda_2_multiplicity,
        is_fiedler_unique,
        is_fiedler_reliable,
        fiedler_condition,
        n_nodes: n,
        n_edges,
    })
}

pub fn fiedler_partition(analysis: &SpectralAnalysis) -> Option<(Vec<usize>, Vec<usize>)> {
    let v = analysis.fiedler_vector.as_ref()?;
    let mut a = Vec::new();
    let mut b = Vec::new();
    for (i, &x) in v.iter().enumerate() {
        if x >= 0.0 {
            a.push(i);
        } else {
            b.push(i);
        }
    }
    Some((a, b))
}

pub fn check_dag_health(analysis: &SpectralAnalysis, threshold: f64) -> Option<String> {
    // Se λ₂ está no regime não-confiável, o alerta é diferente:
    // não é "grafo fragmentado" mas "grafo em região onde o método espectral
    // não distingue λ₂ de λ₁".
    if !analysis.is_fiedler_reliable && analysis.algebraic_connectivity > 0.0 {
        return Some(format!(
            "λ₂ = {:.4e} abaixo do limiar de confiabilidade numérica. \
             A partição espectral é indefinida neste regime; use heurística topológica.",
            analysis.algebraic_connectivity
        ));
    }
    if analysis.algebraic_connectivity < threshold {
        return Some(format!(
            "Conectividade algébrica baixa: λ₂ = {:.4e} < {:.4e}",
            analysis.algebraic_connectivity, threshold
        ));
    }
    None
}