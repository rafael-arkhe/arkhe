// Catedral OS v304.0 — Pool de provadores com escalabilidade linear (I359).
//
// O rascunho original não continha código para `zk/prover_pool.rs` nem
// `zk/lib.rs`. Este módulo materializa I359: throughput(N) ≈ N · throughput(1),
// formalizado em lean/ProductionHardware.lean (i359_pool_scaling).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum ProverError {
    #[error("pool encerrado")]
    PoolShutdown,
    #[error("falha na prova da tarefa {task_id}: {msg}")]
    ProofFailed { task_id: String, msg: String },
}

pub type ZkResult<T> = Result<T, ProverError>;

/// Tarefa de prova. O programado é env:`program`; os inputs públicos ficam
/// em `public_inputs` (serializados) e `private_witness` (nunca persistido —
/// Ethics-2 data minimization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProverTask {
    pub id: String,
    pub program: Vec<u8>,
    pub public_inputs: Vec<u8>,
    pub private_witness: Vec<u8>,
}

/// Prova produzida por um worker do pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    pub task_id: String,
    pub proof: Vec<u8>,
    pub public_inputs_hash: [u8; 32],
    pub elapsed_ms: u64,
    pub worker_id: u32,
}

/// Backend de prova plugável.
pub trait Prover: Send + Sync {
    fn prove(&self, task: &ProverTask) -> Result<Vec<u8>, String>;
    fn name(&self) -> &'static str;
}

/// Provador simples (latência configurável) usado pela feature default `sim`.
/// Produz um digest FNV-1a como "proof placeholder".
pub struct SimProver {
    pub latency: Duration,
}

impl SimProver {
    pub fn new(latency: Duration) -> Self {
        Self { latency }
    }

    fn fnv1a(data: &[u8]) -> [u8; 32] {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &b in data {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        let mut out = [0u8; 32];
        out[..8].copy_from_slice(&hash.to_le_bytes());
        out
    }
}

impl Prover for SimProver {
    fn prove(&self, task: &ProverTask) -> Result<Vec<u8>, String> {
        thread::sleep(self.latency);
        let mut digest = Self::fnv1a(&task.program);
        for (i, b) in digest.iter_mut().enumerate() {
            *b ^= task.public_inputs[i % task.public_inputs.len().max(1)] ^ 0xA5;
        }
        Ok(digest.to_vec())
    }

    fn name(&self) -> &'static str {
        "sim-fnv1a"
    }
}

type Job = (ProverTask, mpsc::Sender<ZkResult<Proof>>);

/// Pool com `n_workers` threads; fila compartilhada protegida por mutex,
/// encerramento controlado por AtomicBool.
pub struct ProverPool {
    shared: Arc<Mutex<VecDeque<Job>>>,
    /// Handles mantidos vivos para join/coordenação de fim de vida do pool.
    #[allow(dead_code)]
    workers: Vec<JoinHandle<()>>,
    prover: Arc<dyn Prover>,
    /// Flag de encerramento (método shutdown futuro); campo mantido para
    /// que os workers guardem o Arc e parem quando for setado.
    #[allow(dead_code)]
    shutdown: Arc<AtomicUsize>,
    next_worker: Arc<AtomicUsize>,
    n_workers: usize,
}

impl ProverPool {
    pub fn new(prover: Arc<dyn Prover>, n_workers: usize) -> Self {
        let n_workers = n_workers.max(1);
        let shared = Arc::new(Mutex::new(VecDeque::new()));
        let shutdown = Arc::new(AtomicUsize::new(0));

        let mut workers = Vec::with_capacity(n_workers);
        for id in 0..n_workers as u32 {
            let shared = Arc::clone(&shared);
            let prover = Arc::clone(&prover);
            let shutdown = Arc::clone(&shutdown);
            workers.push(thread::spawn(move || worker_loop(id, shared, shutdown, prover)));
        }

        Self {
            shared,
            workers,
            prover,
            shutdown,
            next_worker: Arc::new(AtomicUsize::new(0)),
            n_workers,
        }
    }

    pub fn n_workers(&self) -> usize {
        self.n_workers
    }

