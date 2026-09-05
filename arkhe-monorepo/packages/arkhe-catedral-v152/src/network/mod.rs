//! Rede distribuída da Catedral OS v152.0 — sincronização IPFS com validação
//! cruzada (I194/I201).

pub mod ipfs_sync;

pub use ipfs_sync::{DistributedLedger, SyncHandover, ZenoVerifier};