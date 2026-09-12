//! TPM 2.0 Storage Root Key (SRK) anchor for identity attestation.
//!
//! The identity seed is bound to the machine's TPM through the Microsoft
//! Platform Crypto Provider. A persisted RSA key created there is sealed by
//! the TPM SRK — the private part never leaves the chip — and the exported
//! public part yields a restart-stable, machine-unique fingerprint (SHA3-256).
//! [`load`] attaches that fingerprint to the observatory so an identity
//! attestation issued here cannot be replayed on different hardware
//! (Loopseal / Provenance-1: anchored provenance).
//!
//! Degradation: when no TPM is present (or the provider refuses access)
//! [`load`] returns `None` and the bridge runs unanchored, logging the reason.
//! This keeps the bridge functional on CI and non-TPM hosts.

use sha3::{Digest, Sha3_256};

/// TPM-provided anchor material attached to an
/// [`Observatory`](crate::Observatory).
#[derive(Debug, Clone)]
pub struct TpmAnchor {
    /// CNG key-storage provider that sealed the key in the TPM.
    pub provider: &'static str,
    /// Key algorithm (RSA in the current implementation).
    pub algorithm: &'static str,
    /// Hex SHA3-256 of the exported TPM-sealed public key.
    pub fingerprint: String,
}

impl TpmAnchor {
    /// Sign `payload` using the TPM-held private key (RSA PKCS#1 v1.5,
    /// SHA-256 digest). The secret material never leaves the chip — the
    /// signature is produced by `NCryptSignHash` inside the TPM, so it can
    /// only be reproduced on this hardware (Provenance-1 anchored).
    pub fn sign(&self, payload: &[u8]) -> Option<Vec<u8>> {
        tpm_impl::sign_payload(payload)
    }

    /// Verify that `signature` is a valid TPM signature over `payload`,
    /// using this machine's SRK-anchored public key.
    pub fn verify(&self, payload: &[u8], signature: &[u8]) -> bool {
        tpm_impl::verify_payload(payload, signature)
    }

    /// A short human-readable anchor token (first 16 hex chars).
    pub fn short(&self) -> &str {
        &self.fingerprint[..16.min(self.fingerprint.len())]
    }
}

/// No TPM access outside Windows — run unanchored.
#[cfg(not(target_os = "windows"))]
mod tpm_impl {
    pub fn load() -> Option<super::TpmAnchor> {
        None
    }
    pub fn sign_payload(_payload: &[u8]) -> Option<Vec<u8>> {
        None
    }
    pub fn verify_payload(_payload: &[u8], _signature: &[u8]) -> bool {
        false
    }
}

#[cfg(target_os = "windows")]
mod tpm_impl {
    #![allow(unsafe_code)]
    use std::os::windows::ffi::OsStrExt;

    use super::{Digest, Sha3_256, TpmAnchor};

    /// CNG provider exposing TPM-sealed keys (firmware TPM 2.0).
    const PROVIDER: &str = "Microsoft Platform Crypto Provider";
    const KEY_NAME: &str = "ArkheSRKAnchor";
    const ALGORITHM: &str = "RSA";
    const BLOB: &str = "RSAPUBLICBLOB";

    const NTE_SUCCESS: u32 = 0x0000_0000;

    /// NCRYPT_PAD_PKCS1_FLAG — RSA PKCS#1 v1.5 signature padding.
    const NCRYPT_PAD_PKCS1_FLAG: u32 = 0x0000_0002;
    /// BCRYPT_SHA256_ALGORITHM wide string.
    const SHA256_ALG: &str = "SHA256";

    type NcryptHandle = usize;

    /// RSA PKCS#1 padding info struct consumed by NCryptSignHash /
    /// NCryptVerifySignature. Layout must match `BCRYPT_PKCS1_PADDING_INFO`.
    #[repr(C)]
    struct BcryptPkcs1PaddingInfo {
        alg_id: *const u16,
    }

