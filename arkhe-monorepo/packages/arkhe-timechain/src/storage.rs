//! Persistent append-only storage for snapshots (the Shadow / block ledger on
//! disk).
//!
//! This closes the checklist item *"armazenamento persistente da Sombra"* with
//! a `Result`-based (no `unwrap`) file-backed store keyed by block height.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Anything that can be snapshotted across heights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot<T> {
    pub height: u64,
    pub payload: T,
    pub written_at: f64,
}

/// A persistent, height-keyed snapshot store.
///
/// Only the last snapshot per height is retained (append-only semantics per
/// key). All errors are surfaced via `Result` — never `unwrap`.
#[derive(Debug, Clone)]
pub struct PersistentStore<P: AsRef<Path>> {
    /// Directory where `.bin` snapshots live.
    pub dir: P,
}

impl<P: AsRef<Path>> PersistentStore<P> {
    /// Open (creating if needed) the store directory.
    pub fn open(dir: P) -> Result<Self, StorageError> {
        fs::create_dir_all(dir.as_ref())
            .map_err(|e| StorageError::Io { path: dir.as_ref().display().to_string(), source: e })?;
        Ok(Self { dir })
    }

    fn path_for(&self, height: u64) -> std::path::PathBuf {
        self.dir.as_ref().join(format!("snapshot_{height}.bin"))
    }

    /// Persist a snapshot, overwriting an existing height.
    pub fn save<T: Serialize>(&self, snapshot: &Snapshot<T>) -> Result<(), StorageError> {
        let bytes = bincode::serialize(snapshot).map_err(StorageError::Encode)?;
        fs::write(self.path_for(snapshot.height), bytes).map_err(|e| StorageError::Io {
            path: self.path_for(snapshot.height).display().to_string(),
            source: e,
        })
    }

    /// Load a snapshot by height.
    pub fn load<T: DeserializeOwned>(&self, height: u64) -> Result<Snapshot<T>, StorageError> {
        let path = self.path_for(height);
        let bytes = fs::read(&path).map_err(|e| StorageError::Read {
            path: path.display().to_string(),
            source: e,
        })?;
        bincode::deserialize(&bytes).map_err(StorageError::Decode)
    }

    /// List all heights currently stored, ascending.
    pub fn list_heights(&self) -> Result<Vec<u64>, StorageError> {
        let mut out = Vec::new();
        for entry in fs::read_dir(self.dir.as_ref()).map_err(StorageError::ReadDir)? {
            let entry = entry.map_err(StorageError::ReadDir)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(stripped) = name
                .strip_prefix("snapshot_")
                .and_then(|s| s.strip_suffix(".bin"))
            {
                if let Ok(h) = stripped.parse::<u64>() {
                    out.push(h);
                }
            }
        }
        out.sort_unstable();
        Ok(out)
    }
}

/// Storage failure modes.
#[derive(Debug)]
pub enum StorageError {
    Io { path: String, source: std::io::Error },
    Read { path: String, source: std::io::Error },
    ReadDir(std::io::Error),
    Encode(bincode::Error),
    Decode(bincode::Error),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, .. } => write!(f, "I/O error on {path}"),
            Self::Read { path, .. } => write!(f, "read error on {path}"),
            Self::ReadDir(e) => write!(f, "read-dir error: {e}"),
            Self::Encode(e) => write!(f, "encode error: {e}"),
            Self::Decode(e) => write!(f, "decode error: {e}"),
        }
    }
}

impl std::error::Error for StorageError {}