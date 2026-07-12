//! Gerenciamento de chaves e recuperação social.
//!
//! `∀ key, key_protected(key) ∧ key_backed_up(key) ∧ key_rotated(key, max_age)`

use crate::InvariantVerdict;
use chrono::{DateTime, Duration, Utc};

/// Metadados de uma chave gerenciada.
#[derive(Debug, Clone)]
pub struct KeyMetadata {
    pub created_at: DateTime<Utc>,
    pub backed_up: bool,
    pub hardware_protected: bool,
}

/// Verifica rotação: a chave não pode exceder `max_age`.
pub fn check_key_rotation(key: &KeyMetadata, max_age: Duration, now: DateTime<Utc>) -> InvariantVerdict {
    if !key.backed_up {
        return InvariantVerdict::violated("key has no backup");
    }
    let age = now - key.created_at;
    if age > max_age {
        return InvariantVerdict::violated(format!(
            "key age {}s exceeds max {}s",
            age.num_seconds(),
            max_age.num_seconds()
        ));
    }
    InvariantVerdict::Holds
}

/// Quórum de recuperação social: número de guardiões que aprovaram uma
/// recuperação de conta deve atingir o limiar configurado.
pub fn check_recovery_quorum(approvals: usize, threshold: usize, total_guardians: usize) -> InvariantVerdict {
    if threshold == 0 || threshold > total_guardians {
        return InvariantVerdict::violated("invalid recovery threshold configuration");
    }
    if approvals >= threshold {
        InvariantVerdict::Holds
    } else {
        InvariantVerdict::violated(format!(
            "only {approvals}/{threshold} guardian approvals collected"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_backed_up_key_holds() {
        let now = Utc::now();
        let key = KeyMetadata { created_at: now, backed_up: true, hardware_protected: true };
        assert!(check_key_rotation(&key, Duration::days(365), now).holds());
    }

    #[test]
    fn stale_key_is_rejected() {
        let now = Utc::now();
        let key = KeyMetadata {
            created_at: now - Duration::days(400),
            backed_up: true,
            hardware_protected: true,
        };
        assert!(!check_key_rotation(&key, Duration::days(365), now).holds());
    }

    #[test]
    fn key_without_backup_is_rejected() {
        let now = Utc::now();
        let key = KeyMetadata { created_at: now, backed_up: false, hardware_protected: true };
        assert!(!check_key_rotation(&key, Duration::days(365), now).holds());
    }

    #[test]
    fn quorum_met_holds() {
        assert!(check_recovery_quorum(3, 3, 5).holds());
    }

    #[test]
    fn quorum_not_met_is_rejected() {
        assert!(!check_recovery_quorum(2, 3, 5).holds());
    }
}
