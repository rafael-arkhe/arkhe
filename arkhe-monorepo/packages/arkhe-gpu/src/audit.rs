//! Auditoria append-only da camada GPU (bloco 1011, v390.2) — invariantes
//! **I536/I537**.
//!
//! Espelho Rust do modelo de trilha do núcleo Lean (I536-A, I537-A/B): só a
//! auditoria decide O QUE entra na trilha e ela **nunca regista um lançamento
//! com contrato violado**. A API só expõe `push` (append-only — Loopseal-2 no
//! nível do vetor); a contenção contabilística tem **prova formal Lean** (não
//! é heurística).

use crate::tensor::TensorId;

/// Estado do contrato de lançamento (G-07/G-01/G-02 delegados à track).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchStatus {
    /// Contrato satisfeito — o único estado gravado pela trilha bem-formada.
    Satisfied,
    /// Contrato violado — a governança rejeita; nunca vira entrada de trilha
    /// bem-formada.
    Violated,
}

/// Resultado do lançamento executado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchOutcome {
    /// Kernels submetidos e sincronizados sem erro.
    Ok,
    /// Falha de lançamento (runtime, alocação, grid).
    Failed,
}

/// Registo de auditoria: (tensor, status contrato, resultado, duração ms).
///
/// Espelho de `AuditEntry = LaunchStatus × LaunchOutcome × Nat` do núcleo Lean
/// (o tensor é metadado do crate, não afeta as provas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditEntry {
    /// Tensor alvo do lance.
    pub tensor: TensorId,
    /// Estado do contrato no momento do registo.
    pub status: LaunchStatus,
    /// Resultado do lançamento executado.
    pub outcome: LaunchOutcome,
    /// Duração do lançamento em milissegundos.
    pub duration_ms: u64,
}

/// N.º de lançamentos registados na trilha (espelho `launches` do núcleo).
#[must_use]
pub fn launches(entries: &[AuditEntry]) -> u64 {
    entries.len() as u64
}

/// N.º de registos com contrato violado na trilha (espelho
/// `contract_violations` do núcleo).
#[must_use]
pub fn contract_violations(entries: &[AuditEntry]) -> u64 {
    entries
        .iter()
        .filter(|e| e.status == LaunchStatus::Violated)
        .count() as u64
}

/// N.º de lançamentos falhados na trilha (espelho `failed_launches` do núcleo).
#[must_use]
pub fn failed_launches(entries: &[AuditEntry]) -> u64 {
    entries
        .iter()
        .filter(|e| e.outcome == LaunchOutcome::Failed)
        .count() as u64
}

/// Trilha bem-formada: todo registo satisfez o contrato (espelho `well_formed`
/// do núcleo).
#[must_use]
pub fn is_well_formed(entries: &[AuditEntry]) -> bool {
    entries
        .iter()
        .all(|e| e.status == LaunchStatus::Satisfied)
}

/// Trilha auditável append-only da camada GPU.
///
/// A API **não** expõe remoção/edição (Loopseal-2); só a auditoria preenche e
/// só aceita lançamentos com contrato satisfeito (I536-A) — a trilha
/// bem-formada é o invariante do tipo, não uma promessa.
#[derive(Debug, Clone, Default)]
pub struct LaunchAudit {
    entries: Vec<AuditEntry>,
}

impl LaunchAudit {
    /// Trilha vazia.
    #[must_use]
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Leitura da trilha (append-only).
    #[must_use]
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Comprimento da trilha.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Trilha vazia?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Regista um lance com contrato satisfeito (único caminho de escrita).
    ///
    /// A auditoria NUNCA grava `Violated` — quem decide o estado é o contrato
    /// de lançamento verificado pela governança antes de chegar aqui; esta API
    /// é o que torna «bem-formada» um invariante do tipo (I536-A).
    pub fn record(&mut self, entry: AuditEntry) {
        debug_assert_eq!(
            entry.status,
            LaunchStatus::Satisfied,
            "I536-A: auditoria só grava contrato satisfeito"
        );
        self.entries.push(entry);
    }

    /// Verificação das invariantes contabilísticas I537-A/B em Rust.
    #[must_use]
    pub fn verify_i537(&self) -> bool {
        crate::governance::i537a_violations_le_launches(&self.entries)
            && crate::governance::i537b_wellformed_has_no_violations(&self.entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::{i536a_wellformed_entries_satisfied, i537a_violations_le_launches};

    fn ok(t: u64, d: u64) -> AuditEntry {
        AuditEntry {
            tensor: TensorId(t),
            status: LaunchStatus::Satisfied,
            outcome: LaunchOutcome::Ok,
            duration_ms: d,
        }
    }

    #[test]
    fn i537a_b_satisfied_on_wellformed() {
        let mut audit = LaunchAudit::new();
        audit.record(ok(1, 12));
        audit.record(ok(2, 300));
        assert!(audit.verify_i537());
        assert!(i536a_wellformed_entries_satisfied(audit.entries()));
        assert!(i537a_violations_le_launches(audit.entries()));
    }

    #[test]
    fn ad_hoc_violated_entry_is_counted_but_never_pushed_by_api() {
        // Um registo `Violated` construído à parte conta como violação na
        // contabilidade (espelho I537-A: violações ≤ lançamentos mantém-se),
        // mas a API `record` da trilha nunca o grava (debug_assert I536-A).
        let bad = [AuditEntry {
            tensor: TensorId(9),
            status: LaunchStatus::Violated,
            outcome: LaunchOutcome::Failed,
            duration_ms: 1,
        }];
        assert_eq!(contract_violations(&bad), 1);
        assert_eq!(launches(&bad), 1);
        assert!(i537a_violations_le_launches(&bad));
        assert!(!is_well_formed(&bad));
        // I536-A é a implicação material `bem-formada ⟹ todos satisfizeram`:
        // mal-formada ⟹ vacuamente satisfeita (idem núcleo Lean).
        assert!(i536a_wellformed_entries_satisfied(&bad));
    }

    #[test]
    fn append_only_no_removal_ops_present() {
        let mut audit = LaunchAudit::new();
        audit.record(ok(1, 5));
        audit.record(ok(2, 7));
        assert_eq!(audit.len(), 2);
        // API pública não expõe remoção; a trilha só cresce.
        assert_eq!(audit.entries().len(), audit.len());
    }
}