//! Estados epistémicos — barreira de tipo I645.

use serde::{Deserialize, Serialize};

/// Estado epistémico de uma proposição.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EpistemicStatus {
    /// Verificado empiricamente (com test.log).
    Verified(VerifiedStatus),
    /// Verificado apenas estaticamente (sem test.log).
    Unverified(UnverifiedStatus),
}

/// Proposição verificada empiricamente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedStatus {
    pub proposition: String,
    pub test_log_hash: String,
    pub timestamp: u64,
}

/// Proposição verificada apenas estaticamente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnverifiedStatus {
    pub proposition: String,
    pub reason: String,
    pub timestamp: u64,
}

impl EpistemicStatus {
    /// Retorna `true` se o estado for `Verified`.
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified(_))
    }

    /// Promove de `Unverified` para `Verified` com evidência concreta.
    ///
    /// A barreira I645 impede coerção implícita — esta é a única via legítima.
    pub fn promote(self, evidence: &str) -> Result<VerifiedStatus, Self> {
        match self {
            EpistemicStatus::Unverified(u) => Ok(VerifiedStatus {
                proposition: u.proposition,
                test_log_hash: evidence.to_string(),
                timestamp: u64::MAX, // substituído pelo caller
            }),
            EpistemicStatus::Verified(v) => Err(EpistemicStatus::Verified(v)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unverified_nao_e_verified() {
        let u = UnverifiedStatus {
            proposition: "p ≠ 0".into(),
            reason: "sem teste".into(),
            timestamp: 0,
        };
        let status = EpistemicStatus::Unverified(u);
        assert!(!status.is_verified());
    }

    #[test]
    fn verified_e_verified() {
        let v = VerifiedStatus {
            proposition: "p ≠ 0".into(),
            test_log_hash: "abc123".into(),
            timestamp: 1000,
        };
        let status = EpistemicStatus::Verified(v);
        assert!(status.is_verified());
    }

    #[test]
    fn promote_unverified_para_verified() {
        let u = UnverifiedStatus {
            proposition: "a(p) ≠ p³ − p²".into(),
            reason: "estático".into(),
            timestamp: 0,
        };
        let status = EpistemicStatus::Unverified(u);
        let result = status.promote("hash_real");
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v.proposition, "a(p) ≠ p³ − p²");
        assert_eq!(v.test_log_hash, "hash_real");
    }

    #[test]
    fn promote_verified_nao_promove() {
        let v = VerifiedStatus {
            proposition: "p ≠ 0".into(),
            test_log_hash: "abc".into(),
            timestamp: 1000,
        };
        let status = EpistemicStatus::Verified(v);
        let result = status.promote("novo_hash");
        assert!(result.is_err());
    }

    #[test]
    fn serializacao_roundtrip() {
        let u = UnverifiedStatus {
            proposition: "teste".into(),
            reason: "motivo".into(),
            timestamp: 42,
        };
        let status = EpistemicStatus::Unverified(u);
        let json = serde_json::to_string(&status).unwrap();
        let back: EpistemicStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, back);
    }
}
