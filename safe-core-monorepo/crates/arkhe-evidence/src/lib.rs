//! FI-011/FI-017 — hash-chained, tamper-evident evidence log. See
//! [`chain`] for the full explanation of how this differs from
//! `arkhe-web3-security`'s flat `EvidenceBus`.

#![deny(unsafe_code)]

pub mod chain;

pub use chain::{ChainError, EvidenceChain, EvidenceRecord, GENESIS_HASH};
