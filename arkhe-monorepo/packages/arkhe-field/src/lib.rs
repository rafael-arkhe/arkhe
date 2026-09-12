//! # ARKHE Field 1.0 — escala constitucional de 2370 MeV
//!
//! Módulo de campo da pesquisa ARKHE-VSEPR (núcleo Rust 2, 2026-09-09) que ancora
//! o runtime nos dois resultados experimentais de 2026:
//!
//! 1. **X(2370) como glueball 0⁻⁺ dominante** — BESIII, apresentado no plenário da
//!    ICHEP 2026 (5 de agosto de 2026, Brasil). Preprint [arXiv:2607.20366]
//!    (22 de julho de 2026). Revisão completa em [arXiv:2503.13286] e
//!    [Nucl. Phys. B 1028:117495 (2026)].
//! 2. **Primeira medição simultânea de alta precisão de dois parâmetros de
//!    oscilação de neutrinos** — JUNO, [Nature 654, 343–348 (2026)],
//!    DOI [10.1038/s41586-026-10538-z]: sin²θ₁₂ = 0.3092 ± 0.0087 e
//!    Δm²₂₁ = (7.50 ± 0.12) × 10⁻⁵ eV².
//!
//! E na matemática:
//!
//! 3. **Completação da classificação de Parker–Rowley** — [arXiv:2607.27256]
//!    (H. Dietrich, 28 de julho de 2026) prova que o Grupo Monster é uma
//!    completação do amalgam de Goldschmidt G₃, fechando o caso aberto de
//!    Parker–Rowley ([J. Algebra 235:131–153, 2001]).
//!
//! ## Cadeia de evidências
//!
//! A correspondência VSEPR↔Glueball mapeia a camada de valência constitucional
//! (substrato 227-F) numa geometria simetria-singlete: o mesmo espelhamento entre
//! domínios de pares eletrônicos equivalentes (VSEPR `AXₙ`, sem pares livre → campo
//! dipolar nulo) e os cinco modos de decaimento "dourados" PPP flavo-simétricos do
//! X(2370) observados pela BESIII. A cadeia completa está documentada em
//! [`EVIDENCE_CHAIN.md`](../EVIDENCE_CHAIN.md) (no repo do crate).
//!
//! ## Invariantes constitucionais impactados
//!
//! - **Ghost-3 / Correlation-1**: toda constante aqui é âncora de referência
//!   cruzada verificável (DOI/arXiv) e reconcilia entre si.
//! - **Gap-1**: a escala 2370 MeV entra no cálculo de Φ_C como invariante de
//!   coerência de campo; a matriz de mistura é unitária por construção.
//! - **Gap-2**: unidades e valores só de fontes primárias de 2026, sem inferência.
//! - **Loopseal-1/2**: `TOONChainMonster` é ancorado e append-only.
//! - **Simplicity-2**: dependência do crate reduzida a `serde` + `sha3`.

pub mod glueball;
pub mod groups;
pub mod juno;
pub mod mixing;
pub mod monster;
pub mod x2370;

pub use glueball::{
    CorrespondenceCheck, GlueballValidation, VseprGlueballCorrespondence, vsepr_geometry_of,
};
pub use groups::{CompletionResult, GoldschmidtG3, PermGroup, compose, dih8, sym4};
pub use juno::{
    JUNO_BASELINE_KM, JUNO_DAYS, JUNO_DELTA_M21_ERR, JUNO_DELTA_M21_SQ_EV2, JUNO_NATURE_REF,
    JUNO_PRECISION_IMPROVEMENT, JUNO_SIN2_THETA12, JUNO_SIN2_THETA12_ERR, JunoMeasurement,
    juno_reference, juno_sin2_theta12, juno_spectrum_measurement,
};
pub use mixing::{Cmplx, Layer, LayerMatrix, ReactorValidation, determine_dm21_sq_from_survival};
pub use monster::{
    ARXIV_2607_27256, ClassificationLedger, COMPLETERS_2026,
    DIETRICH_LEE_POPIEL_REF, MONSTER_NAME, MONSTER_ORDER_F64, MONSTER_ORDER_STR,
    MONSTER_PRIME_EXPONENTS, MonsterMaximalArgument, NONCOMPLETERS_2026,
    PARKER_ROWLEY_2001_REF, PARKER_ROWLEY_COMPLETERS_2001, PARKER_ROWLEY_NONCOMPLETERS_2001,
    SPORADIC_COUNT, TOONChainMonster, ToonAnchor, monster_has_prime_part,
    psl2_order, verify_monster_classification, verify_parker_rowley_completion,
};
pub use x2370::{
    BESIII_REFERENCES, B_JPSI_GAMMA_X_TIMES_B_KSTAR_LIMIT, FLAVOR_SINGLET_MIN_MEV,
    KSTAR_SUPPRESSION_CL, SPIN_PARITY_SIGNIFICANCE_SIGMA, X2370Channel, X2370Evidence,
    X_MASS_MEV_SIGNATURE, X_MASS_SIGNATURE_MEV, X_REFERENCE_ICHEP_2026, X_REFERENCE_V782,
    X_WITH_SIGNATURE_MEV, besiii_evidence, besiii_references,
};