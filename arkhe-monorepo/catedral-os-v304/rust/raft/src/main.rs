// Catedral OS v304.0 — Núcleo Raft com snapshots e batching (I360).
//
// Este arquivo usa async-raft 0.6.1 (última versão publicada no crates.io;
// a linha "0.8" que o rascunho original citava não existe publicada). O
// rascunho original implementava métodos inexistentes da trait RaftStorage
// (get_data/set_data/delete_data e create_snapshot/apply_snapshot com
// assinaturas erradas). Aqui o contrato 0.6.1 é cumprido sobre a camada Kvs
// (sled por padrão, RocksDB por feature), com persistência de hard state,
// membership, log e snapshots gzip.

use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_raft::raft::MembershipConfig;
use async_raft::storage::{CurrentSnapshotData, HardState, InitialState};
use async_raft::{AppData, AppDataResponse, RaftStorage};
use async_raft::raft::Entry;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tracing::{info, warn};
use uuid::Uuid;

mod kv;
mod snapshot;

use kv::{Kvs, SledStore};
use snapshot::{
    Snapshot, SnapshotBuf, SNAPSHOT_DATA_KEY, SNAPSHOT_META_INDEX_KEY, SNAPSHOT_META_TERM_KEY,
};

// ============================================================================ #
// 1. TIPOS DE DADOS (aplicação)
// ============================================================================ #

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoherenceState {
    pub phi: f64,
    pub handover_count: u64,
    pub last_handover: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhiUpdate {
    pub phi: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum RaftRequest {
    UpdatePhi { phi: f64, timestamp: u64 },
    GetCoherence,
    RegisterHandover { source: String, target: String, phi: f64 },
    BatchUpdatePhi { updates: Vec<PhiUpdate> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RaftResponse {
    PhiUpdated { phi: f64, handover_count: u64 },
    CoherenceState { phi: f64, handover_count: u64 },
    HandoverRegistered { id: String },
    BatchProcessed { count: usize },
    Error { message: String },
}

impl AppData for RaftRequest {}
impl AppDataResponse for RaftResponse {}

// ============================================================================ #
// 2. ERROS DE ARMAZENAMENTO
// ============================================================================ #

const DEFAULT_HARD_STATE: HardState = HardState {
    current_term: 0,
    voted_for: None,
};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("kv: {0}")]
    Kv(#[from] kv::KvError),
    #[error("serde_json: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("utf8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("tempo: {0}")]
    Time(String),
    #[error("outro: {0}")]
    Other(String),
}

// ============================================================================ #
// 3. ARMAZENAMENTO PERSISTENTE (RaftStorage sobre Kvs)
// ============================================================================ #

const KEY_HARD_STATE: &str = "hard_state";
const KEY_MEMBERSHIP: &str = "membership";
const KEY_COHERENCE_STATE: &str = "coherence:state";
const KEY_LAST_RESPONSE: &str = "coherence:last_response";
const KEY_LAST_APPLIED: &str = "coherence:last_applied";
const LOG_PREFIX: &str = "log:";

type AppliedEntry = (u64, RaftResponse);

pub struct CoherenceStorage {
    store: Arc<dyn Kvs>,
    node_id: u64,
    snapshot_interval: u64,
    applied: tokio::sync::Mutex<HashMap<u64, AppliedEntry>>,
}

impl CoherenceStorage {
    pub fn new(store: Arc<dyn Kvs>, node_id: u64, snapshot_interval: u64) -> Self {
        Self {
            store,
            node_id,
            snapshot_interval,
            applied: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn node_id(&self) -> u64 {
        self.node_id
    }

    pub fn store_handle(&self) -> &dyn Kvs {
        &*self.store
    }

    pub async fn applied_entries_len(&self) -> usize {
        self.applied.lock().await.len()
    }

    fn coherence_state(&self) -> Result<CoherenceState> {
        Ok(self
            .store
            .get_json::<CoherenceState>(KEY_COHERENCE_STATE)?
            .unwrap_or_default())
    }

    fn apply_request(&self, index: u64, req: &RaftRequest) -> Result<RaftResponse> {
        let mut state = self.coherence_state()?;
        match req {
            RaftRequest::UpdatePhi { phi, timestamp } => {
                state.phi = phi.clamp(0.0, 1.0);
                state.last_handover = *timestamp;
                self.store.put_json(KEY_COHERENCE_STATE, &state)?;
                Ok(RaftResponse::PhiUpdated {
                    phi: state.phi,
                    handover_count: state.handover_count,
                })
            }
            RaftRequest::GetCoherence => Ok(RaftResponse::CoherenceState {
                phi: state.phi,
                handover_count: state.handover_count,
            }),
            RaftRequest::RegisterHandover { phi, .. } => {
                state.phi = phi.clamp(0.0, 1.0);
                state.handover_count += 1;
                state.last_handover = index;
                self.store.put_json(KEY_COHERENCE_STATE, &state)?;
                Ok(RaftResponse::HandoverRegistered {
                    id: Uuid::new_v4().to_string(),
                })
            }
            RaftRequest::BatchUpdatePhi { updates } => {
                for update in updates {
                    state.phi = update.phi.clamp(0.0, 1.0);
                    state.last_handover = update.timestamp;
                }
                self.store.put_json(KEY_COHERENCE_STATE, &state)?;
                Ok(RaftResponse::BatchProcessed {
                    count: updates.len(),
                })
            }
        }
    }

    fn last_log(&self) -> Result<(u64, u64)> {
        let mut last_index = 0u64;
        let mut last_term = 0u64;
        for (key, _) in self.store.scan()? {
            let key_str = String::from_utf8(key)?;
            if let Some(suffix) = key_str.strip_prefix(LOG_PREFIX) {
                if let Ok(n) = suffix.parse::<u64>() {
                    if n > last_index {
                        last_index = n;
                        if let Some(entry) = self
                            .store
                            .get_json::<Entry<RaftRequest>>(&key_str)?
                        {
                            last_term = entry.term;
                        }
                    }
                }
            }
        }
        Ok((last_index, last_term))
    }

    fn delete_logs_before(&self, index: u64) -> Result<()> {
        let mut keys = Vec::new();
        for (key, _) in self.store.scan()? {
            let key_str = String::from_utf8(key)?;
            if let Some(suffix) = key_str.strip_prefix(LOG_PREFIX) {
                if let Ok(n) = suffix.parse::<u64>() {
                    if n <= index {
                        keys.push(key_str);
                    }
                }
            }
        }
        for key in keys {
            self.store.delete(key.as_bytes())?;
        }
        Ok(())
    }

    fn meta_membership(&self) -> Result<MembershipConfig> {
        Ok(self
            .store
            .get_json::<MembershipConfig>(KEY_MEMBERSHIP)?
            .unwrap_or_else(|| MembershipConfig::new_initial(self.node_id)))
    }

    /// Persiste o snapshot + metadados e retorna o CurrentSnapshotData.
    fn persist_snapshot(&self, index: u64, term: u64) -> Result<CurrentSnapshotData<SnapshotBuf>> {
        let local = snapshot::create_snapshot(&*self.store)?;
        let packed = snapshot::compress(&local)?;
        let membership = self.meta_membership()?;

        self.store.put(SNAPSHOT_DATA_KEY.as_bytes(), &packed)?;
        self.store.put_json(SNAPSHOT_META_INDEX_KEY, &index)?;
        self.store.put_json(SNAPSHOT_META_TERM_KEY, &term)?;
        self.store.put_json(KEY_MEMBERSHIP, &membership)?;

        info!(
            index, term, entradas = local.entries.len(),
            "snapshot persistido"
        );
        Ok(CurrentSnapshotData {
            term,
            index,
            membership,
            snapshot: Box::new(SnapshotBuf::new(packed)),
        })
    }

    /// Fecha um snapshot (criado via create_snapshot) na máquina de estados do
    /// nó local, trunca o log e registra o snapshot pointer.
    #[allow(dead_code)]
    async fn finalize_local(&self, index: u64, term: u64, id: String) -> Result<()> {
        let packed = self
            .store
            .get(SNAPSHOT_DATA_KEY.as_bytes())?
            .ok_or_else(|| StorageError::Other("snapshot ausente para finalizar".into()))?;
        let snap = snapshot::decompress(&packed)?;
        snapshot::restore_snapshot(&*self.store, &snap)?;

        self.delete_logs_before(index)?;
        let membership = self.meta_membership()?;
        let pointer = Entry::<RaftRequest>::new_snapshot_pointer(index, term, id, membership.clone());
        self.store.put_json(&format!("{}{}", LOG_PREFIX, index), &pointer)?;
        self.store.put_json(KEY_LAST_APPLIED, &index)?;
        info!(index, term, "snapshot finalizado localmente");
        Ok(())
    }

    /// Snapshot do keyspace de coordenação (usado pelo loop de manutenção).
    pub async fn create_snapshot(&self) -> Result<Snapshot> {
        Ok(snapshot::create_snapshot(&*self.store)?)
    }
}

// ============================================================================ #
// 4. IMPLEMENTAÇÃO DO CONTRATO RaftStorage (async-raft 0.6.1)
// ============================================================================ #

#[async_trait]
impl RaftStorage<RaftRequest, RaftResponse> for CoherenceStorage {
    type Snapshot = SnapshotBuf;
    type ShutdownError = StorageError;

    async fn get_membership_config(&self) -> Result<MembershipConfig> {
        let membership = self.meta_membership()?;
        self.store.put_json(KEY_MEMBERSHIP, &membership)?;
        Ok(membership)
    }

    async fn get_initial_state(&self) -> Result<InitialState> {
        let (last_log_index, last_log_term) = self.last_log()?;
        let hard_state = self
            .store
            .get_json::<HardState>(KEY_HARD_STATE)?
            .unwrap_or(DEFAULT_HARD_STATE);

        let membership = self.meta_membership()?;
        let last_applied_log = self
            .store
            .get_json::<u64>(KEY_LAST_APPLIED)?
            .unwrap_or(0);

        // Se um snapshot já foi instalado, o last_applied deve refletir o índice dele.
        let last_applied_log = if last_applied_log == 0 {
            self.store
                .get_json::<u64>(SNAPSHOT_META_INDEX_KEY)?
                .unwrap_or(0)
        } else {
            last_applied_log
        };

        Ok(InitialState {
            last_log_index,
            last_log_term,
            last_applied_log,
            hard_state,
            membership,
        })
    }

    async fn save_hard_state(&self, hs: &HardState) -> Result<()> {
        self.store.put_json(KEY_HARD_STATE, hs)?;
        Ok(())
    }

    async fn get_log_entries(&self, start: u64, stop: u64) -> Result<Vec<Entry<RaftRequest>>> {
        let mut out = Vec::new();
        for index in start..stop {
            if let Some(entry) = self
                .store
                .get_json::<Entry<RaftRequest>>(&format!("{}{}", LOG_PREFIX, index))?
            {
                out.push(entry);
            }
        }
        Ok(out)
    }

    async fn delete_logs_from(&self, start: u64, stop: Option<u64>) -> Result<()> {
        match stop {
            Some(stop) => {
                for index in start..stop {
                    self.store
                        .delete(format!("{}{}", LOG_PREFIX, index).as_bytes())?;
                }
            }
            None => {
                for (key, _) in self.store.scan()? {
                    let key_str = String::from_utf8(key)?;
                    if let Some(suffix) = key_str.strip_prefix(LOG_PREFIX) {
                        if let Ok(n) = suffix.parse::<u64>() {
                            if n >= start {
                                self.store.delete(key_str.as_bytes())?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn append_entry_to_log(&self, entry: &Entry<RaftRequest>) -> Result<()> {
        self.store
            .put_json(&format!("{}{}", LOG_PREFIX, entry.index), entry)?;
        Ok(())
    }

    async fn replicate_to_log(&self, entries: &[Entry<RaftRequest>]) -> Result<()> {
        let mut pairs = Vec::with_capacity(entries.len());
        for entry in entries {
            pairs.push((
                format!("{}{}", LOG_PREFIX, entry.index).into_bytes(),
                serde_json::to_vec(entry)?,
            ));
        }
        self.store.batch_put(pairs)?;
        self.maybe_snapshot().await;
        Ok(())
    }

    async fn apply_entry_to_state_machine(&self, index: &u64, data: &RaftRequest) -> Result<RaftResponse> {
        let response = self.apply_request(*index, data)?;
        self.applied.lock().await.insert(*index, (*index, response.clone()));
        self.store.put_json(KEY_LAST_RESPONSE, &response)?;
        self.store.put_json(KEY_LAST_APPLIED, index)?;
        self.maybe_snapshot().await;
        Ok(response)
    }

    async fn replicate_to_state_machine(&self, entries: &[(&u64, &RaftRequest)]) -> Result<()> {
        for (index, data) in entries {
            let response = self.apply_request(**index, data)?;
            self.applied
                .lock()
                .await
                .insert(**index, (**index, response));
        }
        Ok(())
    }

    async fn do_log_compaction(&self) -> Result<CurrentSnapshotData<Self::Snapshot>> {
        let applied = self.applied.lock().await;
        let (index, _) = applied
            .iter()
            .max_by_key(|(index, _)| *index)
            .map(|(i, e)| (*i, e))
            .unwrap_or((0, &(0, RaftResponse::Error { message: String::new() })));

        let (_, last_term) = self.last_log()?;
        let meta = self.persist_snapshot(index, last_term)?;
        Ok(meta)
    }

    async fn create_snapshot(
        &self,
    ) -> Result<(String, Box<Self::Snapshot>)> {
        let local = snapshot::create_snapshot(&*self.store)?;
        let packed = snapshot::compress(&local)?;
        let id = format!("catedral-snapshot-{}", local.timestamp);
        info!(id, entradas = local.entries.len(), "snapshot criado");
        Ok((id, Box::new(SnapshotBuf::new(packed))))
    }

    async fn finalize_snapshot_installation(
        &self,
        index: u64,
        term: u64,
        delete_through: Option<u64>,
        id: String,
        mut snapshot: Box<Self::Snapshot>,
    ) -> Result<()> {
        let mut packed = Vec::new();
        snapshot.read_to_end(&mut packed).await?;
        let snap = snapshot::decompress(&packed)?;
        snapshot::restore_snapshot(&*self.store, &snap)?;

        // Trunca o log conforme o líder orientou.
        match delete_through {
            Some(n) => self.delete_logs_before(n)?,
            None => {
                for (key, _) in self.store.scan()? {
                    let key_str = String::from_utf8(key)?;
                    if key_str.starts_with(LOG_PREFIX) {
                        self.store.delete(key_str.as_bytes())?;
                    }
                }
            }
        }

        let membership = self.meta_membership()?;
        self.store.put(SNAPSHOT_DATA_KEY.as_bytes(), &packed)?;
        self.store.put_json(SNAPSHOT_META_INDEX_KEY, &index)?;
        self.store.put_json(SNAPSHOT_META_TERM_KEY, &term)?;
        self.store.put_json(KEY_MEMBERSHIP, &membership)?;

        let pointer = Entry::<RaftRequest>::new_snapshot_pointer(index, term, id, membership);
        self.store
            .put_json(&format!("{}{}", LOG_PREFIX, index), &pointer)?;
        self.store.put_json(KEY_LAST_APPLIED, &index)?;
        info!(index, term, "snapshot instalado e log truncado");
        Ok(())
    }

    async fn get_current_snapshot(&self) -> Result<Option<CurrentSnapshotData<Self::Snapshot>>> {
        let index = match self.store.get_json::<u64>(SNAPSHOT_META_INDEX_KEY)? {
            Some(index) => index,
            None => return Ok(None),
        };
        let term = self
            .store
            .get_json::<u64>(SNAPSHOT_META_TERM_KEY)?
            .unwrap_or(0);
        let data = self
            .store
            .get(SNAPSHOT_DATA_KEY.as_bytes())?
            .unwrap_or_default();
        let membership = self.meta_membership()?;
        Ok(Some(CurrentSnapshotData {
            term,
            index,
            membership,
            snapshot: Box::new(SnapshotBuf::new(data)),
        }))
    }
}

impl CoherenceStorage {
    /// Dispara snapshot automático quando o limiar de entradas é atingido (batching).
    async fn maybe_snapshot(&self) {
        let applied = self.applied.lock().await.len() as u64;
        if applied >= self.snapshot_interval {
            match snapshot::create_snapshot(&*self.store) {
                Ok(snap) => info!(
                    "snapshot automático: {} constituições (threshold={})",
                    snap.entries.len(),
                    self.snapshot_interval
                ),
                Err(e) => warn!(error = %e, "falha no snapshot automático"),
            }
        }
    }
}

// ============================================================================ #
// 5. PONTO DE ENTRADA
// ============================================================================ #

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            env::var("RUST_LOG").unwrap_or_else(|_| "info,catedral_raft=trace".to_string()),
        )
        .init();

    let node_name = env::var("NODE_NAME").unwrap_or_else(|_| "no-alfa".to_string());
    let node_id: u64 = env::var("NODE_ID")
        .unwrap_or_else(|_| "1".to_string())
        .parse()
        .map_err(|e| anyhow::anyhow!("NODE_ID inválida: {e}"))?;
    let peers: Vec<String> = env::var("PEERS")
        .unwrap_or_else(|_| "no-beta,no-gama".to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let storage_path =
        env::var("RAFT_STORAGE_PATH").unwrap_or_else(|_| "/var/lib/catedral/raft".to_string());
    let snapshot_interval: u64 = env::var("SNAPSHOT_INTERVAL")
        .unwrap_or_else(|_| "10000".to_string())
        .parse()
        .unwrap_or(10000);

    info!("🏛️ Catedral OS — Raft v304.0");
    info!(
        node_id, nodename = node_name, peers = ?peers, storage_path, snapshot_interval,
        "configuração carregada"
    );

    let store: Arc<dyn Kvs> = Arc::new(SledStore::open(&storage_path)?);
    let storage = Arc::new(CoherenceStorage::new(store, node_id, snapshot_interval));

    if let Some(current) = storage.get_current_snapshot().await? {
        info!(
            index = current.index, term = current.term,
            "snapshot persistido no boot"
        );
    }

    info!("🏛️ Núcleo Raft pronto — storage configurado, snapshot a cada {} entradas aplicadas", snapshot_interval);

    let mut tick = tokio::time::interval(Duration::from_secs(30));
    loop {
        tick.tick().await;
        info!(
            applied = storage.applied_entries_len().await,
            "heartbeat do núcleo Raft"
        );
    }
}