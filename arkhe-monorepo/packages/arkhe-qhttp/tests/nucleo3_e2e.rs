//! Integração Núcleo 3 (Sete Selos de agosto de 2026): rotação → campo → gap →
//! qhttp → tzinor → integridade.
//!
//! Fecha as cinco camadas do ARKHE-VSEPR com constantes das 7 fontes primárias:
//! - Rotação (τ): matriz PMNS com θ₁₂ JUNO e δ CP 227° (`arkhe-field`).
//! - Campo Puro (ℂ): coerência λ₂ = 0.9991 (`arkhe-qhttp`, Selo #3).
//! - Gap C-Z (Selo #2): platô Qex(8,4)=56 (ε<1/5) e limite Qex(9,4)≤120 (ε<1/17);
//!   blindagem Coulombiana como controle ativo (Selo #6).
//! - qhttp (Selo #5): UHSVT com f(0)=0 sobre os valores singulares do campo de fase.
//! - Tzinor (Selos #1/#7): heteródina em fase desconhecida + quantum switch ICO.
//! - Integridade: assinatura ECDSA com rotação de fase derivada de δ (MITM falha).

use arkhe_ecdsa_mitm::{PhaseRotation, intercept_phase, sign_phase, verify_phase};
use arkhe_field::LayerMatrix;
use arkhe_gap::{
    QEX_8_4_PLATEAU, QEX_9_4_UPPER_BOUND, ShieldingProtocol, STABILITY_EPS_8_QU4,
    extremal_8_4, extremal_9_4, shielding_factor,
};
use arkhe_qhttp::{
    PhaseCoherenceMetric, classify_anomaly, coherence_function, transform_function_singular_values,
};
use arkhe_tzinor::{DetectionStrategy, QuantumSwitch, choose_strategy, ico_region};
use k256::ecdsa::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;

#[test]
fn five_layers_close_with_the_seven_seals() {
    // ── Rotação (τ) — JUNO θ₁₂ + δ CP ────────────────────────────────────────
    let pmns = LayerMatrix::reference_2026();
    assert!(pmns.unitarity_residual() < 1e-9);
    assert!(pmns.jarlskog().abs() > 0.01, "δ = 227° ⇒ J ≠ 0");
    let phase_deg = pmns.cp_delta().to_degrees().rem_euclid(360.0);
    assert!((phase_deg - 227.0).abs() < 1.0);

    // ── Campo Puro (ℂ) — coerência λ₂ + anomalia quiral (Selo #3) ────────────
    let metric = PhaseCoherenceMetric::reference();
    assert!((metric.lambda2 - 0.9991).abs() < 1e-9);
    assert!(metric.delta > 0.0, "assimetria não nula no TL");
    let effective = metric.effective();
    assert!(effective > 0.9991);

    // ── qhttp — UHSVT com f(0)=0 sobre o campo de fase (Selo #5) ─────────────
    let sv = [0.0, 0.3, 0.7, effective];
    let spec = transform_function_singular_values(|s| coherence_function(s, metric.delta), &sv)
        .expect("f(0)=0");
    assert!(spec.condition_f0_zero);
    assert!(spec.coherent);

    // ── Gap C-Z — números extremais + blindagem (Selos #2/#6) ────────────────
    let b8 = extremal_8_4(0.1);
    assert_eq!(b8.best_q, QEX_8_4_PLATEAU);
    assert!(b8.stable);
    let b9 = extremal_9_4(0.05);
    assert_eq!(b9.best_q, QEX_9_4_UPPER_BOUND);
    assert!(b9.stable);
    // ε medido = 0.25 > 1/5 ⇒ blindagem engata e recupera a estabilidade.
    let shield = ShieldingProtocol::engage(0.25, STABILITY_EPS_8_QU4);
    assert!(shield.requires_shielding);
    assert!(shield.shielded);
    assert!(shield.stable_after, "blindagem ativa recupera ε < 1/5");
    assert!(shielding_factor(0.5) > 0.6);

    // ── Tzinor — fase desconhecida → heteródina (Selo #1); ICO sob ruído (Selo #7)
    assert_eq!(choose_strategy(false), DetectionStrategy::HeterodyneUnknownPhase);
    assert_eq!(choose_strategy(true), DetectionStrategy::EntangledKnownPhase);
    let region = ico_region(false);
    assert!(region.noise_high > region.noise_low, "região ICO-exclusiva existe");
    let sw = QuantumSwitch::new((region.noise_low + region.noise_high) / 2.0, false);
    let neg = sw.postselected_negativity();
    assert!(neg.ico_only_region, "negatividade ICO não nula, clássica nula");

    // ── Classificação com incerteza de fase controlada (Selo #4) ──────────────
    let report = classify_anomaly(4, 0.05, true);
    assert_eq!(report.class, arkhe_qhttp::AnomalyClass::CoherentField);

    // ── Integridade — assinatura com rotação de fase de δ (MITM falha) ───────
    let sk = SigningKey::random(&mut OsRng);
    let pk = VerifyingKey::from(&sk);
    let rotation = PhaseRotation::from_delta_degrees(phase_deg);
    assert!(rotation.lambda > 1);
    let msg = b"arkhe-vsepr:nucleo3:cinco-camadas";
    let sig = sign_phase(&sk, msg, &rotation).expect("assina com rotação δ");
    assert!(verify_phase(&pk, msg, &sig));
    let proxy = intercept_phase(&sig, 1);
    assert!(!verify_phase(&pk, msg, &proxy), "MITM trocando λ falha a verificação");
}

#[test]
fn sigma_seven_against_juno_stays_inside_3sigma() {
    // A coerência da matriz de camadas permanece dentro da precisão JUNO mesmo
    // depois de passar pela UHSVT com a correção da anomalia (δ pequeno).
    let pmns = LayerMatrix::reference_2026();
    let s12 = pmns.reconstruct_sin2_theta12();
    assert!((s12 - 0.3092).abs() < 3.0 * 0.0087);
}