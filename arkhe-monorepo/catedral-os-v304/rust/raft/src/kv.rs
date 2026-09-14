// Catedral OS v304.0 — camada de armazenamento chave->valor.
//
// Abstrai o persistente subjacente (sled por padrão; RocksDB por feature)
// para que snapshots, log Raft e máquina de estados usem a mesma interface.
// I358: nenhum dado do hardware entra aqui sem passar pelo phy service.

use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
#[allow(dead_code)]
type _BoxErrorDoc = ();

#[derive(Debug, Error)]
pub enum KvError {
    #[error("sled: {0}")]
    Sled(#[from] sled::Error),
    #[cfg(feature = "rocksdb-backend")]
    #[error("rocksdb: {0}")]
    Rocks(#[from] rocksdb::Error),
    #[error("serialização: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl KvError {
    pub fn kind(&self) -> &'static str {
        match self {
            KvError::Sled(_) => "sled",
            #[cfg(feature = "rocksdb-backend")]
            KvError::Rocks(_) => "rocksdb",
            KvError::SerdeJson(_) => "serde_json",
            KvError::Io(_) => "io",
        }
    }
}

/// Contrato mínimo de persistência usado por log, hard state e snapshots.
pub trait Kvs: Send + Sync + 'static {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KvError>;
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), KvError>;
    fn delete(&self, key: &[u8]) -> Result<(), KvError>;
    /// Grava vários pares de uma vez (batching — I360 batching).
    fn batch_put(&self, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), KvError>;
    /// Varredura total do keyspace (usada por snapshots).
    fn scan(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KvError>;
}

impl dyn Kvs {
    pub fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, KvError> {
        match self.get(key.as_bytes())? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    pub fn put_json<T: Serialize>(&self, key: &str, value: &T) -> Result<(), KvError> {
        self.put(key.as_bytes(), &serde_json::to_vec(value)?)
    }
}

// ============================================================================ #
// Backend padrão: sled (puro Rust, zero build nativo)
// ============================================================================ #

pub struct SledStore {
    db: Arc<sled::Db>,
}

impl SledStore {
    pub fn open(path: &str) -> Result<Self, KvError> {
        let db = sled::open(path)?;
        Ok(Self { db: Arc::new(db) })
    }
}

impl Kvs for SledStore {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
        Ok(self.db.get(key)?.map(|v| v.to_vec()))
    }

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), KvError> {
        self.db.insert(key, value)?;
        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<(), KvError> {
        self.db.remove(key)?;
        Ok(())
    }

    fn batch_put(&self, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), KvError> {
        let mut batch = sled::Batch::default();
        for (key, value) in pairs {
            batch.insert(key.as_slice(), value.as_slice());
        }
        self.db.apply_batch(batch)?;
        Ok(())
    }

    fn scan(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KvError> {
        let mut out = Vec::new();
        for item in self.db.iter() {
            let (k, v) = item?;
            out.push((k.to_vec(), v.to_vec()));
        }
        Ok(out)
    }
}

// ============================================================================ #
// Backend opcional: RocksDB (produção; requer toolchain C++ no alvo)
// ============================================================================ #

#[cfg(feature = "rocksdb-backend")]
pub struct RocksStore {
    db: Arc<rocksdb::DB>,
}

#[cfg(feature = "rocksdb-backend")]
impl RocksStore {
    pub fn open(path: &str) -> Result<Self, KvError> {
        let mut opts = rocksdb::Options::new();
        opts.create_if_missing(true);
        opts.set_max_write_buffer_number(4);
        opts.set_write_buffer_size(64 * 1024 * 1024);
        opts.set_compression_type(rocksdb::DBCompressionType::Zstd);
        let db = rocksdb::DB::open(&opts, path)?;
        Ok(Self { db: Arc::new(db) })
    }
}

#[cfg(feature = "rocksdb-backend")]
impl Kvs for RocksStore {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
        Ok(self.db.get(key)?)
    }

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), KvError> {
        self.db.put(key, value)?;
        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<(), KvError> {
        self.db.delete(key)?;
        Ok(())
    }

    fn batch_put(&self, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), KvError> {
        let mut batch = rocksdb::WriteBatch::default();
        for (key, value) in pairs {
            batch.put(key.as_slice(), value.as_slice());
        }
        self.db.write(batch)?;
        Ok(())
    }

    fn scan(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KvError> {
        use rocksdb::IteratorMode;
        let mut out = Vec::new();
        for entry in self.db.iterator(IteratorMode::Start) {
            let (k, v) = entry?.into_pair();
            out.push((k.to_vec(), v.to_vec()));
        }
        Ok(out)
    }
}