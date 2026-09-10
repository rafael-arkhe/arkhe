//! Auditoria assinada NA FONTE (AID-003) com hash chain (append-only).
//!
//! Cada evento de auditoria e assinado com a chave privada do agente no
//! momento em que e produzido — nao se confia em um log central posterior
//! ("per-event signing at source" do VAIP). O hash do evento anterior
//! (`prev_hash`) encadeia os eventos, tornando o ledger imodificavel por
//! insercao/remocao no meio (Ghost-1/Loopseal-2).
//!
//! A mensagem canonicamente assinada usa ordem FIXA de campos (AID-007
//! parcial — serializacao deterministica, sem dep externa de canonicalizacao).

use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::identity::{AgentIdentity, Fingerprint};

/// Separador de campos da forma canonica (hex).
pub const AUDIT_FIELD_SEP: char = '|';

/// Evento de auditoria assinado na fonte (AID-003).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedAuditEvent {
    /// Fingerprint do agente que executou e assinou.
    pub agent_fingerprint: Fingerprint,
    /// Acao executada.
    pub action: String,
    /// Recurso alvo da acao.
    pub resource: String,
    /// Resultado (ex: `success`, `denied`).
    pub result: String,
    /// Timestamp de producao (UTC).
    pub timestamp: DateTime<Utc>,
    /// Hash do contexto (input hash, versao do modelo, catalogo de ferramentas).
    pub context_hash: String,
    /// Hash SHA-256 do evento imediatamente anterior (hash chain; `None` = genesis).
    pub prev_hash: Option<String>,
    /// Assinatura Ed25519 do agente sobre a forma canonica do evento.
    pub signature: Vec<u8>,
}

impl SignedAuditEvent {
    /// Cria, assina e retorna um evento encadeado ao anterior (`prev_hash`).
    pub fn new(
        identity: &AgentIdentity,
        signing_key: &SigningKey,
        action: &str,
        resource: &str,
        result: &str,
        context_hash: &str,
        prev_hash: Option<String>,
    ) -> Self {
        let mut event = Self {
            agent_fingerprint: identity.fingerprint.clone(),
            action: action.to_string(),
            resource: resource.to_string(),
            result: result.to_string(),
            timestamp: Utc::now(),
            context_hash: context_hash.to_string(),
            prev_hash,
            signature: Vec::new(),
        };
        let msg = event.canonical_message();
        event.signature = crate::identity::AgentIdentity::sign(signing_key, &msg);
        event
    }

    /// Verifica a assinatura do evento com a identidade do agente (AID-003).
    /// Na forma canonica, `signature` NAO participa (evita auto-referencia).
    pub fn verify(&self, identity: &AgentIdentity) -> bool {
        identity.verify(&self.canonical_message(), &self.signature)
    }

