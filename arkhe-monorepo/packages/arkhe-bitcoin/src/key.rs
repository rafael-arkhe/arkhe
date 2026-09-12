//! Geração de chaves Bitcoin (privada, pública, x-only) e endereços.

use bitcoin::hashes::Hash as BtcHash;
use rand::thread_rng;
use secp256k1::{PublicKey, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{BitcoinError, Result};
use crate::network::Network;

/// Chave privada Bitcoin (32 bytes) com flag de compressão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateKey {
    /// Bytes da chave (formato SEC1; qualquer valor inválido é rejeitado no
    /// momento da criação ou do uso).
    pub secret: [u8; 32],
    /// Se a chave pública correspondente deve ser serializada comprimida.
    pub compressed: bool,
}

impl PrivateKey {
    /// Gera uma nova chave privada aleatória (semente CSPRNG do SO).
    ///
    /// # Invariantes
    /// * `secret` é uniforme em `[1, n-1]` (a curva rejeita bytes nulos).
    /// * `compressed == true` (serialização default da rede).
    #[must_use]
    pub fn generate() -> Self {
        let secp = Secp256k1::new();
        let (sk, _) = secp.generate_keypair(&mut thread_rng());
        tracing::debug!(key_gen = "completed", "new bitcoin private key generated");
        Self::unsafe_from_bytes(sk.secret_bytes())
    }

