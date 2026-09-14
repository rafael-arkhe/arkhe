mod kv;
mod snapshot;
mod storage;

use crate::storage::{CoherenceStorage, RaftRequest};
use anyhow::Result;
use async_raft::raft::{AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse, VoteRequest, VoteResponse};
use async_raft::raft::{ClientWriteRequest, EntryPayload};
use async_raft::storage::InitialState;
use async_raft::{Config, Raft, RaftNetwork, RaftStorage};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

const SNAPSHOT_DIR: &str = "/var/lib/catedral/raft/snapshots";

// ============================================================================
// Network adapter
//
// In production this maps to a gRPC / HTTP transport (one per peer). The RPC
// types below are the wire types; a concrete transport should serialize them
// and dispatch to the corresponding `Raft::append_entries` / `vote` /
// `install_snapshot` methods on the remote node's Raft instance.
// ============================================================================

struct CatedralRaftNetwork {}

#[async_trait::async_trait]
impl RaftNetwork<RaftRequest> for CatedralRaftNetwork {
    async fn append_entries(
        &self,
        target: u64,
        rpc: AppendEntriesRequest<RaftRequest>,
    ) -> Result<AppendEntriesResponse> {
        warn!("append_entries -> {} : transport not bound", target);
        // Wire transport placeholder. Real implementation:
        //   let resp = reqwest::Client::new()
        //       .post(format!("http://node-{target}:50051/raft/append"))
        //       .json(&rpc).send().await?.json().await?;
        Ok(AppendEntriesResponse {
            term: rpc.term,
            success: false,
            conflict_opt: None,
        })
    }

    async fn install_snapshot(
        &self,
        target: u64,
        rpc: InstallSnapshotRequest,
    ) -> Result<InstallSnapshotResponse> {
        warn!("install_snapshot -> {} : transport not bound", target);
        Ok(InstallSnapshotResponse { term: rpc.term })
    }

    async fn vote(&self, target: u64, rpc: VoteRequest) -> Result<VoteResponse> {
        warn!("vote -> {} : transport not bound", target);
        Ok(VoteResponse {
            term: rpc.term,
            vote_granted: false,
        })
    }
}

// ============================================================================
// Entry point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_target(true)
        .init();

    let node_id: u64 = std::env::var("NODE_ID")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    let peers: Vec<u64> = std::env::var("PEERS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    info!("Catedral OS — Raft v304.0");
    info!("Node ID: {}", node_id);
    info!("Peers: {:?}", peers);

    let config = Config::build("catedral-raft".into())
        .heartbeat_interval(500)
        .election_timeout_min(1500)
        .election_timeout_max(3000)
        .validate()?;

    let kv: Arc<dyn kv::KvStore> = {
        #[cfg(feature = "rocksdb")]
        {
            let path = format!("/var/lib/catedral/raft/{}", node_id);
            Arc::new(kv::RocksDb::open(&path)?)
        }
        #[cfg(not(feature = "rocksdb"))]
        {
            Arc::new(kv::MemoryDb::new())
        }
    };

    let snapshot_dir = std::env::var("SNAPSHOT_DIR").unwrap_or_else(|_| SNAPSHOT_DIR.to_string());
    let storage = Arc::new(CoherenceStorage::new(
        node_id,
        kv,
        PathBuf::from(snapshot_dir),
        10_000,
    ));

    let network = Arc::new(CatedralRaftNetwork {});
    let raft = Raft::new(node_id, Arc::new(config), network, storage.clone());

    let initial: InitialState = storage.get_initial_state().await?;
    if initial.hard_state.current_term == 0 && initial.last_log_index == 0 {
        let mut members = HashSet::new();
        members.insert(node_id);
        for peer in peers {
            members.insert(peer);
        }
        if let Err(e) = raft.initialize(members).await {
            info!("initialize returned (expected): {:?}", e);
        }
    }

    // Client write dispatcher (demonstrates state machine interaction).
    let raft_clone = raft.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let req = ClientWriteRequest::new(RaftRequest::GetCoherence);
            match raft_clone.client_write(req).await {
                Ok(resp) => info!("coherence read: {:?}", resp.data),
                Err(e) => warn!("client_write failed: {:?}", e),
            }
        }
    });

    let mut rx = raft.metrics();
    loop {
        rx.changed().await.ok();
        let m = rx.borrow().clone();
        info!(
            "Metrics — state={:?}, leader={:?}, term={}, last_log={}, applied={}",
            m.state, m.current_leader, m.current_term, m.last_log_index, m.last_applied
        );
    }
}