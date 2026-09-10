//! Testes de integração do Gate (bloco 1010, v390.1): o FFI fecha a paridade
//! núcleo↔Rust dos invariantes I534/I535 — **cargo test chama o kernel Lean
//! real** (`std::process` → `lean.exe` do PATH/ARKHE_LEAN_BIN) sobre
//! `src/lean/OrchestratorGateNucleus.lean`.

use std::path::PathBuf;

use arkhe_lean_bridge::kernel::{KernelVerdict, LeanKernel, ProofSource};

use arkhe_orchestrator_gate::gate::GateVerdict;
use arkhe_orchestrator_gate::{
    evaluate, allowed, GATE_NUCLEUS_INVARIANTS, GATE_NUCLEUS_LEAN_RELATIVE,
};

/// Resolve o caminho absoluto do núcleo Lean do gate no monorepo.
fn nucleus_path() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(GATE_NUCLEUS_LEAN_RELATIVE);
    root.canonicalize()
        .expect("núcleo Lean do gate deve existir no monorepo")
}

#[test]
fn kernel_elaborates_orchestrator_gate_nucleus_without_sorry() {
    let kernel = LeanKernel::from_env();
    let source = ProofSource::new("OrchestratorGateNucleus", nucleus_path())
        .expect("leitura da fonte de prova");
    let verdict = kernel
        .verify(&source, &GATE_NUCLEUS_INVARIANTS)
        .expect("kernel Lean deve responder");
    assert!(
        verdict.is_sound(),
        "veredito não saudável: exit_ok={}, digest={}, elaboradas={:?}/{}",
        verdict.kernel_exit_ok,
        verdict.digest_matches,
        verdict.acknowledged,
        GATE_NUCLEUS_INVARIANTS.len(),
    );
    assert_eq!(verdict.acknowledged.len(), GATE_NUCLEUS_INVARIANTS.len());
    assert!(
        verdict.stderr.is_empty(),
        "stderr não-vazio indica elaboração com erro: {}",
        verdict.stderr
    );
}

#[test]
fn gate_constants_parity_with_kernel_accepted_text() {
    // A paridade fecha contra o TEXTO que o kernel aceitou (mesma convenção do
    // teste `criteria_constants_parity_with_kernel_accepted_text` da ponte):
    // alterar uma constante no núcleo ou no Rust faz deste teste falhar.
    let text = std::fs::read_to_string(nucleus_path()).expect("núcleo legível");
    assert!(text.contains("def scale_x1e4 : Nat := 10000"), "escala divergiu");
    assert!(text.contains("def floor_gap1_x1e4 : Nat := 5774"), "piso divergiu");
    assert!(text.contains("def ceiling_gap1_x1e4 : Nat := 9999"), "teto divergiu");
    use arkhe_orchestrator_gate::gate::{
        CEILING_GAP1_X1E4, FLOOR_GAP1_X1E4, SCALE_X1E4,
    };
    assert_eq!((SCALE_X1E4, FLOOR_GAP1_X1E4, CEILING_GAP1_X1E4), (10_000, 5774, 9999));
}

#[test]
fn kernel_sound_maps_to_verified_and_allows_in_band() {
    // Um `KernelVerdict` sintetizado com todos os requisitos de
    // `is_sound()` → `verified` → autoriza Φ dentro da banda (I534-D).
    let sound = KernelVerdict {
        name: "probe",
        digest_matches: true,
        kernel_exit_ok: true,
        acknowledged: vec!["I534A_gate_never_accepts_rejected".to_string()],
        stdout: "I534A_gate_never_accepts_rejected".to_string(),
        stderr: String::new(),
    };
    let v = GateVerdict::from_kernel_sound(sound.is_sound());
    assert_eq!(v, GateVerdict::Verified);
    assert!(allowed(v, 8500));
    assert!(evaluate(v, 8500).allowed());

    // Veredito não-saudável (falta o acknowledge no stdout) → nunca autoriza,
    // mesmo dentro da banda (I534-A).
    let broken = KernelVerdict {
        name: "probe",
        digest_matches: true,
        kernel_exit_ok: true,
        acknowledged: vec!["I534A_gate_never_accepts_rejected".to_string()],
        stdout: "outra coisa".to_string(),
        stderr: String::new(),
    };
    let rejected = GateVerdict::from_kernel_sound(broken.is_sound());
    assert_eq!(rejected, GateVerdict::Rejected);
    assert!(!allowed(rejected, 8500));
    assert!(!evaluate(rejected, 8500).allowed());
}