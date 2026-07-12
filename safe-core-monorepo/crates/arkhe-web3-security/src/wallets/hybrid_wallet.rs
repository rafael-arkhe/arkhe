//! FI-W20 — Policy-aware hybrid (classical + post-quantum) wallet.
//!
//! Wraps [`arkhe_crypto_pqc::HybridKeyPair`] with a [`WalletPolicy`] that
//! controls how strictly a signature must satisfy both sub-schemes.

use arkhe_crypto_pqc::{generate_hybrid_keypair, HybridKeyPair, HybridSignature, PqcSignatureError};

use crate::wallets::policy::WalletPolicy;
use crate::InvariantVerdict;

#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error("PQC operation failed: {0}")]
    Pqc(#[from] PqcSignatureError),
    #[error("wallet policy StrictPqc violated: classical Ed25519 key was already exported")]
    ClassicalKeyExposed,
}

/// A wallet backed by a hybrid Ed25519 + ML-DSA-65 keypair, whose signing
/// and verification behavior is governed by a [`WalletPolicy`].
pub struct ArkheWallet {
    keypair: HybridKeyPair,
    policy: WalletPolicy,
    classical_key_exposed: bool,
}

impl ArkheWallet {
    /// Generates a fresh wallet under the given policy.
    pub fn generate(policy: WalletPolicy) -> Result<Self, WalletError> {
        let keypair = generate_hybrid_keypair()?;
        Ok(Self { keypair, policy, classical_key_exposed: false })
    }

    pub fn policy(&self) -> WalletPolicy {
        self.policy
    }

    /// Signs `msg`. The full hybrid signature (both sub-signatures) is
    /// always produced regardless of policy — policy only affects what
    /// [`Self::verify`] requires on the receiving end.
    pub fn sign(&self, msg: &[u8]) -> Result<HybridSignature, WalletError> {
        if self.policy == WalletPolicy::StrictPqc && self.classical_key_exposed {
            return Err(WalletError::ClassicalKeyExposed);
        }
        Ok(self.keypair.signing_key.sign(msg)?)
    }

    /// FI-W20: verifies `sig` over `msg` according to this wallet's policy.
    pub fn verify(&self, msg: &[u8], sig: &HybridSignature) -> InvariantVerdict {
        if self.policy == WalletPolicy::StrictPqc && self.classical_key_exposed {
            return InvariantVerdict::violated(
                "StrictPqc wallet cannot verify: classical Ed25519 key was already exported",
            );
        }

        match self.policy {
            WalletPolicy::HybridOnly => match self.keypair.verifying_key.verify(msg, sig) {
                Ok(()) => InvariantVerdict::Holds,
                Err(e) => InvariantVerdict::violated(e.to_string()),
            },
            WalletPolicy::PqcOnly | WalletPolicy::StrictPqc => {
                if self.keypair.verifying_key.verify_ml_dsa_only(msg, sig) {
                    InvariantVerdict::Holds
                } else {
                    InvariantVerdict::violated("ML-DSA-65 signature is invalid")
                }
            }
        }
    }

    /// Exports the classical Ed25519 public key bytes. Under
    /// `WalletPolicy::StrictPqc` this permanently taints the wallet: any
    /// subsequent `sign`/`verify` call fails, since FI-W20 requires a
    /// strict-PQC wallet to never rely on classical key material once it
    /// has left the process.
    pub fn export_classical_public_key(&mut self) -> [u8; 32] {
        self.classical_key_exposed = true;
        self.keypair.verifying_key.ed25519_bytes()
    }

    pub fn ml_dsa_public_key_bytes(&self) -> Vec<u8> {
        self.keypair.verifying_key.ml_dsa_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hybrid_only_requires_both_halves_valid() {
        let wallet = ArkheWallet::generate(WalletPolicy::HybridOnly).unwrap();
        let msg = b"transfer 10 ARKHE to bob";
        let mut sig = wallet.sign(msg).unwrap();
        assert!(wallet.verify(msg, &sig).holds());

        sig.ed_sig[0] ^= 0xFF;
        assert!(!wallet.verify(msg, &sig).holds());
    }

    #[test]
    fn pqc_only_ignores_broken_classical_half() {
        let wallet = ArkheWallet::generate(WalletPolicy::PqcOnly).unwrap();
        let msg = b"transfer 10 ARKHE to bob";
        let mut sig = wallet.sign(msg).unwrap();
        sig.ed_sig[0] ^= 0xFF;
        // PqcOnly doesn't care that the classical half is broken.
        assert!(wallet.verify(msg, &sig).holds());

        sig.ml_sig[0] ^= 0xFF;
        // But it still requires the ML-DSA-65 half to be valid.
        assert!(!wallet.verify(msg, &sig).holds());
    }

    #[test]
    fn strict_pqc_behaves_like_pqc_only_until_classical_key_is_exported() {
        let mut wallet = ArkheWallet::generate(WalletPolicy::StrictPqc).unwrap();
        let msg = b"transfer 10 ARKHE to bob";
        let sig = wallet.sign(msg).unwrap();
        assert!(wallet.verify(msg, &sig).holds());

        let _ = wallet.export_classical_public_key();

        // Once the classical key is exported, StrictPqc refuses to sign or verify at all.
        assert!(wallet.sign(msg).is_err());
        assert!(!wallet.verify(msg, &sig).holds());
    }
}
