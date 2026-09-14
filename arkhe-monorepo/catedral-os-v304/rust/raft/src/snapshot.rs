// Catedral OS v304.0 — Snapshots Raft com compressão gzip e recuperação (I360).
//
// O snapshot cobre somente o keyspace da máquina de estados (coherence:*),
// excluindo `log:` e snapshots anteriores, evitando o "snapshot do snapshot"
// que o rascunho original produzia (erro de projeto corrigido na validação).

use std::collections::BTreeMap;
use std::io::{Read, SeekFrom, Write};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite};
use tracing::info;

use crate::kv::Kvs;
use crate::StorageError;

/// Buffer de snapshot que satisfaz os traits async de IO exigidos por
/// `RaftStorage::Snapshot` no async-raft 0.6.1 (AsyncRead + AsyncWrite +
/// AsyncSeek). Escrito durante streaming de install; lido a partir do disco.
#[derive(Debug, Default)]
pub struct SnapshotBuf {
    data: Vec<u8>,
    pos: usize,
}

impl SnapshotBuf {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, pos: 0 }
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.data
    }
}

impl AsyncRead for SnapshotBuf {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let available = &self.data[self.pos.min(self.data.len())..];
        let len = std::cmp::min(available.len(), buf.remaining());
        buf.put_slice(&available[..len]);
        self.pos += len;
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for SnapshotBuf {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let pos = self.pos;
        if pos == self.data.len() {
            self.data.extend_from_slice(buf);
        } else {
            let end = std::cmp::min(pos + buf.len(), self.data.len());
            let target_len = end - pos;
            let source = &buf[..target_len];
            self.data[pos..end].copy_from_slice(source);
        }
        self.pos += buf.len();
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncSeek for SnapshotBuf {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> std::io::Result<()> {
        match position {
            SeekFrom::Start(n) => self.pos = n as usize,
            SeekFrom::End(n) => self.pos = (self.data.len() as i64 + n).max(0) as usize,
            SeekFrom::Current(n) => self.pos = (self.pos as i64 + n).max(0) as usize,
        }
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(self.pos as u64))
    }
}

pub const SNAPSHOT_PREFIX: &str = "snapshot:";
pub const SNAPSHOT_DATA_KEY: &str = "snapshot:current_data";
pub const SNAPSHOT_META_INDEX_KEY: &str = "snapshot:meta_index";
pub const SNAPSHOT_META_TERM_KEY: &str = "snapshot:meta_term";
pub const LOG_PREFIX: &str = "log:";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub timestamp: u64,
    /// keyspace da máquina de estados, comprimido.
    pub entries: BTreeMap<String, Vec<u8>>,
}

fn is_snapshot_key(key: &str) -> bool {
    key.starts_with(SNAPSHOT_PREFIX)
}

/// Cria um snapshot do keyspace da máquina de estados.
pub fn create_snapshot(store: &dyn Kvs) -> Result<Snapshot, StorageError> {
    let mut entries = BTreeMap::new();
    for (key, value) in store.scan()? {
        let key_str = String::from_utf8(key)?;
        if key_str.starts_with(LOG_PREFIX) || is_snapshot_key(&key_str) {
            continue;
        }
        entries.insert(key_str, value);
    }
    Ok(Snapshot {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| StorageError::Time(e.to_string()))?
            .as_secs(),
        entries,
    })
}

/// Restaura (substitui) o keyspace da máquina de estados a partir de um snapshot.
pub fn restore_snapshot(store: &dyn Kvs, snapshot: &Snapshot) -> Result<(), StorageError> {
    let current = store.scan()?;
    let mut removals = Vec::new();
    for (key, _) in current {
        let key_str = String::from_utf8(key)?;
        if key_str.starts_with(LOG_PREFIX) || is_snapshot_key(&key_str) {
            continue;
        }
        removals.push(key_str);
    }
    for key in removals {
        store.delete(key.as_bytes())?;
    }

    let mut pairs = Vec::with_capacity(snapshot.entries.len());
    for (key, value) in &snapshot.entries {
        pairs.push((key.as_bytes().to_vec(), value.clone()));
    }
    store.batch_put(pairs)?;
    info!(
        "snapshot restaurado: {} entradas (ts={})",
        snapshot.entries.len(),
        snapshot.timestamp
    );
    Ok(())
}

/// Comprime o snapshot em bytes (gzip).
pub fn compress(snapshot: &Snapshot) -> Result<Vec<u8>, StorageError> {
    let json = serde_json::to_vec(snapshot)?;
    let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&json)?;
    Ok(encoder.finish()?)
}

/// Descomprime bytes de volta em Snapshot.
pub fn decompress(data: &[u8]) -> Result<Snapshot, StorageError> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(serde_json::from_slice(&decompressed)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kv::SledStore;

    fn tmp_store() -> SledStore {
        let dir = std::env::temp_dir().join(format!("catedral-raft-test-{}", uuid::Uuid::new_v4()));
        SledStore::open(dir.to_str().unwrap()).unwrap()
    }

    #[test]
    fn snapshot_roundtrip_compression() -> Result<(), StorageError> {
        let store = tmp_store();
        let kvs: &dyn Kvs = &store;
        kvs.put_json("coherence:state", &serde_json::json!({"phi": 0.994}))?;
        store.put(b"log:5", b"payload")?;
        store.put(b"snapshot:old", b"x")?;

        let snap = create_snapshot(kvs)?;
        assert!(snap.entries.contains_key("coherence:state"));
        assert!(!snap.entries.contains_key("log:5"), "log não deve ir para snapshot");
        assert!(!snap.entries.contains_key("snapshot:old"), "snapshots não devem ser inclusos");

        let packed = compress(&snap)?;
        let unpacked = decompress(&packed)?;
        assert_eq!(unpacked.entries, snap.entries);
        Ok(())
    }
}