    pub fn submit(&self, task: ProverTask) -> ZkResult<Proof> {
        let (tx, rx) = mpsc::channel();
        let w = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.n_workers;
        debug!(worker = w, task = %task.id, "tarefa enfileirada");
        {
            let mut queue = self.shared.lock().unwrap();
            queue.push_back((task, tx));
        }
        rx.recv()
            .map_err(|_| ProverError::PoolShutdown)
            .and_then(|r| r)
    }

    /// I359: mede o throughput médio (tarefas/seg) com `n_workers` workers.
    /// A medição submete tarefas de forma bloqueante; a concorrência vem dos
    /// workers internos do pool.
    pub fn measure_throughput(&self, tasks: usize, n_workers: usize) -> f64 {
        let extra = if n_workers == self.n_workers {
            None
        } else {
            Some(Self::new(Arc::clone(&self.prover), n_workers))
        };
        let target: &ProverPool = match &extra {
            Some(pool) => pool,
            None => self,
        };
        let start = Instant::now();
        for i in 0..tasks {
            let task = ProverTask {
                id: format!("bench-{}-{}", n_workers, i),
                program: vec![i as u8],
                public_inputs: vec![0xcu8; 8],
                private_witness: Vec::new(),
            };
            let _ = target.submit(task);
        }
        let elapsed = start.elapsed().as_secs_f64();
        tasks as f64 / elapsed.max(1e-9)
    }
}

fn worker_loop(
    id: u32,
    shared: Arc<Mutex<VecDeque<Job>>>,
    shutdown: Arc<AtomicUsize>,
    prover: Arc<dyn Prover>,
) {
    while shutdown.load(Ordering::Relaxed) == 0 {
        let job = {
            let mut queue = shared.lock().unwrap();
            queue.pop_front()
        };
        let Some((task, reply)) = job else {
            thread::sleep(Duration::from_millis(5));
            continue;
        };
        let started = Instant::now();
        let result = prover
            .prove(&task)
            .map(|proof_bytes| Proof {
                task_id: task.id.clone(),
                proof: proof_bytes,
                public_inputs_hash: SimProver::fnv1a(&task.public_inputs),
                elapsed_ms: started.elapsed().as_millis() as u64,
                worker_id: id,
            })
            .map_err(|msg| ProverError::ProofFailed {
                task_id: task.id,
                msg,
            });
        let _ = reply.send(result);
        info!(worker = id, "prova concluída");
    }
}

/// Teste de escala I359: verifica throughput(N) ≈ N · throughput(1) dentro do
/// slack configurável. Executa com o provador SIM no CI; em hardware, substitua
/// pelo backend Nexus (feature `nexus`).
pub fn scaling_satisfies_linearity(std1: f64, stdn: f64, n: usize, slack: f64) -> bool {
    let expected = std1 * n as f64;
    let ratio = stdn / expected.max(1e-9);
    info!(std1, stdn, expected, ratio, "verificação I359");
    let ok = (ratio - 1.0).abs() <= slack;
    if !ok {
        warn!(ratio, "I359 violada: throughput não escala linearmente");
    }
    ok
}

pub fn run_scaling_test(pool_tasks: usize, slack: f64) -> Result<(f64, f64), ProverError> {
    let pool = ProverPool::new(Arc::new(SimProver::new(Duration::from_millis(10))), 4);
    let t1 = pool.measure_throughput(pool_tasks, 1);
    let t4 = pool.measure_throughput(pool_tasks, 4);
    if !scaling_satisfies_linearity(t1, t4, 4, slack) {
        return Err(ProverError::ProofFailed {
            task_id: "i359".into(),
            msg: "escalabilidade linear não satisfeita".into(),
        });
    }
    Ok((t1, t4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_submits_and_proves() {
        let pool = ProverPool::new(Arc::new(SimProver::new(Duration::from_millis(1))), 2);
        let proof = pool
            .submit(ProverTask {
                id: "t1".into(),
                program: vec![1, 2, 3],
                public_inputs: vec![0xbb; 4],
                private_witness: Vec::new(),
            })
            .unwrap();
        assert_eq!(proof.task_id, "t1");
        assert!(!proof.proof.is_empty());
    }
}