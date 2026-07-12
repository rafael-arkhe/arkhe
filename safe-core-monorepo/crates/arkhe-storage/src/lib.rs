//! FI-032 — content-addressed storage: convergent CHK encryption and
//! Merkle-verified chunked objects ([`chk`], ported from a sibling
//! workspace — see that module's provenance note), plus ML-KEM-1024
//! encapsulation of a `ChkKey` for sharing a stored object with one
//! specific recipient agent ([`encapsulation`], new).

#![forbid(unsafe_code)]

pub mod chk;
pub mod encapsulation;

pub use chk::{
    chk_decode, chk_encode, merkle_root, load_object, store_object, ChkBlock, ChkKey, ChunkStore,
    Hash, HashtreeManifest, InMemoryStore, StorageError,
};
pub use encapsulation::{decapsulate_chk_key, encapsulate_chk_key, EncapsulatedChkKey, EncapsulationError};
