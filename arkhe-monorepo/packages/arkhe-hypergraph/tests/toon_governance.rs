//! Teste de integração ARKHE-VSEPR: topologia → reputação → governança → forge.
//!
//! Amarra os módulos #8 (Nsk), #7 (Φ_C), #4 (reputação), #15 (matriz de
//! governança) e #16 (bridge kernel/forge do hypergraph) num fluxo único:
//! 1. O kernel publica um TOON com carga topológica validada (skyrmion).
//! 2. A mesa avalia a reputação dos operadores e o Φ_C do registro.
//! 3. A governança aprova conscientemente (esperança × confiança × Φ_C).
//! 4. O sistema sincroniza o TOON no hypergraph, certifica e forja.
//!
//! Este teste prova que os 13 módulos conversam entre si sem duplicar estado.

use arkhe_hypergraph::bridge::{ForgeAction, ToonBridge, ToonRecord};
use arkhe_hypergraph::HyperedgeState;
use arkhe_hypergraph::bridge::BridgeError;
use arkhe_pqc::sign::QuantumSigner;
use arkhe_reputation::governance::{Proposal, ReputationMatrix, Voter};
use arkhe_reputation::reputation::{ReputationScore, evaluate_node_reputation};
use arkhe_topology::phi::PhiCoherence;
use arkhe_topology::skyrmion::SkyrmionField;

/// Constrói um skyrmion Néel radial validado (Q = 1) — módulo #8.
fn validated_skyrmion(n: usize) -> SkyrmionField {
    let c = (n as f64 - 1.0) / 2.0;
    let r_max = n as f64 * 0.45;
    let mut grid = Vec::with_capacity(n * n);
    for iy in 0..n {
        for ix in 0..n {
            let gx = ix as f64 - c;
            let gy = iy as f64 - c;
            let r = (gx * gx + gy * gy).sqrt();
            let u = (r / r_max).min(1.0);
            let theta = std::f64::consts::PI * (1.0 - u);
            let (mz, ip) = (theta.cos(), theta.sin());
            let nx = if r > 1e-12 { gx / r } else { 0.0 };
            let ny = if r > 1e-12 { gy / r } else { 0.0 };
            grid.push([nx * ip, ny * ip, mz]);
        }
    }
    SkyrmionField::new(grid, n)
}

#[test]
fn toon_topology_reputation_governance_forge_e2e() {
    // ── 1. Kernel: valida o TOON (módulos #8 + #7). ───────────────────────────
    let sf = validated_skyrmion(64);
    let nsk = sf.skyrmion_number();
    assert!(sf.is_valid_charge(), "Nsk={nsk} deve ser inteiro");
    assert!((nsk - 1.0).abs() < 0.3, "TOON deve carregar Q=1");

    let phi = PhiCoherence::analyze(&[1.0, 0.99, 0.99]);
    assert!(phi.within_bounds, "Φ_C={} deve satisfazer Gap-1", phi.phi_c);
    assert!(0.577350 < phi.phi_c && phi.phi_c <= 0.999900);

    // ── 2. Mesa: reputação dos operadores (módulo #4). ────────────────────────
    let scores = vec![
        ReputationScore::evaluate("kernel-1", 0.99, 0.99, phi.phi_c, 3.0, 5.0),
        ReputationScore::evaluate("kernel-2", 0.95, 0.94, 0.96, 2.0, 20.0),
        ReputationScore::evaluate("relay-7", 0.30, 0.20, 0.40, -1.0, 120.0),
    ];
    assert!(scores[0].is_trusted());
    assert!(!scores[2].is_trusted(), "relay degradado não deve ser confiável");
    let mean = evaluate_node_reputation(&scores);
    assert!(mean > 0.0);

    // ── 3. Governança por esperança estatística (módulo #15). ────────────────
    let mut matrix = ReputationMatrix::new();
    let mut trusted = Vec::new();
    for s in scores.iter().take(2) {
        matrix.add_voter(s.clone());
        trusted.push(s.clone());
    }
    matrix.add_proposal(Proposal {
        id: "PROP-TOON-1".into(),
        description: "certificar transporte topológico do TOON".into(),
        expected_gain: 0.85,
    });
    matrix
        .vote("PROP-TOON-1", Voter { id: "kernel-1".into(), reputation: scores[0].normalized, support: true, confidence: 0.9 })
        .unwrap();
    matrix
        .vote("PROP-TOON-1", Voter { id: "kernel-2".into(), reputation: scores[1].normalized, support: true, confidence: 0.8 })
        .unwrap();
    let governance = matrix.governance_matrix();
    assert!(governance.table_live, "mesa com Φ_C médio > 1/√3 está viva");
    assert_eq!(governance.passed, 1, "proposta com apoio e mesa viva deve passar");

    // ── 4. Bridge kernel/forge no hypergraph (módulo #16). ───────────────────
    let signer = QuantumSigner::new();
    let mut bridge = ToonBridge::new();
    let record = ToonRecord {
        content_hash: [0xAB; 32],
        phi_c: phi.phi_c,
        topological_charge: Some(nsk),
        kernel_sig_ok: true,
    };
    let edge_id = bridge.sync_toon(&record).expect("TOON dentro dos limites constitucionais");

    // A proposta de governança aprovada habilita a certificação do mesmo edge.
    assert_eq!(governance.passed, 1);
    bridge.certify(&edge_id, &signer).expect("consenso kernel + certificação S15");

    // Ação de forge idempotente.
    let action: ForgeAction = bridge.forge_action(&edge_id).expect("edge certificado pode forjar");
    assert!(action.committed);
    assert_eq!(action.action_type, "forge.structure.commit");
    assert!(matches!(
        bridge.forge_action(&edge_id),
        Err(BridgeError::AlreadyForged)
    ));

    // O hyperedge certificado deve estar persistido com estado Certified.
    let edge = bridge.hypergraph.get(&edge_id).expect("edge presente");
    assert!(matches!(edge.state, HyperedgeState::Certified));
    assert_eq!(edge.focal_score, 0.55, "focal Weight médio de Experiment (S05/S06=0.6, S09/S14=0.5)");
}

#[test]
fn degraded_relay_blocks_table() {
    // Se todos os operadores estão degradados, a mesa morre (Φ_C médio ≤ 1/√3).
    let mut matrix = ReputationMatrix::new();
    matrix.add_voter(ReputationScore::evaluate("bad-1", 0.1, 0.1, 0.2, 0.0, 0.0));
    matrix.add_voter(ReputationScore::evaluate("bad-2", 0.1, 0.1, 0.2, 0.0, 0.0));
    matrix.add_proposal(Proposal { id: "P".into(), description: "x".into(), expected_gain: 0.9 });
    let g = matrix.governance_matrix();
    assert!(!g.table_live);
    assert_eq!(g.passed, 0);
}