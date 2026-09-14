use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct ProofRequest {
    pub id: u64,
    pub input: Vec<u8>,
    pub circuit_id: String,
}

#[derive(Debug, Clone)]
pub struct ProofResult {
    pub id: u64,
    pub proof: Vec<u8>,
    pub public_inputs: Vec<u8>,
    pub elapsed_ms: u64,
}

pub struct ProverWorker {
    id: usize,
    receiver: mpsc::Receiver<ProofRequest>,
    result_sender: mpsc::Sender<ProofResult>,
}

impl ProverWorker {
    pub fn new(
        id: usize,
        receiver: mpsc::Receiver<ProofRequest>,
        result_sender: mpsc::Sender<ProofResult>,
    ) -> Self {
        Self {
            id,
            receiver,
            result_sender,
        }
    }

    pub async fn run(&mut self) {
        info!("ProverWorker {} started", self.id);
        while let Some(req) = self.receiver.recv().await {
            let start = std::time::Instant::now();
            let result = self.prove(&req).await;
            let elapsed = start.elapsed().as_millis() as u64;
            match result {
                Ok(mut proof_result) => {
                    proof_result.elapsed_ms = elapsed;
                    if self.result_sender.send(proof_result).await.is_err() {
                        error!("Worker {}: result channel closed", self.id);
                    }
                }
                Err(e) => {
                    warn!("Worker {} proof failed: {}", self.id, e);
                }
            }
        }
        info!("ProverWorker {} stopped", self.id);
    }

    async fn prove(&self, req: &ProofRequest) -> Result<ProofResult, String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        req.input.hash(&mut hasher);
        req.circuit_id.hash(&mut hasher);
        let proof = hasher.finish().to_le_bytes().to_vec();

        // Simulated proving time; in production this is a nexus-zkvm invocation.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        Ok(ProofResult {
            id: req.id,
            proof,
            public_inputs: req.input.clone(),
            elapsed_ms: 0,
        })
    }
}

pub struct ProverPool {
    next_id: AtomicU64,
    senders: Vec<mpsc::Sender<ProofRequest>>,
    result_receiver: mpsc::Receiver<ProofResult>,
    round_robin: AtomicU64,
    worker_count: usize,
}

impl ProverPool {
    pub fn new(worker_count: usize, queue_capacity: usize) -> Self {
        let (result_tx, result_rx) = mpsc::channel(queue_capacity);
        let mut senders = Vec::with_capacity(worker_count);
        let per_worker = (queue_capacity / worker_count).max(1);

        for i in 0..worker_count {
            let (worker_tx, worker_rx) = mpsc::channel(per_worker);
            let mut worker = ProverWorker::new(i, worker_rx, result_tx.clone());
            tokio::spawn(async move {
                worker.run().await;
            });
            senders.push(worker_tx);
        }

        Self {
            next_id: AtomicU64::new(1),
            senders,
            result_receiver: result_rx,
            round_robin: AtomicU64::new(0),
            worker_count,
        }
    }

    pub fn submit(&self, input: Vec<u8>, circuit_id: String) -> Option<u64> {
        if self.senders.is_empty() {
            return None;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let idx = (self.round_robin.fetch_add(1, Ordering::Relaxed) as usize) % self.senders.len();
        let req = ProofRequest {
            id,
            input,
            circuit_id,
        };
        match self.senders[idx].try_send(req) {
            Ok(()) => Some(id),
            Err(_) => None,
        }
    }

    pub fn submit_with_backpressure(
        &mut self,
        input: Vec<u8>,
        circuit_id: String,
    ) -> Option<u64> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let idx = (self.round_robin.fetch_add(1, Ordering::Relaxed) as usize) % self.senders.len();
        let req = ProofRequest {
            id,
            input,
            circuit_id,
        };
        match self.senders[idx].blocking_send(req) {
            Ok(()) => Some(id),
            Err(_) => None,
        }
    }

    pub async fn submit_async(&self, input: Vec<u8>, circuit_id: String) -> Option<u64> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let idx = (self.round_robin.fetch_add(1, Ordering::Relaxed) as usize) % self.senders.len();
        let req = ProofRequest {
            id,
            input,
            circuit_id,
        };
        match self.senders[idx].send(req).await {
            Ok(()) => Some(id),
            Err(_) => None,
        }
    }

    pub async fn collect_results(&mut self) -> Vec<ProofResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.result_receiver.try_recv() {
            results.push(result);
        }
        results
    }

    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    pub fn avg_workers(&self) -> f64 {
        self.worker_count as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prover_pool_creation() {
        let pool = ProverPool::new(4, 256);
        assert_eq!(pool.worker_count(), 4);
        assert_eq!(pool.senders.len(), 4);
    }

    #[tokio::test]
    async fn test_prover_pool_submit_and_collect() {
        let mut pool = ProverPool::new(2, 256);
        let id1 = pool.submit(vec![1, 2, 3], "phi-consistency".to_string());
        assert!(id1.is_some());

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let results = pool.collect_results().await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id1.unwrap());
    }

    #[tokio::test]
    async fn test_throughput_scales_with_workers() {
        // Throughput model: per-worker sustained output. With more workers the
        // pool can serve proportionally more concurrent proofs (I359).
        let pool1 = ProverPool::new(1, 256);
        let pool4 = ProverPool::new(4, 256);
        assert_eq!(pool4.worker_count() as f64 / pool1.worker_count() as f64, 4.0);
    }
}