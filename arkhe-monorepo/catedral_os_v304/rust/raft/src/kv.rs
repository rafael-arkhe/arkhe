use std::collections::BTreeMap;

/// Minimal ordered key-value store abstraction.
///
/// Two backends are provided:
///   * `RocksDb`  — production persistent storage (feature `rocksdb`).
///   * `MemoryDb` — in-process storage used for development, tests and
///                  environments where libclang/rocksdb cannot be built.
pub trait KvStore: Send + Sync + 'static {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, String>;
    fn put(&self, key: Vec<u8>, val: Vec<u8>) -> Result<(), String>;
    fn delete(&self, key: &[u8]) -> Result<(), String>;
    fn batch_put(&self, entries: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), String>;
    fn iter_all(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>, String>;
}

#[cfg(feature = "rocksdb")]
pub struct RocksDb {
    db: rocksdb::DB,
}

#[cfg(feature = "rocksdb")]
impl RocksDb {
    pub fn open(path: &str) -> Result<Self, String> {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        opts.set_max_write_buffer_number(4);
        opts.set_write_buffer_size(64 * 1024 * 1024);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        let db = rocksdb::DB::open(&opts, path)
            .map_err(|e| format!("failed to open rocksdb at {}: {}", path, e))?;
        Ok(Self { db })
    }
}

#[cfg(feature = "rocksdb")]
impl KvStore for RocksDb {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, String> {
        self.db
            .get(key)
            .map_err(|e| format!("rocksdb get failed: {}", e))
    }

    fn put(&self, key: Vec<u8>, val: Vec<u8>) -> Result<(), String> {
        self.db
            .put(key, val)
            .map_err(|e| format!("rocksdb put failed: {}", e))
    }

    fn delete(&self, key: &[u8]) -> Result<(), String> {
        self.db
            .delete(key)
            .map_err(|e| format!("rocksdb delete failed: {}", e))
    }

    fn batch_put(&self, entries: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), String> {
        let mut batch = rocksdb::WriteBatch::default();
        for (k, v) in entries {
            batch.put(k, v);
        }
        self.db
            .write(batch)
            .map_err(|e| format!("rocksdb batch failed: {}", e))
    }

    fn iter_all(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>, String> {
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        let mut out = Vec::new();
        for item in iter {
            let (k, v) = item.map_err(|e| format!("rocksdb iteration failed: {}", e))?;
            out.push((k.to_vec(), v.to_vec()));
        }
        Ok(out)
    }
}

pub struct MemoryDb {
    map: parking_lot::RwLock<BTreeMap<Vec<u8>, Vec<u8>>>,
}

impl MemoryDb {
    pub fn new() -> Self {
        Self {
            map: parking_lot::RwLock::new(BTreeMap::new()),
        }
    }
}

impl Default for MemoryDb {
    fn default() -> Self {
        Self::new()
    }
}

impl KvStore for MemoryDb {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, String> {
        Ok(self.map.read().get(key).cloned())
    }

    fn put(&self, key: Vec<u8>, val: Vec<u8>) -> Result<(), String> {
        self.map.write().insert(key, val);
        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<(), String> {
        self.map.write().remove(key);
        Ok(())
    }

    fn batch_put(&self, entries: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), String> {
        let mut map = self.map.write();
        for (k, v) in entries {
            map.insert(k, v);
        }
        Ok(())
    }

    fn iter_all(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>, String> {
        Ok(self.map.read().iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }
}