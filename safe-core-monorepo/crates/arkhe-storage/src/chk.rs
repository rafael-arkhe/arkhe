//! Content-addressed storage: convergent CHK encryption, chunking, and a
//! Merkle tree over chunk addresses.
//!
//! # Provenance
//! Ported from `arkhe-workspace-v5/arkhe/crates/arkhe-storage` (a sibling,
//! unrelated Cargo workspace) into `safe-core-monorepo` — this is the first
//! time this code has been type-checked or tested as part of this
//! workspace. The source crate's own `Cargo.toml` declared `arkhe-core`
//! and `arkhe-identity` as dependencies that its code never actually used
//! (confirmed by reading the full source: no `use` of either) — dropped
//! here rather than carried over as unused weight.
//!
//! # Cryptographic limit (labeled, not hidden)
//! The CHK scheme here uses a deterministic keystream derived via SHA-256
//! (Freenet/Tahoe-LAFS style). This gives content-addressed deduplication,
//! but is **not AEAD**: there is no integrity authentication via MAC —
//! `ChunkStore::put` only rejects a ciphertext/address mismatch, and
//! `load_object` only rejects a tampered Merkle root. Neither is a
//! substitute for a real MAC. [`crate::encapsulation`] adds real AEAD
//! specifically for the cross-agent key-sharing path — it does not change
//! anything about local at-rest storage using this module directly.
#![allow(clippy::module_name_repetitions)]

use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// 32-byte digest (SHA-256). Content address.
pub type Hash = [u8; 32];

fn sha256(data: &[u8]) -> Hash {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

/// Convergent CHK key: hash of the plaintext itself. Identical content
/// therefore produces an identical key, giving natural deduplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChkKey(pub Hash);

/// A CHK block: the convergent key (needed to decrypt) plus a ciphertext
/// addressed by its own hash.
#[derive(Debug, Clone)]
pub struct ChkBlock {
    /// Convergent key (hash of the plaintext). Required to decrypt.
    pub key: ChkKey,
    /// Storage address = hash of the ciphertext.
    pub address: Hash,
    /// Ciphertext (plaintext XOR keystream(key)).
    pub ciphertext: Vec<u8>,
}

/// Deterministic keystream derived from the convergent key (SHA-256 in
/// counter mode). Not AEAD — see the module-level limit note.
fn keystream(key: &ChkKey, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut counter: u64 = 0;
    while out.len() < len {
        let mut h = Sha256::new();
        h.update(key.0);
        h.update(counter.to_le_bytes());
        out.extend_from_slice(&h.finalize());
        counter += 1;
    }
    out.truncate(len);
    out
}

/// Encrypts a plaintext into a convergent CHK block.
pub fn chk_encode(plaintext: &[u8]) -> ChkBlock {
    let key = ChkKey(sha256(plaintext));
    let ks = keystream(&key, plaintext.len());
    let ciphertext: Vec<u8> = plaintext.iter().zip(&ks).map(|(p, k)| p ^ k).collect();
    let address = sha256(&ciphertext);
    ChkBlock { key, address, ciphertext }
}

/// Recovers the plaintext from a CHK block, verifying convergence.
/// Errors if the recovered plaintext does not reproduce the key (CHK
/// integrity check).
pub fn chk_decode(block: &ChkBlock) -> Result<Vec<u8>, StorageError> {
    let ks = keystream(&block.key, block.ciphertext.len());
    let plaintext: Vec<u8> = block.ciphertext.iter().zip(&ks).map(|(c, k)| c ^ k).collect();
    if ChkKey(sha256(&plaintext)) != block.key {
        return Err(StorageError::ConvergenceMismatch);
    }
    Ok(plaintext)
}

/// Content-addressed storage abstraction.
pub trait ChunkStore {
    /// Writes a block under its address (hash of the ciphertext). Idempotent.
    fn put(&mut self, address: Hash, ciphertext: Vec<u8>) -> Result<(), StorageError>;
    /// Reads a block by address.
    fn get(&self, address: &Hash) -> Result<Vec<u8>, StorageError>;
    /// Number of distinct blocks stored.
    fn len(&self) -> usize;
    /// True if no blocks are stored.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// In-memory `ChunkStore` implementation.
#[derive(Debug, Default)]
pub struct InMemoryStore {
    blocks: HashMap<Hash, Vec<u8>>,
}

impl InMemoryStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ChunkStore for InMemoryStore {
    fn put(&mut self, address: Hash, ciphertext: Vec<u8>) -> Result<(), StorageError> {
        if sha256(&ciphertext) != address {
            return Err(StorageError::AddressMismatch);
        }
        self.blocks.insert(address, ciphertext); // idempotent: dedup
        Ok(())
    }
    fn get(&self, address: &Hash) -> Result<Vec<u8>, StorageError> {
        self.blocks.get(address).cloned().ok_or(StorageError::NotFound)
    }
    fn len(&self) -> usize {
        self.blocks.len()
    }
}

/// Internal Merkle node: hash(left || right).
fn merkle_parent(left: &Hash, right: &Hash) -> Hash {
    let mut h = Sha256::new();
    h.update(left);
    h.update(right);
    h.finalize().into()
}

/// Computes the Merkle root of a sequence of leaves. An odd level promotes
/// the last node (duplicates the final leaf).
pub fn merkle_root(leaves: &[Hash]) -> Option<Hash> {
    if leaves.is_empty() {
        return None;
    }
    let mut level: Vec<Hash> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let parent = match pair {
                [a, b] => merkle_parent(a, b),
                [a] => merkle_parent(a, a),
                _ => unreachable!(),
            };
            next.push(parent);
        }
        level = next;
    }
    Some(level[0])
}