    // SAFETY: only NCrypt.dll BNTP functions are called; all pointers are
    // derived from wide, null-terminated buffers or freshly created handles.
    #[link(name = "ncrypt", kind = "raw-dylib")]
    unsafe extern "system" {
        fn NCryptOpenStorageProvider(
            provider: *mut NcryptHandle,
            name: *const u16,
            flags: u32,
        ) -> u32;
        fn NCryptCreatePersistedKey(
            provider: NcryptHandle,
            key: *mut NcryptHandle,
            algorithm: *const u16,
            name: *const u16,
            legacy: u32,
            flags: u32,
        ) -> u32;
        fn NCryptOpenKey(
            provider: NcryptHandle,
            key: *mut NcryptHandle,
            name: *const u16,
            legacy: u32,
            flags: u32,
        ) -> u32;
        fn NCryptFinalizeKey(key: NcryptHandle, flags: u32) -> u32;
        fn NCryptExportKey(
            key: NcryptHandle,
            export: NcryptHandle,
            blob_type: *const u16,
            params: *const std::ffi::c_void,
            output: *mut u8,
            output_len: u32,
            written: *mut u32,
            flags: u32,
        ) -> u32;
        fn NCryptSignHash(
            key: NcryptHandle,
            padding: *const BcryptPkcs1PaddingInfo,
            hash: *const u8,
            hash_len: u32,
            signature: *mut u8,
            signature_len: u32,
            written: *mut u32,
            flags: u32,
        ) -> u32;
        fn NCryptVerifySignature(
            key: NcryptHandle,
            padding: *const BcryptPkcs1PaddingInfo,
            hash: *const u8,
            hash_len: u32,
            signature: *const u8,
            signature_len: u32,
            flags: u32,
        ) -> u32;
        fn NCryptFreeObject(object: NcryptHandle) -> u32;
    }

    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    pub fn load() -> Option<TpmAnchor> {
        let provider = open_provider()?;
        let anchor = anchor_from_provider(provider);
        unsafe { NCryptFreeObject(provider) };
        anchor
    }

    fn open_provider() -> Option<NcryptHandle> {
        let name = wide(PROVIDER);
        let mut provider: NcryptHandle = 0;
        let status = unsafe { NCryptOpenStorageProvider(&mut provider, name.as_ptr(), 0) };
        if status != NTE_SUCCESS {
            tracing::warn!("tpm anchor: open provider failed 0x{status:08X}; unanchored");
            return None;
        }
        Some(provider)
    }

    /// Open the persisted SRK-anchored key, creating+finalizing it the first
    /// time. Both public-key export and hardware signing share this handle.
    fn open_key(provider: NcryptHandle) -> Option<NcryptHandle> {
        let algorithm = wide(ALGORITHM);
        let key_name = wide(KEY_NAME);

        let mut key: NcryptHandle = 0;
        let mut status = unsafe { NCryptOpenKey(provider, &mut key, key_name.as_ptr(), 0, 0) };
        if status != NTE_SUCCESS {
            status = unsafe {
                NCryptCreatePersistedKey(provider, &mut key, algorithm.as_ptr(), key_name.as_ptr(), 0, 0)
            };
            if status == NTE_SUCCESS {
                status = unsafe { NCryptFinalizeKey(key, 0) };
            }
        }
        if status != NTE_SUCCESS {
            unsafe { NCryptFreeObject(key) };
            tracing::warn!("tpm anchor: open/create key failed 0x{status:08X}; unanchored");
            return None;
        }
        Some(key)
    }

    fn anchor_from_provider(provider: NcryptHandle) -> Option<TpmAnchor> {
        let key = open_key(provider)?;
        let mut blob = Vec::new();
        let exported = export_public(key, &mut blob);
        unsafe { NCryptFreeObject(key) };
        if !exported {
            return None;
        }
        let digest = Sha3_256::digest(&blob);
        let fingerprint = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        tracing::info!(
            "tpm anchor: SRK sealed (fingerprint {})",
            &fingerprint[..16.min(fingerprint.len())]
        );
        Some(TpmAnchor {
            provider: PROVIDER,
            algorithm: ALGORITHM,
            fingerprint,
        })
    }

    /// Sign a payload with the TPM-held private key. `NCryptSignHash` runs
    /// the RSA operation inside the chip: no secret ever leaves it. SHA-256
    /// is the digest negotiated with the padding info (PKCS#1 v1.5).
    pub fn sign_payload(payload: &[u8]) -> Option<Vec<u8>> {
        let provider = open_provider()?;
        let key = open_key(provider)?;
        let result = sign_with_key(key, payload);
        unsafe { NCryptFreeObject(key) };
        unsafe { NCryptFreeObject(provider) };
        result
    }

