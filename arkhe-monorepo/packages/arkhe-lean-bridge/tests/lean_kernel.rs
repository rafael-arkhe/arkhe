//! Testes de integração da Ponte Lean: **cargo test chama o kernel Lean real
//! via FFI** (`std::process` → `lean.exe` do PATH/ARKHE_LEAN_BIN) e fecha a
//! paridade núcleo↔Rust dos critérios F1–F6.

use std::path::PathBuf;

use arkhe_lean_bridge::criteria::{BridgeEntry, FalsificationCriteria};
use arkhe_lean_bridge::{
    kernel::{digest_sha3_256, LeanKernel, ProofSource},
    criteria::CriterionId,
    NUCLEUS_INVARIANTS, NUCLEUS_LEAN_RELATIVE,
};

/// Resolve o caminho absoluto do núcleo Lean no monorepo.
fn nucleus_path() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(NUCLEUS_LEAN_RELATIVE);
    root.canonicalize().expect("núcleo Lean deve existir no monorepo")
}

#[test]
fn kernel_elaborates_bridge_nucleus_without_sorry() {
    let kernel = LeanKernel::from_env();
    let source = ProofSource::new("LeanBridgeNucleus", nucleus_path())
        .expect("leitura da fonte de prova");
    let verdict = kernel
        .verify(&source, &NUCLEUS_INVARIANTS)
        .expect("kernel Lean deve responder");
    assert!(
        verdict.is_sound(),
        "veredito não saudável: exit_ok={}, digest={}, elaboradas={:?}/{}",
        verdict.kernel_exit_ok,
        verdict.digest_matches,
        verdict.acknowledged,
        NUCLEUS_INVARIANTS.len(),
    );
    assert_eq!(verdict.acknowledged.len(), NUCLEUS_INVARIANTS.len());
    assert!(
        verdict.stderr.is_empty(),
        "stderr não-vazio indica elaboração com erro: {}",
        verdict.stderr
    );
}

#[test]
fn fingerpint_ghost1_detects_tampering() {
    let kernel = LeanKernel::from_env();
    let source = ProofSource::new("tamper-probe", nucleus_path()).unwrap();
    // Digest da fonte íntegra.
    let pristine = digest_sha3_256(&std::fs::read(&source.path).unwrap());
    assert_eq!(pristine, source.sha3_256);
    // Uma mutação (append de espaço) tem de alterar o digest Ghost-1.
    let mut tampered = std::fs::read(&source.path).unwrap();
    tampered.push(b' ');
    assert_ne!(digest_sha3_256(&tampered), source.sha3_256, "Ghost-1 quebrado");
    let _ = &kernel;
}

#[test]
fn criteria_constants_parity_with_kernel_accepted_text() {
    let text = std::fs::read_to_string(nucleus_path()).expect("núcleo legível");
    let canon = FalsificationCriteria::canonical();
    // A paridade fecha contra o TEXTO que o kernel aceitou (F6, I529):
    // alterar uma constante no núcleo ou no Rust faz destes testes falharem.
    assert!(text.contains("def floor_gap1_x1e4 : Nat := 5774"), "piso divergiu");
    assert!(text.contains("def ceiling_gap1_x1e4 : Nat := 9999"), "teto divergiu");
    assert!(text.contains("def target_phi_e9_x1e4 : Nat := 8500"), "alvo divergiu");
    assert!(text.contains("def quiet_phi_e9_x1e4 : Nat := 9800"), "quieto divergiu");
    assert!(text.contains("def collapse_phi_e4_x1e4 : Nat := 7214"), "colapso divergiu");
    assert_eq!(canon.floor_gap1_x1e4, 5774);
    assert_eq!(canon.ceiling_gap1_x1e4, 9999);
    assert_eq!(canon.target_phi_e9_x1e4, 8500);
    assert_eq!(canon.quiet_phi_e9_x1e4, 9800);
    assert_eq!(canon.collapse_phi_e4_x1e4, 7214);
}

#[test]
fn criteria_f1_f4_band_and_reward() {
    let c = FalsificationCriteria::canonical();
    let good = BridgeEntry { window: 7, ts: 100, phi_x1e4: 8500 };
    let below = BridgeEntry { window: 8, ts: 101, phi_x1e4: 5774 }; // == piso → fora
    let above_hard = BridgeEntry { window: 9, ts: 102, phi_x1e4: 10_000 }; // 1.0 → fora
    assert!(c.f1_in_band(&good));
    assert!(!c.f1_in_band(&below), "piso é exclusivo (I524)");
    assert!(!c.f1_in_band(&above_hard), "teto é inclusivo (I529)");
    assert!(c.f4_target_reward(&good));
    assert!(!c.f4_target_reward(&below));
}

#[test]
fn criteria_f2_gravity1_strict_advance() {
    let c = FalsificationCriteria::canonical();
    let g = BridgeEntry { window: 0, ts: 1, phi_x1e4: 8500 };
    let n1 = BridgeEntry { window: 1, ts: 1, phi_x1e4: 8500 }; // mesmo tick → falsificação
    let n2 = BridgeEntry { window: 2, ts: 2, phi_x1e4: 8500 };
    assert!(c.f2_strict_advance(None, &g), "génese satisfaz");
    assert!(!c.f2_strict_advance(Some(&g), &n1), "tick não avança → F2 falha");
    assert!(c.f2_strict_advance(Some(&g), &n2), "tick avança → F2 ok");
}

#[test]
fn criteria_f3_f5_f6() {
    let c = FalsificationCriteria::canonical();
    assert!(c.f3_fixed_point(8500, 8500), "ponto fixo I525");
    assert!(!c.f3_fixed_point(8500, 8499), "deriva de medição → F3 falha");
    assert!(c.f5_collapse_preserves_band(), "colapso 20% preserva banda (I528)");
    assert!(c.f6_ceiling_exclusive(9999 + 1), "acima do teto é falsificado (I529)");
    assert!(!c.f6_ceiling_exclusive(9999), "teto é inclusivo");
}

#[test]
fn criterion_mapping_plan_to_nucleus() {
    let cases = [
        (CriterionId::F1, "I601", "I524C_gclamp_monotone"),
        (CriterionId::F2, "I602", "I526_strict_walk_advances"),
        (CriterionId::F3, "I603", "I525_gclamp_fixed_point"),
        (CriterionId::F4, "I604", "I527A_target_inside_gap1"),
        (CriterionId::F5, "I605", "I528_collapse_preserves_band"),
        (CriterionId::F6, "I606", "I529A_ceiling_exclusive"),
    ];
    for (crit, plan, nucleus) in cases {
        let (p, n) = crit.mapped_invariants();
        assert_eq!(p, plan);
        assert_eq!(n, nucleus);
    }
}