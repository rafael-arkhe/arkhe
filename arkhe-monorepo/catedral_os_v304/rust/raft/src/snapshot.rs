use crate::kv::KvStore;
use anyhow::{anyhow, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const SNAPSHOT_PREFIX: &str = "snapshot:";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub timestamp: u64,
    pub entries: HashMap<String, Vec<u8>>,
}

pub fn create_compact_snapshot(kv: &dyn KvStore) -> Result<Vec<u8>> {
    let mut entries = HashMap::new();
    for (k, v) in kv.iter_all().map_err(|e| anyhow!(e))? {
        let key = String::from_utf8(k)?;
        if key.starts_with(SNAPSHOT_PREFIX) || key == "__log__" {
            continue;
        }
        entries.insert(key, v);
    }
    let snapshot = Snapshot {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs(),
        entries,
    };
    let json = serde_json::to_vec(&snapshot)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&json)?;
    Ok(encoder.finish()?)
}

pub fn restore_compact_snapshot(kv: &dyn KvStore, data: &[u8]) -> Result<()> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    let snapshot: Snapshot = serde_json::from_slice(&decompressed)?;
    kv.batch_put(
        snapshot
            .entries
            .into_iter()
            .map(|(k, v)| (k.into_bytes(), v))
            .collect(),
    )
    .map_err(|e| anyhow!(e))?;
    Ok(())
}

pub fn list_snapshot_timestamps(kv: &dyn KvStore) -> Result<Vec<u64>> {
    let mut timestamps = Vec::new();
    for (k, _) in kv.iter_all().map_err(|e| anyhow!(e))? {
        let key = String::from_utf8(k)?;
        if let Some(ts) = key.strip_prefix(SNAPSHOT_PREFIX) {
            if let Ok(timestamp) = ts.parse::<u64>() {
                timestamps.push(timestamp);
            }
        }
    }
    timestamps.sort_unstable();
    Ok(timestamps)
}

pub fn prune_snapshots(kv: &dyn KvStore, keep_latest: usize) -> Result<usize> {
    let mut timestamps = list_snapshot_timestamps(kv)?;
    if timestamps.len() <= keep_latest {
        return Ok(0);
    }
    timestamps.sort_unstable_by(|a, b| b.cmp(a));
    let mut removed = 0;
    for ts in timestamps.into_iter().skip(keep_latest) {
        let key = format!("{}{}", SNAPSHOT_PREFIX, ts);
        kv.delete(key.as_bytes()).map_err(|e| anyhow!(e))?;
        removed += 1;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kv::MemoryDb;

    #[test]
    fn test_compact_roundtrip() {
        let kv = MemoryDb::new();
        kv.put(b"coherence".to_vec(), b"{\"phi\":0.99}".to_vec()).unwrap();
        let data = create_compact_snapshot(&kv).expect("snapshot failed");
        assert!(!data.is_empty());

        let kv2 = MemoryDb::new();
        restore_compact_snapshot(&kv2, &data).expect("restore failed");
        let restored = kv2.get(b"coherence").unwrap().unwrap();
        assert_eq!(&restored, b"{\"phi\":0.99}");
    }
}