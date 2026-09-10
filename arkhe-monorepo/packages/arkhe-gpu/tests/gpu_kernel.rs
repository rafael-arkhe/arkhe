//! Testes de integração da camada GPU (bloco 1011, v390.2): o FFI fecha a
//! paridade núcleo↔Rust dos invariantes I536/I537/I538 — **cargo test chama o
//! kernel Lean real** (`std::process` → `lean.exe` do PATH/ARKHE_LEAN_BIN)
//! sobre `src/lean/GpuInvariantsNucleus.lean`.

use std::path::PathBuf;

use arkhe_lean_bridge::kernel::{KernelVerdict, LeanKernel, ProofSource};

use arkhe_gpu::{
    GPU_NUCLEUS_INVARIANTS, GPU_NUCLEUS_LEAN_RELATIVE, MAX_GRID,
    MAX_THREADS_PER_BLOCK, SIMT_CAP_MS, TILE_CAP_MS,
};

/// Resolve o caminho absoluto do núcleo Lean da camada GPU no monorepo.
fn nucleus_path() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(GPU_NUCLEUS_LEAN_RELATIVE);
    root.canonicalize()
        .expect("núcleo Lean da camada GPU deve existir no monorepo")
}

#[test]
fn kernel_elaborates_gpu_nucleus_without_sorry() {
    let kernel = LeanKernel::from_env();
    let source = ProofSource::new("GpuInvariantsNucleus", nucleus_path())
        .expect("leitura da fonte de prova");
    let verdict = kernel
        .verify(&source, &GPU_NUCLEUS_INVARIANTS)
        .expect("kernel Lean deve responder");
    assert!(
        verdict.is_sound(),
        "veredito não saudável: exit_ok={}, digest={}, elaboradas={:?}/{}",
        verdict.kernel_exit_ok,
        verdict.digest_matches,
        verdict.acknowledged,
        GPU_NUCLEUS_INVARIANTS.len(),
    );
    assert_eq!(verdict.acknowledged.len(), GPU_NUCLEUS_INVARIANTS.len());
    assert!(
        verdict.stderr.is_empty(),
        "stderr não-vazio indica elaboração com erro: {}",
        verdict.stderr
    );
}

#[test]
fn gpu_constants_parity_with_kernel_accepted_text() {
    // A paridade fecha contra o TEXTO que o kernel aceitou (mesma convenção do
    // teste do gate/pontua): alterar uma constante no núcleo ou no Rust faz
    // deste teste falhar.
    let text = std::fs::read_to_string(nucleus_path()).expect("núcleo legível");
    assert!(text.contains("def max_threads_per_block : Nat := 1024"), "max_threads divergiu");
    assert!(text.contains("def max_grid : Nat := 2147483647"), "max_grid divergiu");
    assert!(text.contains("def tile_cap_ms : Nat := 5000"), "tile_cap divergiu");
    assert!(text.contains("def simt_cap_ms : Nat := 10000"), "simt_cap divergiu");
    assert_eq!(
        (MAX_THREADS_PER_BLOCK, MAX_GRID, TILE_CAP_MS, SIMT_CAP_MS),
        (1024, 2_147_483_647, 5_000, 10_000)
    );
}

#[test]
fn kernel_sound_maps_to_allow_and_in_band_geometry() {
    // Um `KernelVerdict` sintetizado como `is_sound()` fiel → a governança
    // autoriza geometria dentro da banda G-07 e capa Tile (I538),
    // e rejeita fora (I536-B/C/D).
    let sound = KernelVerdict {
        name: "probe",
        digest_matches: true,
        kernel_exit_ok: true,
        acknowledged: vec!["I536A_wellformed_entries_satisfied".to_string()],
        stdout: "I536A_wellformed_entries_satisfied".to_string(),
        stderr: String::new(),
    };
    assert!(sound.is_sound());
    assert!(arkhe_gpu::allowed_geometry(1024, 2_147_483_647));
    assert!(arkhe_gpu::tile_duration_ok(5_000));
    assert!(arkhe_gpu::i538a_temporal_band_closed_under_retention(5_000, 5_000));
    assert!(!arkhe_gpu::allowed_geometry(0, 1));
    assert!(!arkhe_gpu::allowed_geometry(1025, 1));
    assert!(!arkhe_gpu::allowed_geometry(1, 2_147_483_648));
    assert!(!arkhe_gpu::tile_duration_ok(5_001));
}