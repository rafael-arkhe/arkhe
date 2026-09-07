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
//! * [`mcp_stub`] — cliente HTTP stub para o protocolo MCP (substitui o
//!   `arkhe-mcp-client` externo, que não tinha evidência pública de manutenção).
//! * [`constants`] — pesos heurísticos e limiares.
//! * [`experiment`] — infraestrutura dos experimentos E1–E4 (feature
//!   `experiments`; compilado apenas com `--features experiments`).

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod coherence;
pub mod constants;
#[cfg(feature = "experiments")]
pub mod experiment;
pub mod field_stability;
pub mod ledger;
pub mod mcp_stub;
pub mod quality_report;
pub mod refiner;

pub use coherence::{phi, phi_default, phi_from_field_stability, WEIGHTS_DEFAULT};
pub use constants::*;
pub use field_stability::FieldStability;
pub use ledger::{CoherenceEntry, CoherenceLedger, GENESIS, GRAVITY_1};
pub use mcp_stub::{HandoverPayload, McpClient, McpError};
pub use quality_report::QualityReport;
pub use refiner::{IterativeRefiner, RefinerOutcome, RefinerResult};