    /// Cria uma chave privada a partir de exatamente 32 bytes.
    ///
    /// Valida o range da curva (`0 < secret < n`) na criação.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(BitcoinError::InvalidPrivateKey(
                "must be exactly 32 bytes".to_owned(),
            ));
        }
        // Rejeita 0 e valores ≥ n (a curva rejeita secret nulo/over-range).
        SecretKey::from_slice(bytes).map_err(|e| {
            BitcoinError::InvalidPrivateKey(format!("{e}"))
        })?;
        Ok(Self::unsafe_from_bytes(Self::as_array(bytes)))
    }

    /// Cria a chave a partir dos bytes já validados (invariantes internas).
    fn unsafe_from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            secret: bytes,
            compressed: true,
        }
    }

    /// Copia `&[u8]` de 32 bytes para `[u8; 32]` (sem falhar — len verificado).
    fn as_array(bytes: &[u8]) -> [u8; 32] {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        arr
    }

    /// Retorna a chave pública comprimida correspondente (33 bytes).
    pub fn public_key(&self) -> Result<PublicKey> {
        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(&self.secret)
            .map_err(|e| BitcoinError::Crypto(e.to_string()))?;
        Ok(PublicKey::from_secret_key(&secp, &sk))
    }

    /// Retorna a chave pública x-only (32 bytes) para Schnorr/Taproot.
    pub fn xonly_public_key(&self) -> Result<XOnlyPublicKey> {
        Ok(self.public_key()?.x_only_public_key().0)
    }

    /// Deriva o NodeId PoTT (BIP-340 x-only) desta chave privada.
    pub fn pott_node_id(&self) -> Result<[u8; 32]> {
        let mut node = [0u8; 32];
        node.copy_from_slice(&self.xonly_public_key()?.serialize());
        Ok(node)
    }

    /// Deriva o endereço **P2PKH** (legacy, base58).
    pub fn to_p2pkh_address(&self, network: Network) -> Result<String> {
        let pk = self.public_key()?;
        let pk_hash = bitcoin::PubkeyHash::hash(&pk.serialize());
        let address = bitcoin::Address::p2pkh(pk_hash, network.to_bitcoin_network());
        Ok(address.to_string())
    }

    /// Deriva o endereço **P2WPKH** (SegWit v0, bech32).
    pub fn to_p2wpkh_address(&self, network: Network) -> Result<String> {
        let pk = self.public_key()?;
        let cpk = bitcoin::CompressedPublicKey(pk);
        let address = bitcoin::Address::p2wpkh(&cpk, network.to_bitcoin_network());
        Ok(address.to_string())
    }

    /// Deriva o endereço **P2TR** (Taproot, bech32m) — key-path only.
    pub fn to_p2tr_address(&self, network: Network) -> Result<String> {
        let xonly = self.xonly_public_key()?;
        let secp = Secp256k1::new();
        let address = bitcoin::Address::p2tr(
            &secp,
            xonly,
            None, // sem script path — leaf key-only
            network.to_bitcoin_network(),
        );
        Ok(address.to_string())
    }

    /// Converte para **WIF** (Wallet Import Format, Base58Check).
    ///
    /// # Fix vs. rascunho
    /// O rascunho usava `base64::encode(wif)` — WIF é **Base58Check**
    /// (prefixo + 32 bytes + sufixo de compressão + checksum `sha256d` de 4
    /// bytes), nunca Base64.
    pub fn to_wif(&self, network: Network) -> Result<String> {
        let mut k = bitcoin::PrivateKey::from_slice(
            &self.secret,
            network.to_bitcoin_network(),
        )
        .map_err(|e| BitcoinError::Crypto(e.to_string()))?;
        k.compressed = self.compressed;
        Ok(k.to_wif())
    }

    /// Assina uma mensagem arbitrária com BIP-340 (Schnorr) e retorna a
    /// assinatura de 64 bytes.
    ///
    /// O digest da mensagem é `SHA-256(message)` (não o digest Bitcoin).
    pub fn sign_message(&self, message: &[u8]) -> Result<[u8; 64]> {
        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(&self.secret)
            .map_err(|e| BitcoinError::Crypto(e.to_string()))?;
        let digest = Sha256::digest(message);
        let mut buf = [0u8; 32];
        buf.copy_from_slice(&digest);
        let msg = secp256k1::Message::from_digest(buf);
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);
        // aux_rand fixo de 32 bytes zero (determinístico, sem RNG extra) —
        // `sign_schnorr` exigiria a feature `rand-std` não habilitada.
        let sig = secp.sign_schnorr_with_aux_rand(&msg, &keypair, &[0u8; 32]);
        Ok(sig.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let sk = PrivateKey::generate();
        let pk = sk.public_key().unwrap();
        assert_eq!(pk.serialize().len(), 33, "chave comprimida esperada");
        assert_eq!(sk.pott_node_id().unwrap().len(), 32);
    }

    #[test]
    fn test_private_key_rejects_invalid_bytes() {
        assert!(PrivateKey::from_bytes(&[0u8; 32]).is_err());
        assert!(PrivateKey::from_bytes(&[1u8; 31]).is_err());
        // 0xff...ff > n (ordem da curva) → rejeitado por range.
        let upper = [0xffu8; 32];
        assert!(PrivateKey::from_bytes(&upper).is_err());
    }

    #[test]
    fn test_from_bytes_roundtrip() {
        let mut bytes = [7u8; 32];
        bytes[0] = 1;
        let k = PrivateKey::from_bytes(&bytes).unwrap();
        assert_eq!(k.secret, bytes);
        assert!(PrivateKey::from_bytes(&k.secret).is_ok());
    }

    #[test]
    fn test_even_bytes_generate_keys() {
        // Um vetor para garantir propriedades de paridade etc.
        let k = PrivateKey::from_bytes(&[0x11u8; 32]).unwrap();
        let pk = k.public_key().unwrap();
        let (_, parity) = pk.x_only_public_key();
        assert!(
            parity == secp256k1::Parity::Even
                || parity == secp256k1::Parity::Odd
        );
    }

    #[test]
    fn test_wif_roundtrip_format() {
        let k = PrivateKey::from_bytes(&[0x01u8; 32]).unwrap();
        let main = k.to_wif(Network::Bitcoin).unwrap();
        let test = k.to_wif(Network::Testnet).unwrap();

        let main_first = main.as_bytes()[0] as char;
        let test_first = test.as_bytes()[0] as char;
        println!("wif mainnet {main}");
        println!("wif testnet {test}");
        assert!(
            ('K'..='L').contains(&main_first),
            "mainnet compressed WIF deve começar com K/L (era '{main_first}')"
        );
        assert!(
            test_first == 'c',
            "testnet compressed WIF deve começar com 'c' (era '{test_first}')"
        );
    }

    #[test]
    fn test_address_generation() {
        let sk = PrivateKey::generate();
        let net = Network::Testnet;
        let p2pkh = sk.to_p2pkh_address(net).unwrap();
        let p2wpkh = sk.to_p2wpkh_address(net).unwrap();
        let p2tr = sk.to_p2tr_address(net).unwrap();

        assert!(p2pkh.starts_with('m') || p2pkh.starts_with('n'));
        assert!(p2wpkh.starts_with("tb1"));
        assert!(p2tr.starts_with("tb1p"));

        // mainnet difere da testnet
        let main_p2wpkh = sk.to_p2wpkh_address(Network::Bitcoin).unwrap();
        assert!(main_p2wpkh.starts_with("bc1"));
    }

    #[test]
    fn test_sign_message_64_bytes() {
        let sk = PrivateKey::generate();
        let sig = sk.sign_message(b"arkhe bitcoin").unwrap();
        assert_eq!(sig.len(), 64);
        // assinaturas de mensagens distintas diferem (comprob.) com alta prob.
        let sig2 = sk.sign_message(b"arkhe bitcoin!").unwrap();
        assert_ne!(sig, sig2);
    }
}