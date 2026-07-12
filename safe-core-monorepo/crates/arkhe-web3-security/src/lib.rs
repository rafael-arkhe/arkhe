//! Invariantes formais de segurança Web3 — OWASP Smart Contract Top 10 2026.
//!
//! Mapeamento: cada submódulo implementa um ou mais invariantes `FI-W##`
//! descritos em `web3-security-architecture.md`. Esta é a versão skeleton
//! (v2.2): lógica real e testada, sem provas Lean 4 / harnesses Kani ainda.

#![deny(unsafe_code)]

pub mod agents;
pub mod contracts;
pub mod blockchain;
pub mod wallets;
pub mod dapps;
pub mod analyzers;
pub mod reports;
pub mod web3_adapter;
#[cfg(kani)]
mod verify;

/// Identificador de invariante formal (`FI-W01`..`FI-W20`).
pub type InvariantId = &'static str;

/// Veredito de checagem de um invariante Web3.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InvariantVerdict {
    /// Invariante respeitado.
    Holds,
    /// Invariante violado — inclui explicação legível.
    Violated { reason: String },
}

impl InvariantVerdict {
    /// True se o invariante foi respeitado.
    pub fn holds(&self) -> bool {
        matches!(self, InvariantVerdict::Holds)
    }

    /// Constrói um veredito violado a partir de uma mensagem.
    pub fn violated(reason: impl Into<String>) -> Self {
        InvariantVerdict::Violated { reason: reason.into() }
    }
}
