//! PQC key management: an identity registry with rotation and revocation.
//! Keys are real (ML-KEM + ML-DSA); "rotation" mints a fresh keypair and
//! marks the predecessor as rotated, and "revocation" marks keys untrusted.

use crate::auth_kem::{PqcIdentity, PqcKeyMaterial};
use crate::sign::QuantumSignature;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyStatus {
    Active,
    Rotated,
    Revoked,
}

#[derive(Debug, Clone)]
pub struct WalletEntry {
    pub serial: u64,
    pub identity: PqcIdentity,
    pub status: KeyStatus,
}

/// Fingerprint of a public key (hex).
pub fn identity_fingerprint(id: &PqcIdentity) -> String {
    let mut h = Sha256::new();
    h.update(&id.dsa_public);
    h.update(&id.kem_public);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub struct PqcWallet {
    pub active: PqcKeyMaterial,
    pub active_serial: u64,
    pub history: Vec<WalletEntry>,
    pub next_serial: u64,
}

impl PqcWallet {
    pub fn new() -> Self {
        let active = PqcKeyMaterial::generate();
        let entry = WalletEntry {
            serial: 0,
            identity: active.identity.clone(),
            status: KeyStatus::Active,
        };
        Self { active, active_serial: 0, history: vec![entry], next_serial: 1 }
    }

    pub fn active_identity(&self) -> &PqcIdentity {
        &self.active.identity
    }

    /// Significant rotation: mark the current active key rotated, mint a fresh one.
    pub fn rotate(&mut self) -> &PqcIdentity {
        for e in self.history.iter_mut() {
            if e.serial == self.active_serial && e.status == KeyStatus::Active {
                e.status = KeyStatus::Rotated;
            }
        }
        let serial = self.next_serial;
        self.next_serial += 1;
        let new_active = PqcKeyMaterial::generate();
        self.history.push(WalletEntry {
            serial,
            identity: new_active.identity.clone(),
            status: KeyStatus::Active,
        });
        self.active = new_active;
        self.active_serial = serial;
        &self.active.identity
    }

    /// Revoke the current active key (e.g. suspected compromise).
    pub fn revoke_active(&mut self) {
        for e in self.history.iter_mut() {
            if e.serial == self.active_serial {
                e.status = KeyStatus::Revoked;
            }
        }
    }

    pub fn sign(&self, msg: &[u8]) -> QuantumSignature {
        self.active.signer.sign(msg)
    }

    pub fn is_active(&self, id: &PqcIdentity) -> bool {
        self.history.iter().any(|e| e.identity == *id && e.status == KeyStatus::Active)
    }

    pub fn find_entry(&self, id: &PqcIdentity) -> Option<&WalletEntry> {
        self.history.iter().find(|e| &e.identity == id)
    }
}

impl Default for PqcWallet {
    fn default() -> Self {
        Self::new()
    }
}