/// Manifest of a chunked object: chunk addresses plus the Merkle root.
#[derive(Debug, Clone)]
pub struct HashtreeManifest {
    /// CHK keys of each chunk, in order (needed to decrypt).
    pub chunk_keys: Vec<ChkKey>,
    /// Storage addresses of each chunk, in order.
    pub chunk_addresses: Vec<Hash>,
    /// Merkle root over the chunk addresses.
    pub root: Hash,
    /// Length of the original plaintext, in bytes.
    pub total_len: usize,
}

/// Chunks, encrypts (CHK), and stores an object; returns its manifest.
pub fn store_object(
    store: &mut dyn ChunkStore,
    data: &[u8],
    chunk_size: usize,
) -> Result<HashtreeManifest, StorageError> {
    if chunk_size == 0 {
        return Err(StorageError::InvalidChunkSize);
    }
    let mut chunk_keys = Vec::new();
    let mut chunk_addresses = Vec::new();
    for chunk in data.chunks(chunk_size) {
        let block = chk_encode(chunk);
        store.put(block.address, block.ciphertext.clone())?;
        chunk_keys.push(block.key);
        chunk_addresses.push(block.address);
    }
    // Empty object: one canonical empty-hash leaf, so the root is defined.
    if chunk_addresses.is_empty() {
        let empty = chk_encode(&[]);
        store.put(empty.address, empty.ciphertext.clone())?;
        chunk_keys.push(empty.key);
        chunk_addresses.push(empty.address);
    }
    let root = merkle_root(&chunk_addresses).ok_or(StorageError::EmptyTree)?;
    Ok(HashtreeManifest { chunk_keys, chunk_addresses, root, total_len: data.len() })
}

/// Recovers and reassembles an object from its manifest, verifying the root.
pub fn load_object(store: &dyn ChunkStore, manifest: &HashtreeManifest) -> Result<Vec<u8>, StorageError> {
    let recomputed = merkle_root(&manifest.chunk_addresses).ok_or(StorageError::EmptyTree)?;
    if recomputed != manifest.root {
        return Err(StorageError::RootMismatch);
    }
    let mut out = Vec::with_capacity(manifest.total_len);
    for (key, addr) in manifest.chunk_keys.iter().zip(&manifest.chunk_addresses) {
        let ciphertext = store.get(addr)?;
        let block = ChkBlock { key: *key, address: *addr, ciphertext };
        out.extend_from_slice(&chk_decode(&block)?);
    }
    out.truncate(manifest.total_len);
    Ok(out)
}

