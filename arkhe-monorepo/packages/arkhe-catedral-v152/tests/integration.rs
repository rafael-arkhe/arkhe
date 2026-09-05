//! Teste de integração end-to-end dos módulos do v152.0.

use arkhe_catedral_v152::core::entropy::constructive_entropy;
use arkhe_catedral_v152::core::spectral::{
    compute_anti_flatness, extract_spectral_handover, godel_threshold, irreducible_coherence,
    is_godel_threshold_reached,
};
use arkhe_catedral_v152::hardware::framer::frame;
use arkhe_catedral_v152::hardware::undulator::{AdaptiveParams, RangingDelta, UndulatorNode};
use arkhe_catedral_v152::network::ipfs_sync::{
    DistributedLedger, InMemoryIpfs, SyncHandover,
};
use arkhe_catedral_v152::verification::lean_runtime::LeanVerifier;
use std::time::Duration;

/// I157–I160: estrutura espectral coerente ao cruzar o limiar de Gödel.
#[test]
fn spectral_crosses_godel_threshold() {
    let spectrum = vec![0.6, 0.6, 0.6, 0.6, 0.6];
    let phi_irr = irreducible_coherence(spectrum.len());
    let phi_crit = godel_threshold(spectrum.len());

    assert!(phi_crit > 0.0);
    assert!(phi_irr > phi_crit);
    assert!(is_godel_threshold_reached(&spectrum));
    assert!(compute_anti_flatness(&spectrum, 1.5, 100) >= 0.0);
}

/// I167: entropia construtiva finita e positiva para um espectro denso.
#[test]
fn constructive_entropy_positive() {
    let spectrum = vec![0.3, 0.3, 0.4];
    let entropy = constructive_entropy(&spectrum, 8, 4, 1.5);
    assert!(entropy.is_finite());
    assert!(entropy > 0.0);
}

/// I194–I198: Zeno Veto + decaimento + adaptação em sequência.
#[test]
fn undulator_full_cycle() {
    let mut u = UndulatorNode::new(RangingDelta(1000)).with_adaptive(AdaptiveParams::default());

    // Handover inicial (aceito, decaído ~1).
    std::thread::sleep(Duration::from_micros(200));
    let h1 = u.receive_handover(0.95, "TCameraS3").expect("first must pass");
    assert!(h1.decayed_phi <= h1.phi);

    // Estimula a proximidade de timeouts → tx de decaimento adapta.
    std::thread::sleep(Duration::from_micros(500));

    let stats = u.stats();
    assert!(stats.handover_count >= 1);
    assert!(stats.decay_rate > 0.0);
}

/// G1 + G11: framing e fallback da bridge.
#[test]
fn bridge_frame_and_fallback() {
    let framed = frame(b"Z1T_INFER");
    assert!(framed.len() >= 6);

    let mut bridge = arkhe_catedral_v152::hardware::z1t::Z1TBridge::<
        arkhe_catedral_v152::hardware::z1t::SimulatedTransport,
    >::new();
    assert!(bridge.connect());
    let resp = bridge.send_command(b"Z1T_INFER").expect("fallback returns");
    assert!(!resp.is_empty());
}

/// I201: ledpersist distributed — publish + pull + validação cruzada.
#[test]
fn distributed_ledger_roundtrip() {
    let ledger = DistributedLedger::new(InMemoryIpfs::new(), "LZ", RangingDelta(1000));
    let h = SyncHandover::new(0.9, "Astra", 1_700_000_000, "Astra", 1000, 0.001);
    let cid = ledger.publish(h).unwrap();
    let pulled = ledger.pull_and_validate(&cid).unwrap();
    assert!(pulled.cross_validated);
}

/// I200: verificação Lean embarcada de todos os invariantes.
#[test]
fn lean_runtime_all_pass() {
    let v = LeanVerifier::new();
    let report = v.verify_all(
        &[0.25, 0.25, 0.25, 0.25],
        16,
        8,
        0.7,
        0.3,
        0.01,
        500,
        1000,
        0.95,
        0.96,
        1e-15,
    );
    assert!(report.all_verified(), "report: {:?}", report);
}

/// Fluxo de percepção completo: espectro → handover espectral.
#[test]
fn spectral_handover_flow() {
    let spectrum = vec![0.4, 0.4, 0.2];
    let handover = extract_spectral_handover(&spectrum, "TCameraS3");
    assert_eq!(handover.source, "TCameraS3");
    assert!(handover.phi_irr > 0.0);
    assert!(handover.phi_crit > 0.0);
}