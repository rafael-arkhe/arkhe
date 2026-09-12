//! ARKHE-VSEPR Núcleo 3 — qhttp (Camada 4, Processamento de Sinais de Fase).
//!
//! Três dos Sete Selos de agosto de 2026 definem esta camada:
//!
//! - **Selo #5 — arXiv:2608.14387** "Linearised quantum signal processing"
//!   (M. Arsenault, H. Kristjánsson, 14 ago 2026, 65 pp.): a UHET é uma
//!   **linearização (randomizada) do GQSP**; a nova **UHSVT** transforma os
//!   valores singulares de qualquer matriz `A` codificada em bloco de um
//!   Hamiltoniano, com a **única** condição `f(0) = 0`.
//!
//! - **Selo #3 — arXiv:2608.13781** "Entanglement asymmetry characterization of
//!   the Chiral Anomaly" (A. B. G. Sierra, 13 ago 2026): a assimetria de
//!   emaranhamento permanece **não nula no limite termodinâmico** apesar do
//!   comutador axial↔vetorial ir a zero — a projeção Fase→Estrutura nunca é
//!   perfeita, e `λ₂ = 0.9991 + δ` incorpora essa pegada ontológica.
//!
//! - **Selo #4 — arXiv:2608.12445** "Limits on Velocity Recovery from the PURSUE
//!   Sensor Videos" (J. Haqq-Misra, R. Kopparapu, 12 ago 2026): classificação de
//!   anomalias exige o conjunto completo de parâmetros e incerteza de fase
//!   controlada `< 0.1 rad`.
//!
//! Invariantes: Gap-1/2, Loopseal-2 (protocolo determinístico), Simplicity-2.

pub mod anomaly;
pub mod coherence;
pub mod uhsvt;

pub use anomaly::{
    ANOMALY_PARAMS_REQUIRED, BOUND_MACH04_FRACTION, PHASE_UNCERTAINTY_TARGET_RAD, PURSUE_CLIP_COUNT,
    PURSUE_REF_2026, AnomalyClass, AnomalyReport, classify_anomaly, classify_phase_uncertainty,
    reconstruct_velocity,
};
pub use coherence::{
    CHIRAL_ANOMALY_REF_2026, COHERENCE_LAMBDA2, ChiralCorrection, PhaseCoherenceMetric,
    anomaly_correction_delta, coherence_function,
};
pub use uhsvt::{
    LQSP_REF_2026, UhsvtSpec, transform_function_singular_values, uhsvt_spec,
    valid_uhsvt_function,
};