use approx::assert_relative_eq;
use spectral_core::laplacian::*;

/// THEOREM: C₄ tem espectro {0,2,2,4}; λ₂=2 é degenerado.
#[test]
fn test_c4_spectrum_and_degeneracy() {
    let lap = build_symmetrized_laplacian(4, &[(0, 1), (1, 3), (2, 3), (0, 2)]);
    let a = analyze_laplacian(&lap).unwrap();

    assert_relative_eq!(a.algebraic_connectivity, 2.0, epsilon = 1e-9);
    assert_relative_eq!(a.lambda_max, 4.0, epsilon = 1e-9);
    assert_eq!(a.lambda_2_multiplicity, 2);
    assert!(!a.is_fiedler_unique);
    assert!(a.fiedler_vector.is_none());
    assert!(fiedler_partition(&a).is_none());
}

/// THEOREM: P₄ tem λ₂ = 2-√2, único.
#[test]
fn test_p4_unique_fiedler() {
    let lap = build_symmetrized_laplacian(4, &[(0, 1), (1, 2), (2, 3)]);
    let a = analyze_laplacian(&lap).unwrap();

    let expected = 2.0 - 2.0_f64.sqrt();
    assert_relative_eq!(a.algebraic_connectivity, expected, epsilon = 1e-9);
    assert_eq!(a.lambda_2_multiplicity, 1);
    assert!(a.is_fiedler_unique);
    assert!(a.fiedler_vector.is_some());
    assert!(fiedler_partition(&a).is_some());
}

/// REGIME NÃO-CONFIÁVEL: dumbbell ε=1e-9 → λ₂ < κ·ε_mach·λ_max → None.
#[test]
fn test_severe_bottleneck_api_reflects_numerical_reality() {
    // K₃ — ε — K₃  com ε = 1e-9
    let mut edges: Vec<(usize, usize)> = Vec::new();
    edges.extend([(0, 1), (0, 2), (1, 2)]);
    edges.extend([(3, 4), (3, 5), (4, 5)]);
    let mut lap = build_symmetrized_laplacian(6, &edges);
    let eps = 1e-9_f64;
    lap[(2, 2)] += eps;
    lap[(3, 3)] += eps;
    lap[(2, 3)] -= eps;
    lap[(3, 2)] -= eps;

    let a = analyze_laplacian(&lap).unwrap();

    // λ₂ deve ser menor que o limiar de confiabilidade numérica.
    assert!(
        a.algebraic_connectivity < 1e-6,
        "λ₂ = {} deveria ser pequeno para ε=1e-9",
        a.algebraic_connectivity
    );
    assert!(a.is_fiedler_unique,
            "λ₂ é simples mas foi classificado degenerado (gap={})",
            a.spectral_gap);
    // CHAVE: is_fiedler_reliable=false → fiedler_vector deve ser None
    assert!(!a.is_fiedler_reliable,
            "λ₂ = {} não deveria ser confiável para ε=1e-9",
            a.algebraic_connectivity);
    assert!(a.fiedler_vector.is_none(),
            "Fiedler vector deve ser None quando não confiável");
    // O gap existe, mas o solver não consegue distinguir o autoespaço
    assert!(a.lambda_2_multiplicity == 1,
            "λ₂ deveria ter multiplicidade 1");
    // check_dag_health deve dar o aviso correto de não-confiabilidade
    assert!(check_dag_health(&a, 0.05).unwrap().contains("confiabilidade"),
            "Mensagem deveria mencionar confiabilidade numérica");
}

/// REGIME CONFIÁVEL: dumbbell ε=1e-6 → λ₂ > κ·ε_mach·λ_max → Some.
#[test]
fn test_bottleneck_eps_1e_6_still_reliable() {
    let mut edges: Vec<(usize, usize)> = Vec::new();
    edges.extend([(0, 1), (0, 2), (1, 2)]);
    edges.extend([(3, 4), (3, 5), (4, 5)]);
    let mut lap = build_symmetrized_laplacian(6, &edges);
    let eps = 1e-6_f64;
    lap[(2, 2)] += eps;
    lap[(3, 3)] += eps;
    lap[(2, 3)] -= eps;
    lap[(3, 2)] -= eps;

    let a = analyze_laplacian(&lap).unwrap();

    // λ₂ ainda pequeno mas acima do limiar numérico
    assert!(a.algebraic_connectivity < 1e-6,
            "λ₂ = {} deveria ser pequeno", a.algebraic_connectivity);
    assert!(a.is_fiedler_unique);
    assert!(a.is_fiedler_reliable,
            "λ₂ = {} deveria ser confiável para ε=1e-6",
            a.algebraic_connectivity);
    assert!(a.fiedler_vector.is_some(),
            "Fiedler vector deve ser Some quando único e confiável");
    // Partição deve separar as duas cliques corretamente
    let (g0, g1) = fiedler_partition(&a).unwrap();
    assert_eq!(g0.len() + g1.len(), 6);
    let left: std::collections::HashSet<usize> = [0, 1, 2].into_iter().collect();
    let right: std::collections::HashSet<usize> = [3, 4, 5].into_iter().collect();
    let got_left: std::collections::HashSet<usize> = g0.into_iter().collect();
    assert!(got_left == left || got_left == right,
            "Partição {:?} não separa as cliques", got_left);
}

/// THEOREM: grafo desconexo → λ₂ ≈ 0.
#[test]
fn test_disconnected_graph() {
    let lap = build_symmetrized_laplacian(4, &[(0, 1), (2, 3)]);
    let a = analyze_laplacian(&lap).unwrap();
    assert!(a.algebraic_connectivity.abs() < 1e-9);
}

#[test]
fn test_health_check_alerts_low_connectivity() {
    let lap = build_symmetrized_laplacian(4, &[(0, 1), (2, 3)]);
    let a = analyze_laplacian(&lap).unwrap();
    assert!(check_dag_health(&a, 0.1).is_some());
}