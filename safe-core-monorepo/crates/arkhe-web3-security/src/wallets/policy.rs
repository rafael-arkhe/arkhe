//! FI-W20 — Wallet signing policy.
//!
//! Governs which signature scheme(s) [`crate::wallets::hybrid_wallet::ArkheWallet`]
//! requires, so the classical → post-quantum transition can be controlled
//! per-wallet without changing call sites.

/// Which signature scheme(s) an [`crate::wallets::hybrid_wallet::ArkheWallet`]
/// requires to accept a signature as valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletPolicy {
    /// Both the Ed25519 half AND the ML-DSA-65 half must verify. The
    /// recommended default during the classical → PQC migration: breaking
    /// either scheme alone is not enough to forge a signature.
    HybridOnly,
    /// Only the ML-DSA-65 half must verify; the Ed25519 half is still
    /// produced (for interop with counterparties that haven't migrated) but
    /// not required. Useful when a peer's verifier only understands PQC.
    PqcOnly,
    /// Same verification requirement as `PqcOnly`, but the wallet
    /// additionally refuses to sign or verify once its classical Ed25519
    /// public key has been exported via
    /// [`crate::wallets::hybrid_wallet::ArkheWallet::export_classical_public_key`]
    /// — the strictest posture, for wallets that must never depend on
    /// classical key material being exposed anywhere outside the process.
    StrictPqc,
}
