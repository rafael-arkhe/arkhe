//! # ARKHE-GPU — Camada GPU da Catedral (bloco 1011, v390.2)
//!
//! Materialização da **camada GPU real** decidida após auditoria dos pareceres
//! v520.0/v521.0/v605.0:
//!
//! * v520.0/v521.0 citavam «implementação v520.0, bloco 1045» e invariantes
//!   I647–I650 com provas `by rfl` — **substrato inexistente** (sem bloco_1045,
//!   sem crate `arkhe-gpu`, sem API lazy documentada no `cutile` real). Este
//!   crate usa os IDs **livres I536/I537/I538** (I500–I535 já em uso na
//!   numeração real) e um núcleo core Lean sem Mathlib.
//! * v605.0 usava `cutile-rs` como nome de dependência e «bloco 7» — o nome do
//!   crate no crates.io é **`cutile`** e o próximo bloco do ledger registado é
//!   **1011**, não «bloco 7».
//! * O mock proposto em v605.0 usava `tensors: HashMap` + `self.tensors.lock()`
//!   (mutabilidade interior sem `Mutex`) — aqui o [`MockGpuBackend`] usa
//!   `Mutex<HashMap<…>>` real.
//!
//! ## O que este crate faz (honesto)
//!
//! ### Governança ([`governance`]) — banda de lançamento G-07
//!
//! * a geometria só é autorizada com `0 < threads ≤ 1024` e `0 < grid ≤ 2³¹−1`;
//! * as durações são capadas por track: **Tile 5 s**, **SIMT 10 s**
//!   (respostas do Prolog `gpu_rules.pl`);
//! * o fecho da banda sob retenção (I538-A): uma duração aceite na capa Tile
//!   composta com uma retenção `k ≤ 5 s` permanece dentro da capa SIMT.
//!
//! ### Auditoria ([`audit`]) — contenção contabilística I536/I537
//!
//! * a trilha é append-only e **só regista lançamentos com contrato
//!   satisfeito** (I536-A: trilha bem-formada ⟹ todo registo satisfez);
//! * `violações ≤ lançamentos` (I537-A) e trilha bem-formada ⟹ zero violações
//!   (I537-B) — as mesmas propriedades do núcleo Lean, fechadas por teste FFI.
//!
//! ### Backends ([`backend`], [`mock`])
//!
//! * `GpuBackend` é a abstração mínima real do crate;
//! * [`MockGpuBackend`] executa sem hardware (dados concretos em
//!   `Mutex<HashMap<…>>`), testável e vendorable;
//! * a track real **Tile** (feature `tile`) usa o crate `cutile` (NVIDIA —
//!   `#[cutile::module]` + `kernel::add(…).first().unpartition().to_host_vec()
//!   .sync_on(&stream)`); a track **SIMT** (cuda-oxide) não é consumível como
//!   dependência Cargo e fica fora do escopo (honestidade). Ambas exigem
//!   Linux e sm_80+ para compilar e NÃO se compilam neste ambiente (GTX 1660
//!   Ti sm_75, CUDA 12.1) — por isso `default = []`.
//!
//! ## Invariantes (núcleo `src/lean/GpuInvariantsNucleus.lean`)
//!
//! * **I536-A..D** — isolamento do contrato e banda de geometria;
//! * **I537-A/B** — contenção contabilística da auditoria;
//! * **I538-A** — fecho da banda temporal sob retenção.
//!
//! Prova formal: 7 teoremas em core Lean 4.33.1 (sem Mathlib, sem `sorry`,
//! aritmética Nat), kernel exit 0 — verificado neste crate pelo teste de
//! integração FFI [`tests/gpu_kernel.rs`]. Zero `unsafe`; zero dependências
//! externas novas no core.

pub mod audit;
pub mod backend;
pub mod governance;
pub mod metrics;
pub mod mock;
pub mod tensor;

pub use audit::{AuditEntry, LaunchAudit, LaunchOutcome, LaunchStatus};
pub use backend::{BackendError, BackendKind, GpuBackend, LaunchPlan};
pub use governance::{
    allowed_geometry, i536a_wellformed_entries_satisfied, i536b_threads_within_band,
    i536c_grid_within_band, i536d_geometry_positive, i537a_violations_le_launches,
    i537b_wellformed_has_no_violations, i538a_temporal_band_closed_under_retention,
    simt_duration_ok, tile_duration_ok, LaunchDecision, RejectReason,
    MAX_GRID, MAX_THREADS_PER_BLOCK, SIMT_CAP_MS, TILE_CAP_MS,
};
pub use mock::MockGpuBackend;
pub use tensor::TensorId;

/// Nomes exatos das invariantes do núcleo GPU (ordem do `#check` em
/// `src/lean/GpuInvariantsNucleus.lean` — consumido pelo teste FFI).
pub const GPU_NUCLEUS_INVARIANTS: [&str; 7] = [
    "I536A_wellformed_entries_satisfied",
    "I536B_threads_within_band",
    "I536C_grid_within_band",
    "I536D_geometry_positive",
    "I537A_violations_le_launches",
    "I537B_wellformed_has_no_violations",
    "I538A_temporal_band_closed_under_retention",
];

/// Caminho canónico do núcleo Lean relativo à raiz do monorepo.
pub const GPU_NUCLEUS_LEAN_RELATIVE: &str = "src/lean/GpuInvariantsNucleus.lean";

/// Escala inteira ×10⁴ declarada no núcleo (1.0) — banda Gap-1.
pub const GPU_PHI_SCALE_X1E4: u64 = 10_000;