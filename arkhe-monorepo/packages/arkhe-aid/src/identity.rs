//! Identidade criptografica de agentes (AID-001).
//!
//! Cada agente recebe um keypair Ed25519 no registro; a chave privada e
//! retornada UMA VEZ ao operador e nunca armazenada pelo sistema (principio
//! "Minimal trust" do VAIP). A chave publica (32 bytes) ancora todas as
//! acoes e eventos do agente.
//!
//! Fingerprint: SHA-256 da forma canonica da chave publica, prefixado com
//! `sha256:`. O VAIP define o fingerprint sobre a chave publica em PEM/SPKI;
//! neste bloco o hash e sobre os 32 bytes crus da chave Ed25519 (simplificacao
//! documentada e auditavel — sem gerador PEM neste bloco).
//!
//! did:key (adocao W3C): identificador por base58btc multibase ('z') sobre o
//! codigo multicodec Ed25519-pub (0xED01) + chave publica. A resolucao
//! completa W3C nao faz parte do escopo deste bloco.

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Prefixo do fingerprint (normativa VAIP: hash SHA-256 da chave publica).
pub const KEY_PKCS8_PREFIX: &str = "sha256:";

/// Fingerprint SHA-256 da chave publica de um agente.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fingerprint(pub String);

/// Identidade criptografica de um agente (AID-001).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Fingerprint SHA-256 da chave publica (identificador portavel).
    pub fingerprint: Fingerprint,
    /// Chave publica Ed25519 (32 bytes crus, RSA-free — RFC 8032/VAIP).
    pub public_key: [u8; 32],
    /// Timestamp de criacao (UTC).
    pub created_at: DateTime<Utc>,
    /// TTL de expiracao — `None` = agente persistente; `Some` = efemero.
    pub ttl: Option<chrono::Duration>,
}

impl AgentIdentity {
    /// Cria uma identidade PERSISTENTE (sinonimo de `AgentIdentity::new`).
    pub fn new() -> (Self, SigningKey) {
        let signing_key = SigningKey::generate(&mut OsRng);
        let public_key = signing_key.verifying_key().to_bytes();
        let fingerprint = Fingerprint::of(&public_key);
        let identity = Self {
            fingerprint,
            public_key,
            created_at: Utc::now(),
            ttl: None,
        };
        (identity, signing_key)
    }

    /// Cria uma identidade EFEMERA com TTL — expira automaticamente apos
    /// `ttl` a partir de `created_at` (ci/CD, tarefas one-off; AID-006 parcial).
    pub fn ephemeral(ttl: chrono::Duration) -> (Self, SigningKey) {
        let (mut identity, signing_key) = Self::new();
        identity.ttl = Some(ttl);
        (identity, signing_key)
    }

    /// Verifica `signature` sobre `message` com a chave publica desta identidade.
    ///
    /// Zero panics: qualquer falha de decodificacao (chave/assinatura invalida)
    /// resulta em `false`, nunca em `unwrap`.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        let Ok(pk) = VerifyingKey::from_bytes(&self.public_key) else {
            return false;
        };
        let Ok(sig) = Signature::from_slice(signature) else {
            return false;
        };
        Verifier::verify(&pk, message, &sig).is_ok()
    }

    /// Assina via `SigningKey` (atende a assinatura de event—via trait `Signer`).
    /// A chave privada e usada apenas no ponto de assinatura; nao e serializada.
    pub fn sign(signing_key: &SigningKey, message: &[u8]) -> Vec<u8> {
        signing_key.sign(message).to_bytes().to_vec()
    }

    /// `true` se a identidade efemera ja expirou (AID-006 parcial).
    pub fn is_expired(&self) -> bool {
        match self.ttl {
            Some(ttl) => Utc::now() > self.created_at + ttl,
            None => false,
        }
    }

    /// Derivacao do identificador did:key (W3C) — base58btc sobre o
    /// multicodec 0xED01 + chave publica (Esquema: `did:key:z<...>`).
    pub fn did_key(&self) -> String {
        let mut buf = [0u8; 34];
        buf[0] = 0xED;
        buf[1] = 0x01;
        buf[2..].copy_from_slice(&self.public_key);
        format!("did:key:z{}", bs58::encode(&buf).into_string())
    }
}

