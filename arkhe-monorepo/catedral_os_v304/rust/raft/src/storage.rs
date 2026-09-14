use crate::kv::KvStore;
use crate::snapshot;
use anyhow::{anyhow, Result};
use async_raft::raft::{Entry, EntryPayload, MembershipConfig};
use async_raft::storage::{CurrentSnapshotData, HardState, InitialState, RaftStorage};
use async_raft::{AppData, AppDataResponse, NodeId};
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;

// ============================================================================
// Application data types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoherenceState {
    pub phi: f64,
    pub handover_count: u64,
    pub last_handover: u64,
}

impl Default for CoherenceState {
    fn default() -> Self {
        Self {
            phi: 0.85,
            handover_count: 0,
            last_handover: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RaftRequest {
    UpdatePhi { phi: f64, timestamp: u64 },
    GetCoherence,
    RegisterHandover { source: String, target: String, phi: f64 },
    BatchUpdatePhi { updates: Vec<(f64, u64)> },
}

impl AppData for RaftRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftResponse {
    PhiUpdated { phi: f64, handover_count: u64 },
    CoherenceState { phi: f64, handover_count: u64 },
    HandoverRegistered { id: String },
    BatchProcessed { count: usize },
    Error { message: String },
}

impl AppDataResponse for RaftResponse {}

#[derive(Debug)]
pub struct CatedralShutdownError;

impl fmt::Display for CatedralShutdownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Catedral storage shutdown error")
    }
}

impl Error for CatedralShutdownError {}

// ============================================================================
// Persistent storage
// ============================================================================

const LOG_KEY: &[u8] = b"__log__";

#[derive(Debug, Clone)]
struct SnapshotMeta {
    term: u64,
    index: u64,
    membership: MembershipConfig,
}

pub struct CoherenceStorage {
    node_id: NodeId,
    kv: Arc<dyn KvStore>,
    snapshot_dir: PathBuf,
    snapshot_interval: u64,
    log: RwLock<Vec<Entry<RaftRequest>>>,
    hard_state: RwLock<Option<HardState>>,
    last_applied: RwLock<u64>,
    membership: RwLock<MembershipConfig>,
    entries_since_snapshot: RwLock<u64>,
    current_snapshot: RwLock<Option<SnapshotMeta>>,
    current_snapshot_path: RwLock<Option<PathBuf>>,
}

impl CoherenceStorage {
    pub fn new(
        node_id: NodeId,
        kv: Arc<dyn KvStore>,
        snapshot_dir: PathBuf,
        snapshot_interval: u64,
    ) -> Self {
        std::fs::create_dir_all(&snapshot_dir).ok();
        let log: Vec<Entry<RaftRequest>> = kv
            .get(LOG_KEY)
            .ok()
            .flatten()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let last_log_index = log.iter().map(|e| e.index).max().unwrap_or(0);
        let memberships: Vec<MembershipConfig> = log
            .iter()
            .filter_map(|e| match &e.payload {
                EntryPayload::ConfigChange(c) => Some(c.membership.clone()),
                EntryPayload::SnapshotPointer(s) => Some(s.membership.clone()),
                _ => None,
            })
            .collect();
        let membership = memberships.last().cloned().unwrap_or_else(|| {
            MembershipConfig::new_initial(node_id)
        });
        info_loaded(last_log_index, membership.clone());

        Self {
            node_id,
            kv,
            snapshot_dir,
            snapshot_interval,
            log: RwLock::new(log),
            hard_state: RwLock::new(None),
            last_applied: RwLock::new(0),
            membership: RwLock::new(membership),
            entries_since_snapshot: RwLock::new(0),
            current_snapshot: RwLock::new(None),
            current_snapshot_path: RwLock::new(None),
        }
    }

    pub fn get_state(&self, key: &str) -> Option<CoherenceState> {
        self.kv
            .get(key.as_bytes())
            .ok()
            .flatten()
            .and_then(|data| serde_json::from_slice(&data).ok())
    }

    pub fn set_state(&self, key: &str, state: &CoherenceState) {
        if let Ok(serialized) = serde_json::to_vec(state) {
            let _ = self.kv.put(key.as_bytes().to_vec(), serialized);
            self.maybe_snapshot();
        }
    }

    fn maybe_snapshot(&self) {
        let mut count = self.entries_since_snapshot.write();
        *count += 1;
        if *count >= self.snapshot_interval {
            if let Ok(data) = snapshot::create_compact_snapshot(self.kv.as_ref()) {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let key = format!("snapshot:{}", now);
                let _ = self.kv.put(key.into_bytes(), data);
            }
            *count = 0;
        }
    }

    fn persist_log(&self, entries: &[Entry<RaftRequest>]) -> Result<()> {
        let serialized = serde_json::to_vec(entries)?;
        self.kv.put(LOG_KEY.to_vec(), serialized).map_err(anyhow_error)?;
        Ok(())
    }

