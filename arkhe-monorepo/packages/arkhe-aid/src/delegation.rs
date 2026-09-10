//! Delegacao atenuada (AID-002).
//!
//! Em uma cadeia de delegacao, cada salto pode apenas ESTREITAR permissoes,
//! nunca amplia-las — attenuacao criptograficamente imposta: `scopes` do
//! delegado e um subconjunto dos escopos do delegador, e a delegacao e
//! assinada pela chave privada do delegador.
//!
//! Os sete escopos hierarquicos seguem o VAIP (permission scopes):
//! Read < Write < Execute < Transact < Communicate < Delegate < Elevate.
//! A verificacao de subconjunto usa a nocao de precedencia hierarquica:
//! possuir `Write` implica `Read`, `Execute` implica `Write`, etc.
//! (a hierarquia e um orden parcial estrito sobre a lista ordenada acima).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::identity::{AgentIdentity, Fingerprint};

/// Escopo de permissao (VAIP, setes escopos hierarquicos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Scope {
    Read,
    Write,
    Execute,
    Transact,
    Communicate,
    Delegate,
    Elevate,
}

/// Erro de parse de escopo textual (vetor Simplicity-2, sem dep extra).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeParseError(pub String);

impl std::fmt::Display for ScopeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "escopo desconhecido: {}", self.0)
    }
}

impl std::error::Error for ScopeParseError {}

impl std::str::FromStr for Scope {
    type Err = ScopeParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "read" => Ok(Scope::Read),
            "write" => Ok(Scope::Write),
            "execute" => Ok(Scope::Execute),
            "transact" => Ok(Scope::Transact),
            "communicate" => Ok(Scope::Communicate),
            "delegate" => Ok(Scope::Delegate),
            "elevate" => Ok(Scope::Elevate),
            _ => Err(ScopeParseError(s.to_string())),
        }
    }
}

impl Scope {
    /// Ordem hierarquica [0..6]; escopo hierarquicamente mais alto tem ordem maior.
    pub fn rank(self) -> u8 {
        match self {
            Scope::Read => 0,
            Scope::Write => 1,
            Scope::Execute => 2,
            Scope::Transact => 3,
            Scope::Communicate => 4,
            Scope::Delegate => 5,
            Scope::Elevate => 6,
        }
    }
}

/// Delegacao de permissoes do delegador para o delegado (assinada na fonte).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delegation {
    /// Fingerprint do delegador (quem concede).
    pub delegator: Fingerprint,
    /// Fingerprint do delegado (quem recebe).
    pub delegate: Fingerprint,
    /// Escopos delegados (⊆ escopos do delegador — AID-002).
    pub scopes: Vec<Scope>,
    /// Timestamp de emissao (UTC).
    pub issued_at: DateTime<Utc>,
    /// Expiracao (`None` = sem expiracao).
    pub expires_at: Option<DateTime<Utc>>,
    /// Assinatura Ed25519 do delegador sobre a forma canonica.
    pub signature: Vec<u8>,
}