/// Storage errors.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    /// Block not found in the store.
    #[error("block not found")]
    NotFound,
    /// Given address does not match the ciphertext's hash.
    #[error("address does not match ciphertext hash")]
    AddressMismatch,
    /// Decrypted plaintext does not reproduce the convergent key (CHK integrity).
    #[error("decrypted plaintext does not reproduce the convergent key")]
    ConvergenceMismatch,
    /// Recomputed Merkle root differs from the one declared in the manifest.
    #[error("recomputed Merkle root does not match the manifest")]
    RootMismatch,
    /// `chunk_size` was zero.
    #[error("chunk_size must not be zero")]
    InvalidChunkSize,
    /// Attempted to compute the root of an empty tree.
    #[error("cannot compute the root of an empty tree")]
    EmptyTree,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chk_roundtrip() {
        let data = b"ARKHE evidence over assertion";
        let block = chk_encode(data);
        assert_eq!(chk_decode(&block).unwrap(), data);
    }

    #[test]
    fn chk_is_convergent_and_deduplicates() {
        let a = chk_encode(b"identical payload");
        let b = chk_encode(b"identical payload");
        assert_eq!(a.key, b.key);
        assert_eq!(a.address, b.address);
        let c = chk_encode(b"different payload");
        assert_ne!(a.key, c.key);
    }

    #[test]
    fn chk_detects_broken_convergence() {
        let mut block = chk_encode(b"tamper me");
        block.ciphertext[0] ^= 0xFF;
        assert_eq!(chk_decode(&block), Err(StorageError::ConvergenceMismatch));
    }

    #[test]
    fn store_rejects_a_forged_address() {
        let mut store = InMemoryStore::new();
        let fake_addr = [0u8; 32];
        assert_eq!(store.put(fake_addr, b"payload".to_vec()), Err(StorageError::AddressMismatch));
        assert!(store.is_empty());
    }

    #[test]
    fn store_get_of_missing_block_fails() {
        let store = InMemoryStore::new();
        assert_eq!(store.get(&[7u8; 32]), Err(StorageError::NotFound));
    }

    #[test]
    fn merkle_root_is_deterministic_and_sensitive() {
        let leaves = [[1u8; 32], [2u8; 32], [3u8; 32]];
        let r1 = merkle_root(&leaves).unwrap();
        let r2 = merkle_root(&leaves).unwrap();
        assert_eq!(r1, r2, "root must be deterministic");
        let mut altered = leaves;
        altered[2] = [9u8; 32];
        assert_ne!(merkle_root(&altered).unwrap(), r1);
        let reordered = [[2u8; 32], [1u8; 32], [3u8; 32]];
        assert_ne!(merkle_root(&reordered).unwrap(), r1);
    }

    #[test]
    fn merkle_root_of_empty_leaves_is_none() {
        assert_eq!(merkle_root(&[]), None);
    }

    #[test]
    fn object_roundtrip_multichunk() {
        let mut store = InMemoryStore::new();
        // 2600 bytes with a 1024-byte chunk size => 3 chunks (1024, 1024, 552).
        let data: Vec<u8> = (0..2600u32).map(|i| (i % 251) as u8).collect();
        let manifest = store_object(&mut store, &data, 1024).unwrap();
        assert_eq!(manifest.chunk_addresses.len(), 3);
        assert_eq!(manifest.total_len, 2600);
        let recovered = load_object(&store, &manifest).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn object_dedups_identical_chunks() {
        let mut store = InMemoryStore::new();
        let data = vec![0xABu8; 2048]; // two identical 1024-byte chunks
        let manifest = store_object(&mut store, &data, 1024).unwrap();
        assert_eq!(manifest.chunk_addresses.len(), 2, "manifest lists 2 chunks");
        assert_eq!(store.len(), 1, "but the store holds only 1 block (dedup)");
        assert_eq!(load_object(&store, &manifest).unwrap(), data);
    }

    #[test]
    fn load_rejects_a_tampered_root() {
        let mut store = InMemoryStore::new();
        let data = b"integrity-checked object".to_vec();
        let mut manifest = store_object(&mut store, &data, 8).unwrap();
        manifest.root[0] ^= 0xFF;
        assert_eq!(load_object(&store, &manifest), Err(StorageError::RootMismatch));
    }

    #[test]
    fn empty_object_has_a_defined_root() {
        let mut store = InMemoryStore::new();
        let manifest = store_object(&mut store, &[], 1024).unwrap();
        assert_eq!(manifest.total_len, 0);
        assert_eq!(load_object(&store, &manifest).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn chunk_size_zero_is_an_error() {
        let mut store = InMemoryStore::new();
        assert!(matches!(store_object(&mut store, b"x", 0), Err(StorageError::InvalidChunkSize)));
    }
}
