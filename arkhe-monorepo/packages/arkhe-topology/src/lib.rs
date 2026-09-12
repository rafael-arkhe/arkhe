//! ARKHE TOPOLOGY — invariantes topológicos constitucionais.
//!
//! Módulos:
//! - [`skyrmion`] — número de Skyrmion Nsk (topologia de campos 2D, módulo #8)
//! - [`phi`] — coerência do fator Φ_C/Loopseal (módulo #7)
//! - [`tda`] — validação topológica via persistent homology (TDA) para designs e motores (módulo #3)

#![deny(unsafe_code)]

pub mod phi;
pub mod skyrmion;
pub mod tda;

pub use phi::PhiCoherence;
pub use skyrmion::SkyrmionField;
pub use tda::{MotorTopology, PersistenceDiagram, TdaEngine};