//! ARKHE-VSEPR Núcleo 3 — Gap de Proteção (Camada 3).
//!
//! Ancorado em dois dos Sete Selos de agosto de 2026:
//!
//! - **Selo #2 — arXiv:2608.13907** "Robust Quantum Extremal Numbers"
//!   (W. Zhang, Z. Han, X. Zhang, 14 ago 2026): a robustez da mistura maximal
//!   exata é quantificada pelo **defeito de maximal-mixing**
//!   `D_A = 2^|A|·Tr(ρ_A²) − 1`. O **platô exato** é
//!   `Q_ex,ε^D(8,4) = 56` para `0 ≤ ε < 1/5` e o limite superior
//!   `Q_ex,ε^D(9,4) ≤ 120` para `0 ≤ ε < 1/17`, via a desigualdade de estabilidade
//!   local `Σ_{i∈T} D_{T∖{i}} ≥ 1` (|T| = 2m+1, 4m qubits).
//!
//! - **Selo #6 — Phys. Rev. X 16, 031040** (2026) "Coulomb Screening of
//!   Superconductivity in Magic-Angle Graphene" (Barrier et al.): blindagem
//!   Coulombiana com eficiência exponencial em escala ~2 nm **suprime
//!   completamente** a supercondutividade do grafeno de ângulo mágico (~1.1°) —
//!   o mecanismo de **controle ativo** do gap C-Z calibrado.
//!
//! Invariantes: Gap-1 (Φ_C), Gap-2 (entropia), Simplicity-2 (superfície mínima).

pub mod extremal;
pub mod shielding;

pub use extremal::{
    EXTREMAL_8_QU4_REF, EXTREMAL_9_QU4_REF, BindKind, ExtremalBinding,
    QEX_8_4_PLATEAU, QEX_9_4_UPPER_BOUND, STABILITY_EPS_8_QU4, STABILITY_EPS_9_QU4,
    defect_maximal_mixing, extremal_8_4, extremal_9_4, local_stability_holds,
};
pub use shielding::{
    ARKHE_TWIST_DEG, COULOMB_SCREEN_REF, MAGIC_ANGLE_TBG_DEG, PRIMARY_SCREENING_DIST_NM,
    SCREENING_DECAY_SCALE_NM, TBG_SCREEN_REF, CoulombShield, ShieldingProtocol,
    is_suppressive_distance, requires_active_shielding, shielding_factor,
};