    pub fn restore_from_snapshot(&self, timestamp: u64) -> Result<()> {
        let key = format!("snapshot:{}", timestamp);
        let data = self
            .kv
            .get(key.as_bytes())
            .map_err(anyhow_error)?
            .ok_or_else(|| anyhow!("snapshot {} not found", timestamp))?;
        snapshot::restore_compact_snapshot(self.kv.as_ref(), &data)?;
        Ok(())
    }
}

fn anyhow_error<T: fmt::Display>(e: T) -> anyhow::Error {
    anyhow!("{}", e)
}

fn info_loaded(last_log_index: u64, _membership: MembershipConfig) {
    tracing::info!("CoherenceStorage loaded, last_log_index={}", last_log_index);
}

#[async_trait]
impl RaftStorage<RaftRequest, RaftResponse> for CoherenceStorage {
    type Snapshot = tokio::fs::File;
    type ShutdownError = CatedralShutdownError;

    async fn get_membership_config(&self) -> Result<MembershipConfig> {
        Ok(self.membership.read().clone())
    }

    async fn get_initial_state(&self) -> Result<InitialState> {
        let log = self.log.read();
        let last_log_index = log.iter().map(|e| e.index).max().unwrap_or(0);
        let last_log_term = log.iter().map(|e| e.term).max().unwrap_or(0);
        Ok(InitialState {
            last_log_index,
            last_log_term,
            last_applied_log: *self.last_applied.read(),
            hard_state: self.hard_state.read().clone().unwrap_or(HardState {
                current_term: 0,
                voted_for: None,
            }),
            membership: self.membership.read().clone(),
        })
    }

    async fn save_hard_state(&self, hs: &HardState) -> Result<()> {
        *self.hard_state.write() = Some(hs.clone());
        Ok(())
    }

    async fn get_log_entries(&self, start: u64, stop: u64) -> Result<Vec<Entry<RaftRequest>>> {
        let log = self.log.read();
        Ok(log
            .iter()
            .filter(|e| e.index >= start && e.index < stop)
            .cloned()
            .collect())
    }

    async fn delete_logs_from(&self, start: u64, stop: Option<u64>) -> Result<()> {
        let mut log = self.log.write();
        match stop {
            Some(stop) => log.retain(|e| !(e.index >= start && e.index < stop)),
            None => log.retain(|e| e.index < start),
        }
        self.persist_log(&log)?;
        Ok(())
    }

    async fn append_entry_to_log(&self, entry: &Entry<RaftRequest>) -> Result<()> {
        let mut log = self.log.write();
        log.retain(|e| e.index != entry.index);
        log.push(entry.clone());
        log.sort_unstable_by_key(|e| e.index);
        self.update_membership_in_log_locked(&log);
        self.persist_log(&log)?;
        Ok(())
    }

    async fn replicate_to_log(&self, entries: &[Entry<RaftRequest>]) -> Result<()> {
        let mut log = self.log.write();
        // Physically append — truncate conflicting suffix.
        let max_index = entries.iter().map(|e| e.index).max().unwrap_or(0);
        log.retain(|e| e.index >= max_index);
        for entry in entries {
            log.retain(|e| e.index != entry.index);
            log.push(entry.clone());
        }
        log.sort_unstable_by_key(|e| e.index);
        self.update_membership_in_log_locked(&log);
        self.persist_log(&log)?;
        Ok(())
    }

    async fn apply_entry_to_state_machine(
        &self,
        index: &u64,
        data: &RaftRequest,
    ) -> Result<RaftResponse> {
        *self.last_applied.write() = *index;
        match data {
            RaftRequest::UpdatePhi { phi, timestamp } => {
                let mut state = self.kv_state();
                state.phi = phi.clamp(0.0, 1.0);
                state.last_handover = *timestamp;
                self.set_state("coherence", &state);
                Ok(RaftResponse::PhiUpdated {
                    phi: state.phi,
                    handover_count: state.handover_count,
                })
            }
            RaftRequest::GetCoherence => {
                let state = self.kv_state();
                Ok(RaftResponse::CoherenceState {
                    phi: state.phi,
                    handover_count: state.handover_count,
                })
            }
            RaftRequest::RegisterHandover { source, target, phi } => {
                let mut state = self.kv_state();
                state.handover_count += 1;
                state.phi = phi.clamp(0.0, 1.0);
                state.last_handover = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                self.set_state("coherence", &state);
                let id = format!("{}-{}-{}", source, target, state.handover_count);
                Ok(RaftResponse::HandoverRegistered { id })
            }
            RaftRequest::BatchUpdatePhi { updates } => {
                let mut state = self.kv_state();
                let count = updates.len();
                for (phi, ts) in updates {
                    state.phi = phi.clamp(0.0, 1.0);
                    state.last_handover = *ts;
                }
                self.set_state("coherence", &state);
                Ok(RaftResponse::BatchProcessed { count })
            }
        }
    }