impl Delegation {
    /// Cria e assina uma delegacao de `delegator` para `delegate` com `scopes`.
    ///
    /// # Panics
    /// Nao ha panic: a assinatura e registrada via `AgentIdentity::sign`.
    pub fn new(
        delegator: &AgentIdentity,
        delegate: &AgentIdentity,
        scopes: Vec<Scope>,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Self {
        let mut d = Self {
            delegator: delegator.fingerprint.clone(),
            delegate: delegate.fingerprint.clone(),
            scopes,
            issued_at: Utc::now(),
            expires_at: None,
            signature: Vec::new(),
        };
        d.signature = crate::identity::AgentIdentity::sign(
            signing_key,
            &d.canonical_message(),
        );
        d
    }

    /// Verifica a assinatura do delegador sobre a forma canonica (AID-002).
    pub fn verify(&self, delegator: &AgentIdentity) -> bool {
        delegator.verify(&self.canonical_message(), &self.signature)
    }

    /// Atenuacao: todo escopo delegado esta coberto pelos escopos do delegador,
    /// considerando a hierarquia (possuir X implica todos X' com rank menor).
    pub fn is_attenuated(&self, delegator_scopes: &[Scope]) -> bool {
        self.scopes.iter().all(|s| {
            delegator_scopes
                .iter()
                .any(|h| h.rank() >= s.rank())
        })
    }

    /// Subconjunto ESTRITO (sem hierarquia): usado nas provas formais do nucleo
    /// Lean (a cadeia nao amplia mesmo quando a hierarquia nao e invocada).
    pub fn is_strict_subset(&self, delegator_scopes: &[Scope]) -> bool {
        self.scopes.iter().all(|s| delegator_scopes.contains(s))
    }

    /// `true` se a delegacao ja expirou.
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(exp) => Utc::now() > exp,
            None => false,
        }
    }

    /// Forma canonica deterministica (ordem fixa de campos, representacao canonica).
    /// Esquema (hex e separada por `|`): `delegator|delegate|scopes|issued|<exp>`.
    pub fn canonical_message(&self) -> Vec<u8> {
        let scopes = self
            .scopes
            .iter()
            .map(|s| format!("{:?}", s))
            .collect::<Vec<_>>()
            .join(",");
        let issued = self.issued_at.timestamp_nanos_opt().unwrap_or(0);
        let exp = self
            .expires_at
            .map(|t| t.timestamp_nanos_opt().unwrap_or(0))
            .map(|t| t.to_string())
            .unwrap_or_else(|| "-".to_string());
        format!(
            "{}|{}|[{}]|{}|{}",
            self.delegator.0, self.delegate.0, scopes, issued, exp
        )
        .into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attenuation_ok_when_subset() {
        let (delegator, delegator_key) = AgentIdentity::new();
        let (delegate, _) = AgentIdentity::new();
        let d = Delegation::new(&delegator, &delegate, vec![Scope::Read], &delegator_key);
        assert!(d.is_attenuated(&[Scope::Read, Scope::Write, Scope::Execute]));
        assert!(d.is_strict_subset(&[Scope::Read, Scope::Write]));
        assert!(d.verify(&delegator));
    }

    #[test]
    fn attenuation_fails_when_widening() {
        let (delegator, delegator_key) = AgentIdentity::new();
        let (delegate, _) = AgentIdentity::new();
        let d = Delegation::new(
            &delegator,
            &delegate,
            vec![Scope::Read, Scope::Write],
            &delegator_key,
        );
        assert!(!d.is_attenuated(&[Scope::Read]));
        assert!(!d.is_strict_subset(&[Scope::Read]));
    }

    #[test]
    fn hierarchy_gives_read_from_write() {
        let (delegator, delegator_key) = AgentIdentity::new();
        let (delegate, _) = AgentIdentity::new();
        let d = Delegation::new(&delegator, &delegate, vec![Scope::Read], &delegator_key);
        // write >= read pela hierarquia VAIP (sem subconjunto estrito)
        assert!(d.is_attenuated(&[Scope::Write]));
        assert!(!d.is_strict_subset(&[Scope::Write]));
    }

    #[test]
    fn delegation_signature_is_binding() {
        let (delegator, delegator_key) = AgentIdentity::new();
        let (other, _) = AgentIdentity::new();
        let (delegate, _) = AgentIdentity::new();
        let d = Delegation::new(&delegator, &delegate, vec![Scope::Read], &delegator_key);
        assert!(d.verify(&delegator));
        assert!(!d.verify(&other));
    }

    #[test]
    fn delegation_expires() {
        let (delegator, delegator_key) = AgentIdentity::new();
        let (delegate, _) = AgentIdentity::new();
        let mut d = Delegation::new(&delegator, &delegate, vec![Scope::Read], &delegator_key);
        assert!(!d.is_expired());
        d.expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        assert!(d.is_expired());
    }

    #[test]
    fn scope_parse_roundtrip() {
        let order = [
            Scope::Read,
            Scope::Write,
            Scope::Execute,
            Scope::Transact,
            Scope::Communicate,
            Scope::Delegate,
            Scope::Elevate,
        ];
        for s in order.into_iter().take(7) {
            let txt = format!("{:?}", s).to_lowercase();
            assert_eq!(txt.parse::<Scope>().unwrap(), s);
        }
        assert!("transmute".parse::<Scope>().is_err());
    }

    #[test]
    fn canonical_message_deterministic() {
        let (delegator, delegator_key) = AgentIdentity::new();
        let (delegate, _) = AgentIdentity::new();
        let d = Delegation::new(&delegator, &delegate, vec![Scope::Read], &delegator_key);
        assert_eq!(d.canonical_message(), d.canonical_message());
        let mut d2 = d.clone();
        d2.scopes.push(Scope::Write);
        assert_ne!(d.canonical_message(), d2.canonical_message());
    }
}