impl Fingerprint {
    /// Computa o fingerprint SHA-256 da chave publica (32 bytes crus).
    pub fn of(public_key: &[u8; 32]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(public_key);
        Fingerprint(format!("{KEY_PKCS8_PREFIX}{}", hex::encode(hasher.finalize())))
    }

    /// Recomputa e compara com o fingerprint armazenado (AID-003 ver_DICTA)+
    pub fn matches(&self, public_key: &[u8; 32]) -> bool {
        *self == Self::of(public_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer;

    #[test]
    fn new_generates_distinct_fingerprints() {
        let (a, _) = AgentIdentity::new();
        let (b, _) = AgentIdentity::new();
        assert_ne!(a.fingerprint, b.fingerprint);
        assert_ne!(a.public_key, b.public_key);
    }

    #[test]
    fn verify_roundtrip_and_tamper() {
        let (identity, key) = AgentIdentity::new();
        let msg = b"mensagem de acao";
        let sig = key.sign(msg);
        assert!(identity.verify(msg, &sig.to_bytes()));
        assert!(!identity.verify(b"outra mensagem", &sig.to_bytes()));
        let mut bad = sig.to_bytes();
        bad[0] ^= 0xFF;
        assert!(!identity.verify(msg, &bad));
    }

    #[test]
    fn verify_malformed_inputs_are_false_not_panic() {
        let (identity, key) = AgentIdentity::new();
        let msg = b"x";
        assert!(!identity.verify(msg, &[0u8; 63]));
        let bad_pk = AgentIdentity {
            public_key: [0u8; 32],
            created_at: identity.created_at,
            ttl: None,
            fingerprint: Fingerprint::of(&[0u8; 32]),
        };
        assert!(!bad_pk.verify(msg, &key.sign(msg).to_bytes()));
    }

    #[test]
    fn fingerprint_matches_recomputation() {
        let (identity, key) = AgentIdentity::new();
        let _ = key;
        assert!(identity.fingerprint.matches(&identity.public_key));
        assert!(!identity.fingerprint.matches(&[0u8; 32]));
        assert!(identity.fingerprint.0.starts_with(KEY_PKCS8_PREFIX));
        assert_eq!(identity.fingerprint.0.len(), KEY_PKCS8_PREFIX.len() + 64);
    }

    #[test]
    fn static_signing_api_compatible() {
        let (_identity, key) = AgentIdentity::new();
        let sig = AgentIdentity::sign(&key, b"data");
        assert_eq!(sig.len(), 64);
        // chave nativa ainda funciona para verificar lacos:
        assert!(key.verify(b"data", &Signature::from_slice(&sig).unwrap()).is_ok());
    }

    #[test]
    fn ephemeral_expires_after_ttl() {
        let (identity, _) = AgentIdentity::ephemeral(chrono::Duration::milliseconds(1));
        assert!(!identity.is_expired());
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(identity.is_expired());
    }

    #[test]
    fn persistent_never_expires() {
        let (identity, _) = AgentIdentity::new();
        assert!(!identity.is_expired());
    }

    #[test]
    fn did_key_format_multicodec() {
        let (identity, _) = AgentIdentity::new();
        let did = identity.did_key();
        assert!(did.starts_with("did:key:z"));
        let payload = &did["did:key:z".len()..];
        let decoded = bs58::decode(payload).into_vec().unwrap();
        assert_eq!(decoded.len(), 34);
        assert_eq!(decoded[0], 0xED);
        assert_eq!(decoded[1], 0x01);
        assert_eq!(&decoded[2..], &identity.public_key[..]);
    }
}