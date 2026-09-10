//! Segregacao de funcoes (AID-010).
//!
//! Um unico agente nao pode executar as duas metades de uma operacao
//! sensivel (ex: pai o pagamento E aprova-lo). Cada etapa da operacao tem UM
//! agente autorizado; um agente que ja executou qualquer etapa da operacao
//! fica impedido de executar outra — independentemente de ser autorizado.

use serde::{Deserialize, Serialize};

/// Erro de violacao de SoD (AID-010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SodError {
    /// A etapa nao existe na operacao.
    UnknownStep,
    /// O agente nao e o autorizado para esta etapa.
    NotAuthorized,
    /// O agente ja executou outra etapa desta operacao (violacao de SoD).
    DutyConflict,
}

impl std::fmt::Display for SodError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SodError::UnknownStep => write!(f, "etapa desconhecida"),
            SodError::NotAuthorized => write!(f, "agente nao autorizado para a etapa"),
            SodError::DutyConflict => write!(f, "segregacao de funcoes violada"),
        }
    }
}

impl std::error::Error for SodError {}

/// Uma etapa de uma operacao sensivel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStep {
    /// Nome da etapa (chave de referencia dentro da operacao).
    pub name: String,
    /// Fingerprint do agente UNICO autorizado para esta etapa.
    pub authorized_agent: String,
}

/// Operacao sensivel decomposta em etapas, com auditoria de execucao.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveOperation {
    /// ID da operacao (ex: `payment-1`).
    pub id: String,
    /// Etapas da operacao (ordem de execucao).
    pub steps: Vec<OperationStep>,
    /// Registro de execucao (etapa -> fingerprint). Vec preserva ordem canonica.
    pub executed_by: Vec<(String, String)>,
}

impl SensitiveOperation {
    /// Tenta registrar a execucao da etapa por um agente, aplicando as regras
    /// de SoD. Retorna `Ok(())` e registra, ou `Err(SodError)` sem efeito.
    pub fn try_record(
        &mut self,
        agent_fingerprint: &str,
        step_name: &str,
    ) -> Result<(), SodError> {
        // 1. Passo existe?
        let step = self
            .steps
            .iter()
            .find(|s| s.name == step_name)
            .ok_or(SodError::UnknownStep)?;

        // 2. Agente e o autorizado da etapa?
        if step.authorized_agent != agent_fingerprint {
            return Err(SodError::NotAuthorized);
        }

        // 3. Agente ja executou outra etapa desta operacao?
        if self
            .executed_by
            .iter()
            .any(|(_, a)| a == agent_fingerprint)
        {
            return Err(SodError::DutyConflict);
        }

        self.executed_by
            .push((step_name.to_string(), agent_fingerprint.to_string()));
        Ok(())
    }

    /// `true` se o agente pode (ainda pode) executar a etapa (regras de SoD).
    pub fn can_execute(&self, agent_fingerprint: &str, step_name: &str) -> bool {
        self.steps.iter().any(|s| s.name == step_name)
            && self
                .steps
                .iter()
                .find(|s| s.name == step_name)
                .is_some_and(|s| s.authorized_agent == agent_fingerprint)
            && !self
                .executed_by
                .iter()
                .any(|(_, a)| a == agent_fingerprint)
    }

    /// `true` se todas as etapas da operacao ja foram executadas.
    pub fn is_complete(&self) -> bool {
        self.steps
            .iter()
            .all(|s| self.executed_by.iter().any(|(step, _)| step == &s.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payment_op() -> SensitiveOperation {
        SensitiveOperation {
            id: "payment-1".to_string(),
            steps: vec![
                OperationStep {
                    name: "create_payment".to_string(),
                    authorized_agent: "agent-a".to_string(),
                },
                OperationStep {
                    name: "approve_payment".to_string(),
                    authorized_agent: "agent-b".to_string(),
                },
            ],
            executed_by: Vec::new(),
        }
    }

    #[test]
    fn two_agents_complete_operation() {
        let mut op = payment_op();
        assert!(op.try_record("agent-a", "create_payment").is_ok());
        assert!(op.try_record("agent-b", "approve_payment").is_ok());
        assert!(op.is_complete());
    }

    #[test]
    fn same_agent_cannot_execute_both_steps() {
        let mut op = payment_op();
        assert!(op.try_record("agent-a", "create_payment").is_ok());
        // agent-a NAO e o autorizado de approve -> negado por autorizacao antes
        // do conflito; o efeito de negacao e o mesmo (SoD impossibilita a acao).
        let err = op.try_record("agent-a", "approve_payment").unwrap_err();
        assert_eq!(err, SodError::NotAuthorized);
        assert!(!op.can_execute("agent-a", "approve_payment"));
        assert!(!op.is_complete());
    }

    #[test]
    fn duty_conflict_when_authorized_for_two_steps() {
        // Config malformada: o mesmo agente autorizado para as DUAS metades.
        // A segunda tentativa viola o SoD MESMO com autorizacao valida.
        let mut op = SensitiveOperation {
            id: "payment-2".to_string(),
            steps: vec![
                OperationStep {
                    name: "create".to_string(),
                    authorized_agent: "agent-x".to_string(),
                },
                OperationStep {
                    name: "approve".to_string(),
                    authorized_agent: "agent-x".to_string(),
                },
            ],
            executed_by: Vec::new(),
        };
        assert!(op.try_record("agent-x", "create").is_ok());
        let err = op.try_record("agent-x", "approve").unwrap_err();
        assert_eq!(err, SodError::DutyConflict);
        assert!(!op.is_complete());
    }

    #[test]
    fn unauthorized_agent_rejected() {
        let mut op = payment_op();
        let err = op
            .try_record("agent-c", "create_payment")
            .unwrap_err();
        assert_eq!(err, SodError::NotAuthorized);
        assert!(!op.can_execute("agent-c", "create_payment"));
    }

    #[test]
    fn unknown_step_rejected() {
        let mut op = payment_op();
        let err = op.try_record("agent-a", "transfer").unwrap_err();
        assert_eq!(err, SodError::UnknownStep);
    }

    #[test]
    fn agent_a_cannot_create_twice() {
        let mut op = payment_op();
        assert!(op.try_record("agent-a", "create_payment").is_ok());
        let err = op.try_record("agent-a", "create_payment").unwrap_err();
        assert_eq!(err, SodError::DutyConflict);
    }

    #[test]
    fn no_fabricated_reuse_of_own_step() {
        // Um agente so pode executar a etapa para a qual foi autorizado, uma vez.
        let mut op = payment_op();
        assert!(op.can_execute("agent-a", "create_payment"));
        assert!(op.try_record("agent-a", "create_payment").is_ok());
        assert!(!op.can_execute("agent-a", "create_payment"));
        assert!(!op.can_execute("agent-b", "create_payment"));
        assert!(!op.can_execute("agent-a", "approve_payment"));
    }
}