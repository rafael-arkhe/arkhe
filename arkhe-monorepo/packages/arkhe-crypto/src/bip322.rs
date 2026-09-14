//! BIP-322 signature verification.
//!
//! # Feature gating
//!
//! This module compiles in two modes:
//!   - `feature = "bip322"`: real implementation backed by
//!     `bdk_message_signer` 0.2.0 + `bdk_wallet` 3.1.0.
//!   - otherwise: stub returning `CryptoError::FeatureDisabled`.
//!
//! The public signature is identical in both modes. This is verified
//! by the doctest at the bottom of this file, which type-checks under
//! both feature configurations.
//!
//! # API note (errata 2026-09-12)
//!
//! `wallet.verify_message(...)` is a **trait method** — it resolves
//! because `bdk_message_signer::MessageSigner` is implemented for
//! `bdk_wallet::Wallet`. The trait must be in scope. The underlying
//! free function `bdk_message_signer::verify::verify_signed_proof` is
//! functionally equivalent and may be preferred for explicitness.
//! Both are valid; this module uses the trait method to keep the call
//! site close to the upstream examples.
//!
//! # Security note (Fix #8)
//!
//! `MessageProof::from_base64` accepts both signed proofs and
//! serialized PSBTs. For **unfinalized** PSBTs, `bdk_message_signer`
//! routes to `verify_psbt_proof`, which — per the crate's own docs —
//! *"validates structure and amounts only, not cryptographic
//! signatures"*. Any caller relying on `Ok(true)` as proof of
//! signature validity would be misled. This module rejects
//! unfinalized PSBTs before calling `verify_message`. Finalized
//! PSBTs continue through the cryptographic path.

//! # Doctest
//!
//! The public signature `verify_bip322` compiles identically in both
//! feature configurations:
//!
//! ```no_run
//! use arkhe_crypto::bip322::verify_bip322;
//! use bitcoin::Network;
//!
//! let _f: fn(&str, &str, &str, &str, Network)
//!     -> arkhe_crypto::Result<bool> = verify_bip322;
//! ```

use crate::error::{CryptoError, Result};
use bitcoin::Network;

#[cfg(feature = "bip322")]
use bdk_message_signer::{MessageProof, MessageSigner};
#[cfg(feature = "bip322")]
use bdk_wallet::Wallet;
#[cfg(feature = "bip322")]
use bitcoin::address::NetworkUnchecked;
#[cfg(feature = "bip322")]
use bitcoin::Address;

/// Verify a BIP-322 signature against an address.
///
/// # Arguments
/// * `message`    — the signed message (UTF-8)
/// * `address`    — the address the signature claims to be from
/// * `signature`  — the BIP-322 proof, base64-encoded
/// * `descriptor` — a descriptor resolving to the same `script_pubkey`
///   as `address`. May be watch-only (public descriptor).
/// * `network`    — required, not derivable from the address
///   (testnet/regtest/signet share legacy prefixes)
///
/// # Errors
/// - `FeatureDisabled` if the `bip322` feature is not enabled.
/// - `InvalidAddress` if the address does not parse or does not
///   belong to the declared network.
/// - `InvalidSignature` if the proof is not valid base64.
/// - `Verification` if the proof is an unfinalized PSBT (Fix #8),
///   or if wallet construction or verification fails.
pub fn verify_bip322(
    message: &str,
    address: &str,
    signature: &str,
    descriptor: &str,
    network: Network,
) -> Result<bool> {
    #[cfg(not(feature = "bip322"))]
    {
        let _ = (message, address, signature, descriptor, network);
        Err(CryptoError::FeatureDisabled)
    }

    #[cfg(feature = "bip322")]
    {
        // Address must be parsed as `NetworkUnchecked`, then pinned to
        // the caller-supplied network. `Address<NetworkChecked>` has
        // no `FromStr`, and the network is not recoverable from the
        // address alone (testnet/regtest/signet share prefixes).
        let addr: Address = address
            .parse::<Address<NetworkUnchecked>>()
            .map_err(|e| CryptoError::InvalidAddress(format!("{e}")))?
            .require_network(network)
            .map_err(|e| CryptoError::InvalidAddress(format!("{e}")))?;

        // `MessageProof` has no `FromStr`; the explicit constructor
        // is `from_base64`. It may return `Signed(_)` or `Psbt(_)`.
        let proof = MessageProof::from_base64(signature)
            .map_err(|e| CryptoError::InvalidSignature(format!("{e}")))?;

        // Fix #8: unfinalized PSBTs route to `verify_psbt_proof`,
        // which validates structure and amounts only — NOT
        // cryptographic signatures (confirmed in source:
        // bdk_message_signer-0.2.0/src/sign.rs:79-104 routes
        // `MessageProof::Psbt` to verify_psbt_proof). Reject them
        // here so `Ok(true)` always implies a signature was checked.
        //
        // Predicate: finalized iff every input carries either a
        // `final_script_sig` or a `final_script_witness` (the fields
        // of `bitcoin::PsbtInput`; there is no `is_finalized` API in
        // bitcoin 0.32 / miniscript 12.3.7).
        if let MessageProof::Psbt(ref psbt) = proof {
            let finalized = psbt
                .inputs
                .iter()
                .all(|i| i.final_script_sig.is_some() || i.final_script_witness.is_some());
            if !finalized {
                return Err(CryptoError::Verification(
                    "unfinalized PSBT proof: rejected \
                     (no cryptographic verification)".into(),
                ));
            }
        }

        // `Wallet::create_single` requires `D: IntoWalletDescriptor
        // + Send + Clone + 'static`. A `&str` carries the caller's
        // lifetime and does not satisfy `'static`; `String` does.
        let wallet = Wallet::create_single(descriptor.to_string())
            .network(network)
            .create_wallet_no_persist()
            .map_err(|e| CryptoError::WalletConstruction(format!("{e}")))?;

        // `verify_message` comes from the `MessageSigner` trait,
        // which must be in scope (Fix #6).
        let result = wallet
            .verify_message(&proof, message, &addr)
            .map_err(|e| CryptoError::Verification(format!("{e}")))?;

        Ok(result.valid)
    }
}