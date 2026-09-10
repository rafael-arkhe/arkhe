//! Abstração mínima real de backend GPU (bloco 1011, v390.2).
//!
//! O crate é agnóstico de hardware: a governança valida geometria e duração
//! (banda G-07 / capas Tile-SIMT), a auditoria regista só contratos satisfeitos
//! (I536–I537) e o backend executa. A única implementação deste crate é o mock
//! (sem hardware); as tracks reais Tile (`cutile`, feature `tile`) e SIMT
//! (cuda-oxide, feature `simt` — fora do escopo) NÃO compilam neste ambiente e
//! nunca são ativadas em CI.

use crate::governance::{self, LaunchDecision, RejectReason};
use crate::tensor::{Shape, TensorId};

/// Classe da implementação de backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Mock em memória inclusa no crate — executável sem hardware (única track
    /// deste ambiente).
    Mock,
    /// Track real NVIDIA cutile (feature `tile`) — Linux + sm_80+ + CUDA 13.3.
    Tile,
    /// Track real NVIDIA cuda-oxide (feature `simt`) — fora do escopo do crate
    /// (não é dependência Cargo).
    Simt,
}

/// Plano de lançamento submetido à governança e ao backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchPlan {
    /// Tensor alvo.
    pub tensor: TensorId,
    /// Threads por bloco (G-07: ≤ 1024).
    pub threads_per_block: u64,
    /// N.º de blocos da grelha (G-07: ≤ 2³¹−1).
    pub grid: u64,
    /// Duração estimada do lance em ms (capas Tile 5 s / SIMT 10 s).
    pub duration_ms: u64,
}

/// Dano de backend tipado (honestidade: `Err` enumera a causa).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// A governança recusou o lance (banda/duração), com a causa.
    Governance(RejectReason),
    /// O tensor não existe nesta instância de backend.
    UnknownTensor(TensorId),
    /// Falha do hardware/da track na execução.
    Runtime(String),
}

/// Um backend GPU real do crate.
pub trait GpuBackend {
    /// Nome estável da instância (para relatórios humanos).
    fn name(&self) -> &str;

    /// Classe da implementação.
    fn kind(&self) -> BackendKind;

    /// Aloca um tensor com a forma dada e devolve o id.
    ///
    /// # Errors
    ///
    /// * [`BackendError::Runtime`] — alocação rejeitada pela track.
    fn allocate(&mut self, shape: Shape) -> Result<TensorId, BackendError>;

    /// Submete um lance. A governança é consultada PRIMEIRO (banda G-07 +
    /// capa de duração); só lances `Allow` chegam à execução — o contrato
    /// satisfeito é o que a auditoria (I536-A) espera gravar.
    ///
    /// # Errors
    ///
    /// * [`BackendError::Governance`] — fora da banda G-07 ou da capa; nada é
    ///   executado.
    /// * [`BackendError::UnknownTensor`] — tensor inexistente.
    /// * [`BackendError::Runtime`] — falha de execução da track.
    fn launch(&mut self, plan: LaunchPlan) -> Result<LaunchOutcome, BackendError> {
        match governance::evaluate(
            plan.threads_per_block,
            plan.grid,
            plan.duration_ms,
            self.track_cap_ms(),
        ) {
            LaunchDecision::Allow => self.execute(plan),
            LaunchDecision::Reject(r) => Err(BackendError::Governance(r)),
        }
    }

    /// Execução concreta após autorização da governança (método da track).
    ///
    /// # Errors
    ///
    /// * [`BackendError::UnknownTensor`] — tensor inexistente na instância.
    /// * [`BackendError::Runtime`] — falha de execução.
    fn execute(&mut self, plan: LaunchPlan) -> Result<LaunchOutcome, BackendError>;

    /// Capa de duração da track (ms): Tile 5000 / SIMT 10000 / Mock 5000.
    fn track_cap_ms(&self) -> u64;

    /// Sincroniza os streams (no-op no mock).
    ///
    /// # Errors
    ///
    /// * [`BackendError::Runtime`] — erro de sincronização da track.
    fn sync(&mut self) -> Result<(), BackendError>;
}

/// Resultado de um lance executado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchOutcome {
    /// Executado e sincronizado.
    Ok,
    /// Falha de execução.
    Failed,
}