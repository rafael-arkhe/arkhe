//! [`MockGpuBackend`] — backend GPU em memória, testável sem hardware
//! (bloco 1011, v390.2).
//!
//! Única implementação deste crate. Corrige o mock proposto em v605.0 (que
//! usava `tensors: HashMap` + `self.tensors.lock()` — mutabilidade interior
//! sem `Mutex`): o conteúdo vive em `Mutex<HashMap<…>>` real, os contadores em
//! `Atomic*`, e cada lance autorizado alimenta a trilha auditável append-only
//! com contrato satisfeito (I536-A).

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use crate::audit::{AuditEntry, LaunchAudit, LaunchOutcome, LaunchStatus};
use crate::backend::{BackendError, BackendKind, GpuBackend, LaunchOutcome as BackendOutcome, LaunchPlan};
use crate::governance::TILE_CAP_MS;
use crate::metrics::Metrics;
use crate::tensor::{Shape, TensorId};

/// Backend GPU mock fiel às bandas G-07 e capas do crate.
#[derive(Debug)]
pub struct MockGpuBackend {
    name: &'static str,
    next_id: Mutex<u64>,
    tensors: Mutex<HashMap<u64, Vec<f64>>>,
    metrics: Metrics,
    audit: Mutex<LaunchAudit>,
    fail_next_launch: Mutex<bool>,
}

impl Default for MockGpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MockGpuBackend {
    /// Instância mock nova, com trilha auditável vazia.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: "arkhe-gpu-mock",
            next_id: Mutex::new(1),
            tensors: Mutex::new(HashMap::new()),
            metrics: Metrics::new(),
            audit: Mutex::new(LaunchAudit::new()),
            fail_next_launch: Mutex::new(false),
        }
    }

    fn tensors(&self) -> MutexGuard<'_, HashMap<u64, Vec<f64>>> {
        self.tensors
            .lock()
            .expect("lock de tensores do mock não deve ser envenedo")
    }

    /// Dados do tensor alocado (cópia — para inspeção/testes).
    ///
    /// # Errors
    ///
    /// * [`BackendError::UnknownTensor`] — id não alocado nesta instância.
    pub fn tensor_data(&self, id: TensorId) -> Result<Vec<f64>, BackendError> {
        self.tensors().get(&id.raw()).cloned().ok_or(BackendError::UnknownTensor(id))
    }

    /// Trilha auditável atual (cópia).
    #[must_use]
    pub fn audit(&self) -> Vec<AuditEntry> {
        self.audit.lock().expect("lock de audit do mock").entries().to_vec()
    }

    /// Consome a trilha auditável.
    #[must_use]
    pub fn into_audit(self) -> LaunchAudit {
        self.audit.into_inner().expect("lock de audit do mock")
    }

    /// Métricas agregadas (leitura).
    #[must_use]
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// Força a falha do próximo lance autorizado (para testar
    /// `LaunchOutcome::Failed` com contrato satisfeito — I537 conta o outcome,
    /// não o estado do contrato).
    pub fn fail_next(&mut self) {
        *self.fail_next_launch.lock().expect("lock de fail_next") = true;
    }
}

impl GpuBackend for MockGpuBackend {
    fn name(&self) -> &str {
        self.name
    }

    fn kind(&self) -> BackendKind {
        BackendKind::Mock
    }

    fn allocate(&mut self, shape: Shape) -> Result<TensorId, BackendError> {
        let id = {
            let mut next = self.next_id.lock().expect("lock de next_id");
            let id = *next;
            *next += 1;
            id
        };
        let data = vec![0.0; shape.numel()];
        self.tensors().insert(id, data);
        Ok(TensorId(id))
    }

    fn execute(&mut self, plan: LaunchPlan) -> Result<BackendOutcome, BackendError> {
        let mut tensors = self.tensors();
        let Some(data) = tensors.get_mut(&plan.tensor.raw()) else {
            return Err(BackendError::UnknownTensor(plan.tensor));
        };

        self.metrics.observe_launch();
        // Execução concreta do mock: preenche o tensor com o total de threads
        // da grelha (valor determinístico derivado do plano — prova de vida).
        let total = (plan.threads_per_block * plan.grid) as f64;

        let failed = {
            let mut flag = self.fail_next_launch.lock().expect("lock de fail_next");
            if *flag {
                *flag = false;
                true
            } else {
                false
            }
        };

        let outcome = if failed {
            self.metrics.observe_failure();
            BackendOutcome::Failed
        } else {
            for v in data.iter_mut() {
                *v = total;
            }
            BackendOutcome::Ok
        };
        drop(tensors);

        // A trilha auditável regista o lance com contrato satisfeito (a
        // governança autorizou; a falha é do outcome, não do contrato).
        self.audit
            .lock()
            .expect("lock de audit do mock")
            .record(AuditEntry {
                tensor: plan.tensor,
                status: LaunchStatus::Satisfied,
                outcome: if failed { LaunchOutcome::Failed } else { LaunchOutcome::Ok },
                duration_ms: plan.duration_ms,
            });
        Ok(outcome)
    }

