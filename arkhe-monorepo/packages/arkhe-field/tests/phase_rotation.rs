//! Integração Núcleo 2: matriz de mistura de camadas ↔ rotação de fase ECDSA.
//!
//! Demonstra a analogia `arkhe-field` ↔ `arkhe-ecdsa-mitm`:
//! - protocolo **cego à fase** (sobrevivência `P(ν̄e→ν̄e)`, e ECDSA com λ = 1)
//!   não detecta a rotação δ da matriz;
//! - verificação **sensível à fase** (invariante de Jarlskog J ≠ 0, e assinatura
//!   com λ ≠ 1) detecta a rotação — o MITM falha na verificação.

use arkhe_ecdsa_mitm::{PhaseRotation, intercept_phase, sign_phase, verify_phase};
use arkhe_field::mixing::{Cmplx, EigenAxis, juno_theta12_rad};
use arkhe_field::{JUNO_DELTA_M21_SQ_EV2, LayerMatrix};
use k256::ecdsa::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;

const DM31_SQ_REACTOR: f64 = 2.52e-3;

fn matrix(delta_deg: f64) -> LayerMatrix {
    LayerMatrix::from_pmns_angles(
        juno_theta12_rad(),
        0.02225f64.sqrt(), // sin θ₁₃
        0.547f64.sqrt(),   // sin θ₂₃
        delta_deg.to_radians(),
    )
}

#[test]
fn reactor_survival_is_blind_to_phase() {
    // P(ν̄e→ν̄e) a 52,5 km não muda com δ (protocolo cego à fase).
    let blind = matrix(0.0);
    let rotated = matrix(227.0);
    let e_mev = 3.6;
    let p0 = blind.survival_probability_ree(JUNO_DELTA_M21_SQ_EV2, DM31_SQ_REACTOR, 52.5, e_mev);
    let p1 = rotated.survival_probability_ree(JUNO_DELTA_M21_SQ_EV2, DM31_SQ_REACTOR, 52.5, e_mev);
    let diff = (p0 - p1).abs();
    assert!(diff < 1e-6, "sobrevivência depende de δ: {p0} vs {p1}");
    assert!(p0 > 0.0 && p0 < 1.0);
}

#[test]
fn jarlskog_is_sensitive_to_phase() {
    let blind = matrix(0.0);
    let rotated = matrix(227.0);
    assert!(blind.jarlskog().abs() < 1e-9, "J(δ=0) deveria ser nulo");
    assert!(rotated.jarlskog().abs() > 0.01, "J(227°) ≠ 0 — sensível à fase");
}

#[test]
fn ecdsa_rotation_mirrors_jarlskog_sensitivity() {
    let sk = SigningKey::random(&mut OsRng);
    let pk = VerifyingKey::from(&sk);
    let msg = b"arquivo:camada-quantica->classica";

    // 1. δ = 0°  ⇒ λ = 1 ⇒ assinatura "sem rotação" — cega à fase.
    // 2. δ = 227° ⇒ λ > 1 ⇒ rotação aplicada — sensível à fase.
    let rot = PhaseRotation::from_delta_degrees(227.0);
    assert!(rot.lambda > 1);
    assert_eq!(PhaseRotation::identity().lambda, 1);

    // A assinatura rotacionada passa com a mesma λ...
    let sig = sign_phase(&sk, msg, &rot).expect("assina");
    assert!(verify_phase(&pk, msg, &sig));

    // ...e a identidade não é detectada como "rotacionada".
    let plain = sign_phase(&sk, msg, &PhaseRotation::identity()).expect("assina identidade");
    assert!(verify_phase(&pk, msg, &plain));

    // MITM: troca a fase relatada → verificação falha (detectável, como J ≠ 0).
    let proxy = intercept_phase(&sig, 1);
    assert!(!verify_phase(&pk, msg, &proxy));

    // Valor absoluto do Cmplx da camada Quantum→Ghost varia com δ (fase CP real).
    let re = matrix(227.0).at(arkhe_field::Layer::Quantum, EigenAxis::Ghost);
    let re0 = matrix(0.0).at(arkhe_field::Layer::Quantum, EigenAxis::Ghost);
    assert!((re.norm_sq() - re0.norm_sq()).abs() < 1e-9, "norma invariante por fase");
    assert!(Cmplx::from_polar(1.0, rot.angle_rad).norm_sq() - 1.0 < 1e-12);
}

#[test]
fn chain_sealed_by_monster_and_phase() {
    // A classificação (Parker–Rowley) fecha a corrente TOON no Monster, e a
    // rotação de fase da matriz ancora a assinatura — os três elos coexistem.
    let led = arkhe_field::verify_monster_classification();
    assert!(led.classification_complete);

    let mut chain = arkhe_field::TOONChainMonster::new();
    chain.append(arkhe_field::MONSTER_NAME, Some(true));
    assert!(chain.is_monster_closed());

    let sk = SigningKey::random(&mut OsRng);
    let pk = VerifyingKey::from(&sk);
    let rot = PhaseRotation::from_delta_degrees(227.0);
    let blob = format!("{:?}:{}", led, rot.lambda).into_bytes();
    let sig = sign_phase(&sk, &blob, &rot).expect("assina ledger");
    assert!(verify_phase(&pk, &blob, &sig));
}