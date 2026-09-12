//! Validação de identidade descentralizada (#14).
//!
//! Suporte a **SIWE (EIP-4361)**: parse e verificação da assinatura (recuperação
//! de endereço via ECDSA-secp256k1 + Keccak); e **EIP-712**: hash tipado EIP-712
//! para dados estruturados ({parameter} struct verificação de assinatura).
//! A verificação é pura, determinística e não se conecta a rede (minimização de dados).

use hex::ToHex;
use k256::ecdsa::{Signature, VerifyingKey};
use sha3::{Digest, Keccak256};
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Erros de validação de identidade.
#[derive(Debug, thiserror::Error)]
pub enum DidError {
    #[error("mensagem SIWE malformada: {0}")]
    Malformed(String),
    #[error("endereço Ethereum não bate (assinatura inválida)")]
    AddressMismatch,
    #[error("não na janela de expiração: {0}")]
    Expired(String),
    #[error("domínio EIP-712 inválido")]
    InvalidDomain,
}

/// Mensagem SIWE (EIP-4361) após parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiweMessage {
    pub domain: String,
    pub address: String,
    pub uri: String,
    pub version: String,
    pub chain_id: u64,
    pub nonce: String,
    pub issued_at: String,
    pub expiration_time: Option<String>,
}

/// Assinatura e metadados para validação.
#[derive(Debug, Clone)]
pub struct SignatureEnvelope {
    pub raw: Vec<u8>,
    pub signer_address: String,
}

