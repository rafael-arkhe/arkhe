//! Arkhe Field Stability — métricas de estabilidade de campo, relatórios de
//! qualidade e integração MCP (via stub HTTP).
//!
//! Este crate nasceu da proposta `arkhe-safe-core` (bloco 987) e foi
//! **renomeado** para `arkhe-field-stability` para evitar colisão de
//! namespace com o já existente `services/safe-core` (o watchdog
//! anti-alucinação do substrate 491-AGI-CORTEX).
//!
//! ## Módulos
//!
//! * [`field_stability`] — estabilidade a partir de histórico de coerência Φ.
//! * [`quality_report`] — relatório consolidado com score ponderado.
//! * [`coherence`] — métrica formal de coerência Φ (Fase 1, Gap-1).
//! * [`refiner`] — `IterativeRefiner`, gradiente ascendente com Φ como alvo
//!   (Fase 2).
//! * [`ledger`] — `CoherenceLedger`, cadeia de dados append-only encadeada
//!   por hash SHA3-256 (Fase 4, Opção A; Gravity-1 nativo).
//! * [`validators`] — SemanticValidity, eixo de garantia da Fase 7 com quórum
//!   estrito de validadores (ortogonal a Φ).
//! * [`loopseal`] — eixo de garantia da Fase 7, detecção de loops/aelíclicidade
//!   em cadeias encadeadas por hash (ortogonal a Φ).
//! * [`phi`] — envelope de relatório Fase 7: Φ canônico quadrático + eixos de
//!   garantia (SemanticValidity/Loopseal) consolidados.
//! * [`mcp_stub`] — cliente HTTP stub para o protocolo MCP (substitui o
//!   `arkhe-mcp-client` externo, que não tinha evidência pública de manutenção).
//! * [`constants`] — pesos heurísticos e limiares.
//! * [`decay`] — invariante I530: decaimento de coerência limitado à banda
//!   [0,1] (bloco 1009; prova core Lean em `src/lean/I530_CoherenceDecay.lean`).
//! * [`experiment`] — infraestrutura dos experimentos E1–E4 (feature
//!   `experiments`; compilado apenas com `--features experiments`).

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod coherence;
pub mod constants;
pub mod decay;
#[cfg(feature = "experiments")]
pub mod experiment;
pub mod field_stability;
pub mod ledger;
pub mod loopseal;
pub mod mcp_stub;
pub mod phi;
pub mod quality_report;
pub mod refiner;
pub mod validators;

pub use coherence::{phi, phi_default, phi_from_field_stability, WEIGHTS_DEFAULT};
pub use constants::*;
pub use decay::{decay_scaled, decayed, CEILING_X1E4, SCALE_X1E4};
pub use field_stability::FieldStability;
pub use ledger::{
    CoherenceEntry, CoherenceLedger, IntegrityStatus, GENESIS, GRAVITY_1,
    MAX_HORIZON_WINDOWS,
};
pub use loopseal::{verify_chain_acyclic, ChainLink, LoopSeal, LoopStatus};
pub use mcp_stub::{HandoverPayload, McpClient, McpError};
pub use phi::{Assurance, CoherenceReport, GAP1_INFERIOR, GAP1_SUPERIOR, gap1_satisfied};
pub use quality_report::QualityReport;
pub use refiner::{IterativeRefiner, RefinerOutcome, RefinerResult};
pub use validators::{
    aggregate_validity, SemanticValidity, Validator, ValidatorSetError, Verdict, MIN_VALIDATORS,
    VALIDATOR_QUORUM,
};