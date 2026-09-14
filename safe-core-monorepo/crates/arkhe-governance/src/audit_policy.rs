// crates/arkhe-governance/src/audit_policy.rs
//! Accountability records and a hash-linked audit chain.
//!
//! This module predates the constitutional machinery in [`crate::constitution`]
//! and is independent of it: [`crate::constitution::AuditLog`] is the
//! append-only trail of governance events, while [`AuditChain`] here is a
//! general-purpose record ledger whose hashes are supplied by the caller.
//!
//! NOTE (first compilation): this module used to `use
//! arkhe_identity::ArkheDid;`. No such type exists anywhere in the workspace —
//! `arkhe-identity` exports the GDID types only, and the non-member crate
//! `safe-core-crypto` names its DID type `DidArkhe`. The subject is therefore
//! carried as its textual DID representation, which is the minimum change that
//! makes the module compile without editing another crate.

use arkhe_core::ArkheHash;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One accountability record: what was done, by whom, and when.
///
/// The hashes are supplied by the caller; this type does not compute them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountabilityRecord {
    /// Description of the action that was taken.
    pub action: String,
    /// DID of the subject the action was taken for or by.
    pub subject: String,
    /// When the action happened.
    pub timestamp: DateTime<Utc>,
    /// Hash of the preceding record, or `None` for the first record.
    pub previous_hash: Option<ArkheHash>,
    /// Hash of this record.
    pub current_hash: ArkheHash,
}

/// An append-only list of [`AccountabilityRecord`]s.
pub struct AuditChain {
    records: Vec<AccountabilityRecord>,
}

impl AuditChain {
    /// Creates an empty chain.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Appends a record.
    pub fn add(&mut self, record: AccountabilityRecord) {
        self.records.push(record);
    }

    /// Returns `true` when every record points at its predecessor.
    ///
    /// This checks the *links* only: the stored hashes are not recomputed, so
    /// it does not detect a record whose contents were altered while its
    /// `current_hash` was left in place.
    pub fn verify(&self) -> bool {
        self.records
            .windows(2)
            .all(|w| w[1].previous_hash == Some(w[0].current_hash))
    }
}

impl Default for AuditChain {
    fn default() -> Self {
        Self::new()
    }
}