    fn track_cap_ms(&self) -> u64 {
        TILE_CAP_MS
    }

    fn sync(&mut self) -> Result<(), BackendError> {
        let _t = self.tensors();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::LaunchPlan;
    use crate::governance::{
        i536a_wellformed_entries_satisfied, i537a_violations_le_launches,
        i537b_wellformed_has_no_violations,
    };

    fn plan(tensor: TensorId, threads: u64, grid: u64, ms: u64) -> LaunchPlan {
        LaunchPlan {
            tensor,
            threads_per_block: threads,
            grid,
            duration_ms: ms,
        }
    }

    #[test]
    fn mock_launch_ok_fills_tensor_and_audits_satisfied() {
        let mut gpu = MockGpuBackend::new();
        let t = gpu.allocate(Shape::new(vec![4])).unwrap();
        let res = gpu.launch(plan(t, 256, 2, 12)).unwrap();
        assert_eq!(res, BackendOutcome::Ok);
        assert_eq!(gpu.tensor_data(t).unwrap(), vec![512.0; 4]);

        let audit = gpu.audit();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].status, LaunchStatus::Satisfied);
        assert_eq!(audit[0].duration_ms, 12);
        assert!(i536a_wellformed_entries_satisfied(&audit));
        assert!(i537a_violations_le_launches(&audit));
        assert!(i537b_wellformed_has_no_violations(&audit));
        assert!(gpu.metrics().launches() == 1 && gpu.metrics().failures() == 0);
    }

    #[test]
    fn mock_failed_outcome_still_satisfies_contract() {
        let mut gpu = MockGpuBackend::new();
        let t = gpu.allocate(Shape::new(vec![2])).unwrap();
        gpu.fail_next();
        let res = gpu.launch(plan(t, 128, 1, 3)).unwrap();
        assert_eq!(res, BackendOutcome::Failed);
        let audit = gpu.audit();
        assert_eq!(crate::audit::failed_launches(&audit), 1);
        assert_eq!(audit[0].status, LaunchStatus::Satisfied);
        assert!(i537b_wellformed_has_no_violations(&audit));
        assert_eq!(gpu.metrics().launches(), 1);
        assert_eq!(gpu.metrics().failures(), 1);
    }

    #[test]
    fn mock_governance_rejects_out_of_band_without_execution() {
        let mut gpu = MockGpuBackend::new();
        let t = gpu.allocate(Shape::new(vec![1])).unwrap();
        // grid acima do cap G-07 e duração acima da capa Tile.
        let err = gpu.launch(plan(t, 1024, crate::governance::MAX_GRID + 1, 5_001));
        assert!(matches!(err, Err(BackendError::Governance(_))));
        // nada executado nem auditado (I536-A: contrato nunca satisfeito fora
        // da banda).
        assert!(gpu.audit().is_empty());
        assert_eq!(gpu.metrics().launches(), 0);
    }

    #[test]
    fn mock_unknown_tensor_rejected() {
        let mut gpu = MockGpuBackend::new();
        let err = gpu.launch(plan(TensorId(404), 32, 1, 1));
        assert!(matches!(err, Err(BackendError::UnknownTensor(_))));
        assert!(gpu.audit().is_empty());
    }

    #[test]
    fn mock_i537b_holds_over_multi_launch_trail() {
        let mut gpu = MockGpuBackend::new();
        let a = gpu.allocate(Shape::new(vec![8])).unwrap();
        let b = gpu.allocate(Shape::new(vec![16])).unwrap();
        gpu.launch(plan(a, 64, 1, 1)).unwrap();
        gpu.launch(plan(b, 32, 4, 1)).unwrap();
        gpu.launch(plan(a, 64, 1, 1)).unwrap();
        let audit = gpu.audit();
        assert_eq!(audit.len(), 3);
        assert_eq!(crate::audit::contract_violations(&audit), 0);
        assert!(i537b_wellformed_has_no_violations(&audit));
        assert!(i536a_wellformed_entries_satisfied(&audit));
    }
}