    /// Hash SHA-256 da forma canonica (para encadeamento / ledger).
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.canonical_message());
        format!("sha256:{}", hex::encode(hasher.finalize()))
    }

    /// `true` se este evento encadeia diretamente em `previous`
    /// (o `prev_hash` deste evento == hash do evento anterior) — Ghost-1 local.
    pub fn follows(&self, previous: &SignedAuditEvent) -> bool {
        self.prev_hash.as_deref() == Some(previous.compute_hash().as_str())
    }

    /// `true` para o evento genesis (sem predecessor).
    pub fn is_genesis(&self) -> bool {
        self.prev_hash.is_none()
    }

    /// Forma canonica deterministica (ordem fixa, excluindo a assinatura).
    /// Esquema (hex): `fp|action|resource|result|timestamp|ctx|prev`.
    pub fn canonical_message(&self) -> Vec<u8> {
        let ts = self.timestamp.timestamp_nanos_opt().unwrap_or(0);
        let prev = self.prev_hash.as_deref().unwrap_or("-");
        format!(
            "{}{}{}{}{}{}{}{}{}{}{}{}{}",
            self.agent_fingerprint.0,
            AUDIT_FIELD_SEP,
            self.action,
            AUDIT_FIELD_SEP,
            self.resource,
            AUDIT_FIELD_SEP,
            self.result,
            AUDIT_FIELD_SEP,
            ts,
            AUDIT_FIELD_SEP,
            self.context_hash,
            AUDIT_FIELD_SEP,
            prev,
        )
        .into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> String {
        "ctx-hash-123".to_string()
    }

    #[test]
    fn event_signs_and_verifies() {
        let (identity, key) = AgentIdentity::new();
        let e = SignedAuditEvent::new(&identity, &key, "read", "r1", "success", &ctx(), None);
        assert!(e.is_genesis());
        assert!(e.verify(&identity));
        assert!(e.compute_hash().starts_with("sha256:"));
    }

    #[test]
    fn tampered_event_fails_verification() {
        let (identity, key) = AgentIdentity::new();
        let mut e = SignedAuditEvent::new(
            &identity,
            &key,
            "read",
            "r1",
            "success",
            &ctx(),
            None,
        );
        e.action = "write".to_string();
        assert!(!e.verify(&identity));
    }

    #[test]
    fn wrong_identity_fails_verification() {
        let (identity, key) = AgentIdentity::new();
        let (other, _) = AgentIdentity::new();
        let e = SignedAuditEvent::new(&identity, &key, "read", "r1", "success", &ctx(), None);
        assert!(!e.verify(&other));
    }

    #[test]
    fn hash_chain_follows() {
        let (identity, key) = AgentIdentity::new();
        let g = SignedAuditEvent::new(&identity, &key, "init", "sys", "success", &ctx(), None);
        let e2 = SignedAuditEvent::new(
            &identity,
            &key,
            "read",
            "r1",
            "success",
            &ctx(),
            Some(g.compute_hash()),
        );
        let e3 = SignedAuditEvent::new(
            &identity,
            &key,
            "write",
            "r2",
            "success",
            &ctx(),
            Some(e2.compute_hash()),
        );
        assert!(e2.follows(&g));
        assert!(e3.follows(&e2));
        assert!(!e3.follows(&g));
        assert!(!e2.follows(&e3));
    }

    #[test]
    fn tampered_previous_breaks_chain() {
        let (identity, key) = AgentIdentity::new();
        let old_genesis = SignedAuditEvent::new(
            &identity,
            &key,
            "init",
            "sys",
            "success",
            &ctx(),
            None,
        );
        let linked = SignedAuditEvent::new(
            &identity,
            &key,
            "read",
            "r1",
            "success",
            &ctx(),
            Some(old_genesis.compute_hash()),
        );
        assert!(linked.follows(&old_genesis));

        // Mutacao do "genesis" depois de assinado: a cadeia antiga apontava para
        // o hash ORIGINAL de `success`; o novo hash de `denied` difere, e um
        // verificador que reconstrua a partir do topo mutado nao fecha a cadeia.
        let mut mutated = old_genesis.clone();
        mutated.result = "denied".to_string();
        let recomputed_linked = SignedAuditEvent::new(
            &identity,
            &key,
            "read",
            "r1",
            "success",
            &ctx(),
            Some(mutated.compute_hash()),
        );
        assert_ne!(mutated.compute_hash(), old_genesis.compute_hash());
        assert!(!linked.follows(&mutated));
        assert!(recomputed_linked.follows(&mutated));
    }

    #[test]
    fn canonical_message_excludes_signature() {
        let (identity, key) = AgentIdentity::new();
        let e = SignedAuditEvent::new(&identity, &key, "read", "r1", "success", &ctx(), None);
        let mut e2 = e.clone();
        e2.signature = vec![0u8; 64];
        assert_eq!(e.canonical_message(), e2.canonical_message());
        assert!(e.verify(&identity));
    }

    #[test]
    fn event_hash_distinguishes_fields() {
        let (identity, key) = AgentIdentity::new();
        let a = SignedAuditEvent::new(&identity, &key, "read", "r1", "success", &ctx(), None);
        let b = SignedAuditEvent::new(&identity, &key, "read", "r1", "denied", &ctx(), None);
        assert_ne!(a.compute_hash(), b.compute_hash());
    }

    #[test]
    fn ledger_is_append_only_semantics() {
        let (identity, key) = AgentIdentity::new();
        let mut chain: Vec<SignedAuditEvent> = Vec::new();
        // Append-only: cada evento se apoia no hash do topo anterior (Ghost-1 local).
        let g = SignedAuditEvent::new(&identity, &key, "init", "sys", "success", &ctx(), None);
        chain.push(g);
        for i in 0..10 {
            let prev = chain.last().unwrap().compute_hash();
            let e = SignedAuditEvent::new(
                &identity,
                &key,
                "op",
                &format!("r{i}"),
                "success",
                &ctx(),
                Some(prev),
            );
            assert!(e.follows(chain.last().unwrap()));
            chain.push(e);
        }
        // Integridade: re-encadeia do genesis, verificando cada par adjacente.
        for w in chain.windows(2) {
            assert!(w[1].follows(&w[0]));
            assert!(w[1].verify(&identity));
        }
    }
}