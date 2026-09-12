#![deny(unsafe_code)]
#![warn(missing_docs)]

//! # `arkhe-bitcoin` — Geração de chaves e endereços Bitcoin
//!
//! Interface segura e flexível para criação de chaves, endereços e assinaturas
//! no ecossistema ARKHE, com suporte a:
//!
//! * **P2PKH** (legacy), **P2WPKH** (SegWit v0) e **P2TR** (Taproot key-path).
//! * **WIF** correto: Base58Check via crate `bitcoin` (não Base64).
//! * Derivação do **NodeId PoTT** (BIP-340 x-only) para o substrate
//!   [`924`](arkhe_pott) e geração opcional de receipts de custódia.
//!
//! ## Segurança
//!
//! * `#![deny(unsafe_code)]` — nenhum `unsafe`.
//! * Chaves privadas validadas na curva (`0 < secret < n`) na criação.
//! * Endereços validados contra a rede alvo antes de qualquer uso.
//! * Receipts PoTT auto-verificados antes de retornar ao chamador.
//!
//! ## Feature `pott`
//!
//! Ative `features = ["pott"]` para habilitar a integração com
//! `arkhe-pott` (substrate 924-POTT-INTERPLANETARY-TRANSPORT).

pub mod address;
pub mod error;
pub mod key;
pub mod network;

#[cfg(feature = "pott")]
pub mod pott_integration;

pub use address::{generate_dummy_address, parse_address, validate_address};
pub use error::{BitcoinError, Result};
pub use key::PrivateKey;
pub use network::Network;

/// Versão do crate (espaço de nomes ARKHE).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");