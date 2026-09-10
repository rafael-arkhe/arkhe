//! # ARKHE Orchestrator Gate — Gate mínimo real (bloco 1010, v390.1)
//!
//! Materialização do **gate mínimo de orquestração** decidido após auditoria
//! dos documentos v514.0 (bloco proposto 1018, orquestração agêntica) e v510.1
//! (bloco proposto 1044, integração ML):
//!
//! * v514.0 propunha «I533 — Orquestração de Estado» — **ID colidido** (I533 já
//!   é real: Loopseal aelíclicidade). Este crate usa **I534/I535**.
//! * v514.0 usava `import Mathlib` e caminhos `crates/…` inexistentes. O núcleo
//!   deste crate é **core Lean 4.33.1 sem Mathlib** (`src/lean/OrchestratorGateNucleus.lean`)
//!   sobre a semântica real da ponte e do crate de coerência.
//! * v514.0 declarava «TEOREM_VERIFICADO» sem executar kernel. Aqui o veredito
//!   é sempre do kernel real via [`kernel::LeanKernel`] (teste FFI).
//!
//! ## Âncoras reais
//!
//! * **`arkhe-lean-bridge`** — `KernelVerdict::is_sound()` (digest SHA3-256 da
//!   fonte + kernel exit 0 + `#check` elaborado) é o único admitido como
//!   veredito `verified`.
//! * **`arkhe-field-stability`** — Φ canônico quadrático (`phi_default`,
//!   I511–I516) e banda Gap-1; `decay_scaled` (I530) para a composição da
//!   retenção (I534-E); `CoherenceLedger` (SHA3-256/Gravity-1) como trilha
//!   auditável onde a recompensa/avanço é registrada **só** quando autorizado.
//!
//! ## Invariantes
//!
//! * **I534** — recompensa/autorização de passo **só** com veredito `verified`
//!   (A) **e** Φ dentro da banda conservadora `(5774, 9999]` (B/C);
//!   autorizado ⟹ verified (D); a banda fecha-se sob retenção `k ≤ teto`
//!   composta com o decaimento real I530 (E).
//! * **I535-A** — contenção contabilística: num transcripto bem-formado (todo
//!   commit exige `verified`) o n.º de commits nunca excede o de provas.
//!
//! Prova formal: `src/lean/OrchestratorGateNucleus.lean` (6 teoremas
//! `I534A..E` + `I535A`), kernel exit 0 — verificado neste crate pelo teste de
//! integração FFI. Zero `unsafe`; zero dependências externas novas.

pub mod gate;

pub use gate::{
    allowed, commitments, evaluate, is_well_formed, phi_to_x1e4, proofs, reject_message,
    retained_after, GateAudit, GateDecision, GateVerdict, RejectReason, TranscriptStep,
    CEILING_GAP1_X1E4, FLOOR_GAP1_X1E4, SCALE_X1E4, i535a_commits_le_proofs,
};

/// Nomes exatos das invariantes do gate (ordem do `#check` no núcleo
/// `src/lean/OrchestratorGateNucleus.lean` — consumido pelo teste FFI).
pub const GATE_NUCLEUS_INVARIANTS: [&str; 6] = [
    "I534A_gate_never_accepts_rejected",
    "I534B_below_floor_blocked",
    "I534C_above_ceiling_blocked",
    "I534D_authorized_implies_verified",
    "I534E_band_closed_under_retention",
    "I535A_commits_le_proofs",
];

/// Caminho canónico do núcleo Lean relativo à raiz do monorepo.
pub const GATE_NUCLEUS_LEAN_RELATIVE: &str = "src/lean/OrchestratorGateNucleus.lean";

/// Escala inteira ×10⁴ declarada no núcleo (1.0).
pub const GATE_PHI_SCALE_X1E4: u64 = 10_000;