impl SiweMessage {
    /// Faz o parse do texto SIWE (formato EIP-4361).
    pub fn parse(message: &str) -> Result<Self, DidError> {
        let mut lines = message.lines();
        let header = lines
            .next()
            .ok_or_else(|| DidError::Malformed("vazio".into()))?;
        // O cabeçalho: "<domain> wants you to sign in with your Ethereum account:"
        let domain = header
            .strip_suffix(" wants you to sign in with your Ethereum account:")
            .ok_or_else(|| DidError::Malformed("cabeçalho inválido".into()))?;
        // A linha seguinte traz o endereço Ethereum.
        let address = lines
            .next()
            .ok_or_else(|| DidError::Malformed("endereço ausente".into()))?;
        let mut chain_id = None;
        let mut uri = None;
        let mut version = None;
        let mut nonce = None;
        let mut issued_at = None;
        let mut expiration_time = None;
        for line in message.lines().skip(1) {
            if let Some(rest) = line.strip_prefix("Chain ID:") {
                chain_id = rest.trim().parse::<u64>().ok();
            } else if let Some(rest) = line.strip_prefix("URI:") {
                uri = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("Version:") {
                version = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("Nonce:") {
                nonce = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("Issued At:") {
                issued_at = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("Expiration Time:") {
                expiration_time = Some(rest.trim().to_string());
            }
        }
        let chain_id =
            chain_id.ok_or_else(|| DidError::Malformed("sem Chain ID".into()))?;
        let uri = uri.ok_or_else(|| DidError::Malformed("sem URI".into()))?;
        let version = version.ok_or_else(|| DidError::Malformed("sem Version".into()))?;
        let nonce = nonce.ok_or_else(|| DidError::Malformed("sem Nonce".into()))?;
        let issued_at =
            issued_at.ok_or_else(|| DidError::Malformed("sem Issued At".into()))?;
        Ok(Self {
            domain: domain.trim().to_string(),
            address: normalize_address(address.trim())?,
            uri,
            version,
            chain_id,
            nonce,
            issued_at,
            expiration_time,
        })
    }

    /// Verifica a assinatura contra a mensagem SIWE e a identidade declarada.
    pub fn verify(&self, message: &str, env: &SignatureEnvelope) -> Result<(), DidError> {
        let digest = Keccak256::digest(message.as_bytes());
        let sig: Signature = k256::ecdsa::Signature::from_slice(&env.raw[..64])
            .map_err(|_| DidError::Malformed("assinatura inválida".into()))?;
        let recovery_id = k256::ecdsa::RecoveryId::try_from(env.raw[64])
            .map_err(|_| DidError::Malformed("recovery id".into()))?;
        let vk = VerifyingKey::recover_from_prehash(&digest, &sig, recovery_id)
            .map_err(|_| DidError::Malformed("falhou recuperação".into()))?;
        let recovered = address_from_verifying_key(&vk);
        if recovered.to_lowercase() != env.signer_address.to_lowercase() {
            return Err(DidError::AddressMismatch);
        }
        Ok(())
    }
}

fn normalize_address(address: &str) -> Result<String, DidError> {
    let a = address.trim();
    if !a.starts_with("0x") || a.len() != 42 {
        return Err(DidError::Malformed("endereço fora do padrão 0x.. 20 bytes".into()));
    }
    Ok(a.to_string())
}

fn address_from_verifying_key(vk: &VerifyingKey) -> String {
    let pt = vk.to_encoded_point(false);
    let public_key = &pt.as_bytes()[1..]; // 64 bytes
    let hash = Keccak256::digest(public_key);
    let bytes = &hash[12..];
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    out.push_str(&bytes.encode_hex::<String>());
    out
}

/// Domínio EIP-712.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Eip712Domain {
    pub name: String,
    pub version: String,
    pub chain_id: u64,
    pub verifying_contract: String,
}

/// Campo tipado de uma mensagem EIP-712.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TypedField {
    Address,
    Uint256,
    String,
    Bool,
}

/// Estrutura tipada EIP-712.
#[derive(Debug, Clone)]
pub struct TypedStruct {
    pub name: String,
    pub fields: Vec<(String, TypedField)>,
    pub values: BTreeMap<String, String>,
}

const EIP712_DOMAIN_TYPE: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";

/// Calcula o digest EIP-712 de uma mensagem tipada.
pub fn eip712_digest(
    domain: &Eip712Domain,
    message: &TypedStruct,
) -> Result<[u8; 32], DidError> {
    if !domain.verifying_contract.starts_with("0x") {
        return Err(DidError::InvalidDomain);
    }
    let mut hasher = Keccak256::new();
    hasher.update(encode_type(&message.name, &message.fields));
    let type_hash = hasher.finalize();

    let mut msg_hasher = Keccak256::new();
    msg_hasher.update([0x19u8, 0x01u8]);
    msg_hasher.update(encode_domain(domain));
    msg_hasher.update(type_hash);
    msg_hasher.update(encode_data(&message.fields, &message.values));
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&msg_hasher.finalize());
    Ok(digest)
}

fn encode_type(name: &str, fields: &[(String, TypedField)]) -> String {
    let mut s = format!("{name}(");
    let parts: Vec<String> = fields
        .iter()
        .map(|(n, t)| format!("{}({n})", field_kind(t)))
        .collect();
    s.push_str(&parts.join(","));
    s.push(')');
    s
}

fn field_kind(t: &TypedField) -> String {
    match t {
        TypedField::Address => "address".into(),
        TypedField::Uint256 => "uint256".into(),
        TypedField::String => "string".into(),
        TypedField::Bool => "bool".into(),
    }
}

fn encode_domain(domain: &Eip712Domain) -> [u8; 32] {
    // keccak(domainSeparator) = keccak(EIP712Domain(...) + encoded)
    let mut h = Keccak256::new();
    h.update(EIP712_DOMAIN_TYPE.as_bytes());
    h.update(
        format!("{}\x02{}\x03{}", domain.name, domain.version, domain.chain_id)
            .as_bytes(),
    );
    h.update(hex::decode(domain.verifying_contract.trim_start_matches("0x")).unwrap_or_default());
    h.finalize().into()
}

fn encode_data(fields: &[(String, TypedField)], values: &BTreeMap<String, String>) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, t) in fields {
        let v = values.get(name);
        match t {
            TypedField::Address | TypedField::Uint256 => {
                let s = v.map(|s| s.trim_start_matches("0x")).unwrap_or("");
                let bytes = hex::decode(s).unwrap_or_default();
                let mut b = [0u8; 32];
                let n = bytes.len().min(32);
                b[32 - n..].copy_from_slice(&bytes);
                out.extend_from_slice(&b);
            }
            TypedField::String => {
                let h = Keccak256::digest(v.map_or("", |s| s).as_bytes());
                out.extend_from_slice(&h);
            }
            TypedField::Bool => {
                let v = v.map(|s| s == "true").unwrap_or(false);
                out.push(if v { 1 } else { 0 });
                out.extend_from_slice(&[0u8; 31]);
            }
        }
    }
    out
}

