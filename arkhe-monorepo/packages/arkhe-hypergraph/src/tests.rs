//! Tests for the hypergraph core and governance.

use crate::governance::{AnubisVeto, AotbRegistry};
use crate::{
    AgentId, HyperedgeError, HyperedgeState, HyperedgeType, PortalGunHypergraph,
    ScientificPayload,
};
use arkhe_pqc::sign::QuantumSigner;

#[test]
fn propose_and_sign_consensus() {
    let mut hg = PortalGunHypergraph::new();
    for a in [
        AgentId::S01, AgentId::S03, AgentId::S04, AgentId::S06,
    ] {
        hg.register(a);
    }
    let id = hg.propose(
        HyperedgeType::Casimir,
        ScientificPayload::Hypothesis { text: "Casimir wormhole valid".into(), proposer: AgentId::S01 },
    );
    assert!(!hg.check_consensus(&id), "no signatures yet");

    let signer_s01 = QuantumSigner::new();
    let signer_s03 = QuantumSigner::new();
    let signer_s04 = QuantumSigner::new();
    let signer_s06 = QuantumSigner::new();
    assert!(hg.sign(&id, &signer_s01, AgentId::S01));
    assert!(hg.sign(&id, &signer_s03, AgentId::S03));
    assert!(hg.sign(&id, &signer_s04, AgentId::S04));
    assert!(!hg.check_consensus(&id), "still missing S06");
    assert!(hg.sign(&id, &signer_s06, AgentId::S06));
    assert!(hg.check_consensus(&id), "full consensus reached");
}

#[test]
fn non_member_cannot_sign() {
    let mut hg = PortalGunHypergraph::new();
    for a in [AgentId::S05, AgentId::S06, AgentId::S09, AgentId::S14] {
        hg.register(a);
    }
    let id = hg.propose(HyperedgeType::Experiment, ScientificPayload::SimulationResult {
        fidelity: 0.9,
        data_hash: [0u8; 32],
    });
    // S01 is not a member of the Experiment edge, so it must be rejected.
    assert!(!hg.sign(&id, &QuantumSigner::new(), AgentId::S01));
}

#[test]
fn only_s07_can_veto() {
    let mut hg = PortalGunHypergraph::new();
    let id = hg.propose(HyperedgeType::Safety, ScientificPayload::VetoRequest {
        risk_score: 0.9,
        justification: "high risk".into(),
    });
    // S04 (non-voter) must not be able to veto.
    assert!(matches!(
        hg.transition(&id, HyperedgeState::Vetoed { reason: "x".into(), agent: AgentId::S07 }, AgentId::S04),
        Err(HyperedgeError::Guardrail(_))
    ));
    // S07 can.
    assert!(hg
        .transition(&id, HyperedgeState::Vetoed { reason: "anubis".into(), agent: AgentId::S07 }, AgentId::S07)
        .is_ok());
}

#[test]
fn certify_requires_consensus_and_s15() {
    let mut hg = PortalGunHypergraph::new();
    for a in [AgentId::S04, AgentId::S07, AgentId::S15] {
        hg.register(a);
    }
    let id = hg.propose(HyperedgeType::Safety, ScientificPayload::Certificate { aotb_hash: [0u8; 32] });
    // Attempt certification by S04 (not S15) must fail.
    assert!(matches!(
        hg.transition(&id, HyperedgeState::Certified, AgentId::S04),
        Err(HyperedgeError::Guardrail(_))
    ));
    // Even by S07/S15, without consensus it fails.
    assert!(hg.sign(&id, &QuantumSigner::new(), AgentId::S04));
    assert!(hg.sign(&id, &QuantumSigner::new(), AgentId::S07));
    assert!(matches!(
        hg.transition(&id, HyperedgeState::Certified, AgentId::S15),
        Err(HyperedgeError::MissingConsensus)
    ));
    // Final membership signature (S15) permits certification.
    assert!(hg.sign(&id, &QuantumSigner::new(), AgentId::S15));
    assert!(hg.check_consensus(&id));
    assert!(hg.transition(&id, HyperedgeState::Certified, AgentId::S15).is_ok());
}

#[test]
fn formalizing_requires_s04_signature() {
    let mut hg = PortalGunHypergraph::new();
    let id = hg.propose(HyperedgeType::Safety, ScientificPayload::LeanProof {
        theorem: "t".into(),
        proof_hash: [0u8; 32],
    });
    assert!(matches!(
        hg.transition(&id, HyperedgeState::Formalizing, AgentId::S04),
        Err(HyperedgeError::Guardrail(_))
    ), "S04 must have signed before formalizing");
    hg.sign(&id, &QuantumSigner::new(), AgentId::S04);
    assert!(hg.transition(&id, HyperedgeState::Formalizing, AgentId::S04).is_ok());
}

#[test]
fn anubis_veto_gates_risky_edges() {
    let mut hg = PortalGunHypergraph::new();
    let id = hg.propose(
        HyperedgeType::Experiment,
        ScientificPayload::SimulationResult { fidelity: 0.05, data_hash: [0u8; 32] },
    );
    let edge = hg.get(&id).unwrap();

    let mut anubis = AnubisVeto::new(0.9);
    // high payload risk pushes the total over the threshold.
    let res = anubis.evaluate(edge, AnubisVeto::payload_risk(&edge.payload));
    assert!(res.veto, "high-risk payload must be vetoed");
    assert_eq!(anubis.vetoes.len(), 1);
}

#[test]
fn aotb_certifies_only_certified_edges() {
    let mut hg = PortalGunHypergraph::new();
    let id = hg.propose(HyperedgeType::Safety, ScientificPayload::Hypothesis {
        text: "review".into(),
        proposer: AgentId::S04,
    });
    let edge = &mut *hg.get_mut(&id).unwrap();
    edge.state = HyperedgeState::Certified;
    let mut aotb = AotbRegistry::new();
    let cert = aotb.certify(edge, 42).expect("certified edge is certifiable");
    assert_eq!(cert.block_height, 42);
    assert_eq!(cert.certifier, AgentId::S15);
    let h = cert.aotb_hash;
    assert_eq!(h, aotb.get(&h).unwrap().aotb_hash);

    // Non-certified must not certify.
    let m = hg.propose(HyperedgeType::Casimir, ScientificPayload::Hypothesis {
        text: "x".into(),
        proposer: AgentId::S01,
    });
    let me = hg.get(&m).unwrap();
    assert!(aotb.certify(me, 43).is_none());
}