    /// Verify a signature against the TPM-anchored public key. Returns `true`
    /// only when `NCryptVerifySignature` confirms it, so a payload signed on
    /// any other machine cannot pass (Provenance-1).
    pub fn verify_payload(payload: &[u8], signature: &[u8]) -> bool {
        let provider = match open_provider() {
            Some(p) => p,
            None => return false,
        };
        let key = match open_key(provider) {
            Some(k) => k,
            None => {
                unsafe { NCryptFreeObject(provider) };
                return false;
            }
        };
        let ok = verify_with_key(key, payload, signature);
        unsafe { NCryptFreeObject(key) };
        unsafe { NCryptFreeObject(provider) };
        ok
    }

    fn sign_with_key(key: NcryptHandle, payload: &[u8]) -> Option<Vec<u8>> {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(payload);
        let alg = wide(SHA256_ALG);
        let padding = BcryptPkcs1PaddingInfo {
            alg_id: alg.as_ptr(),
        };

        // First pass: discover signature length.
        let mut needed: u32 = 0;
        unsafe {
            NCryptSignHash(
                key,
                &padding,
                digest.as_ptr(),
                digest.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut needed,
                NCRYPT_PAD_PKCS1_FLAG,
            )
        };
        if needed == 0 {
            return None;
        }

        let mut signature = vec![0u8; needed as usize];
        let mut written: u32 = 0;
        let status = unsafe {
            NCryptSignHash(
                key,
                &padding,
                digest.as_ptr(),
                digest.len() as u32,
                signature.as_mut_ptr(),
                signature.len() as u32,
                &mut written,
                NCRYPT_PAD_PKCS1_FLAG,
            )
        };
        if status != NTE_SUCCESS {
            tracing::warn!("tpm anchor: sign failed 0x{status:08X}");
            return None;
        }
        signature.truncate(written as usize);
        Some(signature)
    }

    fn verify_with_key(key: NcryptHandle, payload: &[u8], signature: &[u8]) -> bool {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(payload);
        let alg = wide(SHA256_ALG);
        let padding = BcryptPkcs1PaddingInfo {
            alg_id: alg.as_ptr(),
        };
        let status = unsafe {
            NCryptVerifySignature(
                key,
                &padding,
                digest.as_ptr(),
                digest.len() as u32,
                signature.as_ptr(),
                signature.len() as u32,
                NCRYPT_PAD_PKCS1_FLAG,
            )
        };
        status == NTE_SUCCESS
    }

    fn export_public(key: NcryptHandle, output: &mut Vec<u8>) -> bool {
        let blob_type = wide(BLOB);
        let mut needed: u32 = 0;
        unsafe {
            NCryptExportKey(
                key,
                0,
                blob_type.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                &mut needed,
                0,
            )
        };
        output.resize(needed as usize, 0);
        let mut written: u32 = 0;
        let status = unsafe {
            NCryptExportKey(
                key,
                0,
                blob_type.as_ptr(),
                std::ptr::null(),
                output.as_mut_ptr(),
                needed,
                &mut written,
                0,
            )
        };
        if status != NTE_SUCCESS {
            tracing::warn!("tpm anchor: export public blob failed 0x{status:08X}; unanchored");
            return false;
        }
        output.truncate(written as usize);
        true
    }
}

pub use tpm_impl::load;

#[cfg(all(test, target_os = "windows"))]
mod tests {
    #[test]
    fn load_is_stable_and_consistent() {
        let first = super::load();
        let second = super::load();
        if let (Some(a), Some(b)) = (&first, &second) {
            assert_eq!(
                a.fingerprint, b.fingerprint,
                "SRK fingerprint must be restart-stable"
            );
            assert_eq!(a.algorithm, "RSA");
        }
    }

    #[test]
    fn tpm_signature_round_trips_on_hardware() {
        let anchor = match super::load() {
            Some(a) => a,
            None => return, // no TPM on this host — nothing to prove
        };
        let payload = b"arkhe attestation signing self-test v1";
        let signature = anchor.sign(payload).expect("TPM must sign on hardware");
        assert!(anchor.verify(payload, &signature), "TPM signature must verify");
        let tampered = b"arkhe attestation signing self-test v2";
        assert!(
            !anchor.verify(tampered, &signature),
            "signature must not verify a different payload"
        );
    }
}