/// Verifica uma assinatura compacta (r‖s‖v) contra o digest.
pub fn verify_signature(digest: &[u8; 32], env: &SignatureEnvelope) -> Result<(), DidError> {
    let sig: Signature = k256::ecdsa::Signature::from_slice(&env.raw[..64])
        .map_err(|_| DidError::Malformed("assinatura inválida".into()))?;
    let recovery_id = k256::ecdsa::RecoveryId::try_from(env.raw[64])
        .map_err(|_| DidError::Malformed("recovery id".into()))?;
    // `recover_from_prehash` verifica a assinatura internamente (eip: pk recovered).
    let vk = VerifyingKey::recover_from_prehash(&digest[..], &sig, recovery_id)
        .map_err(|_| DidError::Malformed("recuperação falhou".into()))?;
    let recovered = address_from_verifying_key(&vk);
    if recovered.to_lowercase() != env.signer_address.to_lowercase() {
        return Err(DidError::AddressMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::ecdsa::SigningKey;

    fn signer() -> (SigningKey, String) {
        let signing = SigningKey::random(&mut rand::rngs::OsRng);
        let vk = signing.verifying_key();
        let addr = address_from_verifying_key(vk);
        (signing, addr)
    }

    /// Assina (prehash) e serializa no formato compacto r‖s‖v.
    fn sign_recoverable(sk: &SigningKey, prehash: &[u8]) -> Vec<u8> {
        let (sig, rid) = sk
            .sign_prehash_recoverable(prehash)
            .expect("falhou assinatura recoverable");
        let mut raw = sig.to_bytes().to_vec();
        raw.push(u8::from(rid));
        raw
    }

    #[test]
    fn parse_siwe_message() {
        let msg = "example.com wants you to sign in with your Ethereum account:\n0x1234567890abcdef1234567890abcdef12345678\n\nI accept the Terms.\n\nURI: https://example.com/login\nVersion: 1\nChain ID: 11155111\nNonce: abcdefg\nIssued At: 2026-01-01T00:00:00Z";
        let siwe = SiweMessage::parse(msg).unwrap();
        assert_eq!(siwe.domain, "example.com");
        assert_eq!(siwe.chain_id, 11155111);
        assert_eq!(siwe.nonce, "abcdefg");
    }

    #[test]
    fn malformed_siwe_rejected() {
        assert!(SiweMessage::parse("nope").is_err());
    }

    #[test]
    fn eip712_digest_is_deterministic() {
        let domain = Eip712Domain {
            name: "ARKHE".into(),
            version: "1".into(),
            chain_id: 1,
            verifying_contract: "0x0000000000000000000000000000000000000000".into(),
        };
        let msg = TypedStruct {
            name: "ReputationClaim".into(),
            fields: vec![
                ("owner".into(), TypedField::Address),
                ("score".into(), TypedField::Uint256),
            ],
            values: BTreeMap::from([
                ("owner".into(), "0x0000000000000000000000000000000000000001".into()),
                ("score".into(), "100".into()),
            ]),
        };
        let a = eip712_digest(&domain, &msg).unwrap();
        let b = eip712_digest(&domain, &msg).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn sign_verify_roundtrip() {
        let (sk, addr) = signer();
        let msg = "I am node abc";
        let msg_hash = Keccak256::digest(msg.as_bytes());
        let raw = sign_recoverable(&sk, &msg_hash);
        let env = SignatureEnvelope { raw, signer_address: addr.clone() };
        let digest: [u8; 32] = msg_hash.into();
        verify_signature(&digest, &env).unwrap();
    }

    #[test]
    fn wrong_signer_rejected() {
        let (sk, _) = signer();
        let (_sk2, addr2) = signer();
        let msg_hash = Keccak256::digest(b"x");
        let raw = sign_recoverable(&sk, &msg_hash);
        let env = SignatureEnvelope { raw, signer_address: addr2 };
        let digest: [u8; 32] = msg_hash.into();
        assert!(verify_signature(&digest, &env).is_err());
    }
}