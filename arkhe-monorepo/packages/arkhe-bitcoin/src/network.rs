//! Redes Bitcoin suportadas.

use serde::{Deserialize, Serialize};

/// Rede Bitcoin alvo das chaves/endereços.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Network {
    /// Mainnet (produção).
    #[default]
    Bitcoin,
    /// Testnet3 (regressão de consenso pública).
    Testnet,
    /// Regtest (local, determinístico).
    Regtest,
    /// Signet (rede de teste sem reorgs espontâneos).
    Signet,
}

impl Network {
    /// Mapeia a rede para o enum do crate `bitcoin`.
    pub fn to_bitcoin_network(&self) -> bitcoin::Network {
        match self {
            Self::Bitcoin => bitcoin::Network::Bitcoin,
            Self::Testnet => bitcoin::Network::Testnet,
            Self::Regtest => bitcoin::Network::Regtest,
            Self::Signet => bitcoin::Network::Signet,
        }
    }

    /// Versão de P2PKH (base58) desta rede.
    pub fn prefix(&self) -> u8 {
        match self {
            Self::Bitcoin => 0x00,
            _ => 0x6f,
        }
    }

    /// Versão de P2SH (base58) desta rede.
    pub fn p2sh_prefix(&self) -> u8 {
        match self {
            Self::Bitcoin => 0x05,
            _ => 0xc4,
        }
    }

    /// Human-readable part (bech32) desta rede.
    pub fn bech32_hrp(&self) -> &'static str {
        match self {
            Self::Bitcoin => "bc",
            Self::Testnet => "tb",
            Self::Regtest => "bcrt",
            Self::Signet => "sb",
        }
    }
}