    async fn replicate_to_state_machine(
        &self,
        entries: &[(&u64, &RaftRequest)],
    ) -> Result<()> {
        for (idx, data) in entries {
            self.apply_entry_to_state_machine(idx, data).await?;
        }
        Ok(())
    }

    async fn do_log_compaction(&self) -> Result<CurrentSnapshotData<Self::Snapshot>> {
        let data = snapshot::create_compact_snapshot(self.kv.as_ref())?;
        let index = *self.last_applied.read();
        let term = self
            .log
            .read()
            .iter()
            .filter(|e| e.index <= index)
            .map(|e| e.term)
            .max()
            .unwrap_or(0);
        let membership = self.membership.read().clone();
        let id = format!("compact-{}-{}", index, term);
        let path = self.snapshot_dir.join(format!("{}.snap", id));

        let mut file = tokio::fs::File::create(&path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        *self.current_snapshot.write() = Some(SnapshotMeta {
            term,
            index,
            membership: membership.clone(),
        });
        *self.current_snapshot_path.write() = Some(path.clone());

        let read = tokio::fs::File::open(&path).await?;
        Ok(CurrentSnapshotData {
            term,
            index,
            membership,
            snapshot: Box::new(read),
        })
    }

    async fn create_snapshot(&self) -> Result<(String, Box<Self::Snapshot>)> {
        let id = format!("tmp-{}", SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos());
        let path = self.snapshot_dir.join(format!("{}.snap", id));
        let file = tokio::fs::File::create(&path).await?;
        Ok((id, Box::new(file)))
    }

    async fn finalize_snapshot_installation(
        &self,
        index: u64,
        term: u64,
        delete_through: Option<u64>,
        id: String,
        _snapshot: Box<Self::Snapshot>,
    ) -> Result<()> {
        let membership = self.membership.read().clone();
        let path = self.snapshot_dir.join(format!("{}.snap", id));

        let mut log = self.log.write();
        if let Some(delete_through) = delete_through {
            log.retain(|e| e.index >= delete_through);
        }
        log.push(Entry::new_snapshot_pointer(index, term, id.clone(), membership.clone()));
        log.sort_unstable_by_key(|e| e.index);

        drop(log);
        self.persist_log(&self.log.read().clone())?;

        *self.current_snapshot.write() = Some(SnapshotMeta {
            term,
            index,
            membership,
        });
        *self.current_snapshot_path.write() = Some(path);
        Ok(())
    }

    async fn get_current_snapshot(&self) -> Result<Option<CurrentSnapshotData<Self::Snapshot>>> {
        let path = self.current_snapshot_path.read().clone();
        let meta = self.current_snapshot.read().clone();
        match (path, meta) {
            (Some(path), Some(meta)) => {
                let file = tokio::fs::File::open(&path).await?;
                Ok(Some(CurrentSnapshotData {
                    term: meta.term,
                    index: meta.index,
                    membership: meta.membership,
                    snapshot: Box::new(file),
                }))
            }
            _ => Ok(None),
        }
    }
}

impl CoherenceStorage {
    fn kv_state(&self) -> CoherenceState {
        self.get_state("coherence").unwrap_or_default()
    }

    fn update_membership_in_log_locked(&self, log: &[Entry<RaftRequest>]) {
        if let Some(membership) = log
            .iter()
            .rev()
            .find_map(|e| match &e.payload {
                EntryPayload::ConfigChange(c) => Some(c.membership.clone()),
                EntryPayload::SnapshotPointer(s) => Some(s.membership.clone()),
                _ => None,
            })
        {
            *self.membership.write() = membership;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kv::MemoryDb;
    use std::time::Duration;

    #[tokio::test]
    async fn test_apply_and_read_state() {
        let stor = CoherenceStorage::new(
            1,
            Arc::new(MemoryDb::new()),
            PathBuf::from(std::env::temp_dir()).join(format!("catedral-raft-test-{}", std::process::id())),
            100,
        );
        let resp = stor
            .apply_entry_to_state_machine(&1, &RaftRequest::UpdatePhi { phi: 0.99, timestamp: 42 })
            .await
            .unwrap();
        match resp {
            RaftResponse::PhiUpdated { phi, .. } => assert_eq!(phi, 0.99),
            _ => panic!("wrong response"),
        }
        assert_eq!(stor.get_state("coherence").unwrap().phi, 0.99);
    }

    #[tokio::test]
    async fn test_log_append_and_read() {
        let stor = CoherenceStorage::new(
            1,
            Arc::new(MemoryDb::new()),
            PathBuf::from(std::env::temp_dir()).join(format!("catedral-raft-test2-{}", std::process::id())),
            100,
        );
        let entries = stor.get_log_entries(0, 100).await.unwrap();
        assert_eq!(entries.len(), 0);
    }
}