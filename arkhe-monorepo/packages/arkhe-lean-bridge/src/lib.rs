//! # ARKHE Lean Bridge — Ponte Lean↔Rust (v494.1 real, bloco de registo proposto)
//!
//! Verificação formal com o **kernel Lean 4.33.1 como único ponto de
//! confiança**. A ponte não interpreta provas: invoca o binário do kernel
//! (`lean` no `PATH`, ou `ARKHE_LEAN_BIN`) sobre o núcleo
//! [`LeanBridgeNucleus`] (invariantes I524–I529) e confirma, de forma
//! *forte*, três factos:
//!
//! 1. **exit 0** — o kernel elaborou o ficheiro sem `sorry` (dívida formal
//!    zero);
//! 2. **nomes elaborados** — cada invariante esperada apareceu no `#check`
//!    (o kernel viu cada declaração);
//! 3. **fingerprint SHA3-256** (Ghost-1/`manifest.sha3`) da fonte — a prova
//!    verificada é exatamente a assinada.
//!
//! Os critérios de falsificação F1–F6 ([`crate::criteria`]) mapeiam o plano
//! v494.1 (I601–I606 do ledger paralelo 'clareira') para os IDs reais
//! I524–I529 provados no núcleo. Documentação de mapeamento e honestidade em
//! [`docs/e9-falsification-criteria.md`](https://arkhe.invalid/docs).

pub mod criteria;
pub mod kernel;

pub use criteria::{BridgeEntry, CriterionId, FalsificationCriteria};
pub use kernel::{KernelVerdict, LeanKernel, ProofSource, VerifyError};

/// Nomes exatos das invariantes do núcleo (ordem do `#check` em
/// `src/lean/LeanBridgeNucleus.lean` — também consumido pelo teste de FFI).
pub const NUCLEUS_INVARIANTS: [&str; 10] = [
    "I524A_gclamp_ge_lo",
    "I524B_gclamp_le_hi",
    "I524C_gclamp_monotone",
    "I525_gclamp_fixed_point",
    "I526_strict_walk_advances",
    "I527A_target_inside_gap1",
    "I527B_quiet_inside_gap1",
    "I528_collapse_preserves_band",
    "I529A_ceiling_exclusive",
    "I529B_outside_after_ceiling_transitive",
];

/// Caminho canónico do núcleo Lean relativo à raiz do monorepo.
pub const NUCLEUS_LEAN_RELATIVE: &str = "src/lean/LeanBridgeNucleus.lean";

/// Escala inteira declarada no núcleo (decimais de Φ e alvos: ×10⁴).
pub const PHI_SCALE_X1E4: u64 = 10_000;