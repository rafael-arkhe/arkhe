#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
};

use hkdf::Hkdf;
use sha3::{Digest, Sha3_256};
use zeroize::Zeroize;
use crate::error::IdentityError;
use crate::types::MasterIdentity;

/// Lowercase-hex encode, no external `hex` dependency (works in `no_std`).
fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

pub fn derive_master_identity(
    seed: &[u8; 64],
    agent_id: &str,
) -> Result<MasterIdentity, IdentityError> {
    let hk = Hkdf::<Sha3_256>::new(None, seed);

    let mut ml_dsa_65_seed = [0u8; 32];
    let info = format!("arkhe-v1-{}-mldsa65", agent_id);
    hk.expand(info.as_bytes(), &mut ml_dsa_65_seed)
        .map_err(|e| IdentityError::Derivation(format!("HKDF: {}", e)))?;

    // Bind the DID fingerprint to the actual public key (and agent id), so a
    // DID uniquely identifies one keypair rather than merely one master seed.
    // Two agents sharing a seed derive different keys and thus different DIDs.
    let (pk, mut sk) = pqc::ml_dsa_65_keypair_from_seed(&ml_dsa_65_seed)?;
    sk.zeroize(); // secret key is not needed here; wipe it promptly
    let mut hasher = Sha3_256::new();
    hasher.update(b"arkhe-did-v1");
    hasher.update(agent_id.as_bytes());
    hasher.update(&pk);
    let fingerprint = to_hex(&hasher.finalize()[..16]);

    Ok(MasterIdentity { ml_dsa_65_seed, agent_id: agent_id.to_string(), fingerprint })
}

pub mod pqc {
    #[cfg(not(feature = "std"))]
    use alloc::{format, vec::Vec};

    use crate::error::IdentityError;
    use libcrux_ml_dsa::ml_dsa_65;

    pub fn ml_dsa_65_keypair_from_seed(seed: &[u8; 32]) -> Result<(Vec<u8>, Vec<u8>), IdentityError> {
        let keypair = ml_dsa_65::generate_key_pair(*seed);
        Ok((keypair.verification_key.as_ref().to_vec(), keypair.signing_key.as_ref().to_vec()))
    }

    pub fn ml_dsa_65_sign(sk: &[u8], message: &[u8]) -> Result<Vec<u8>, IdentityError> {
        let sk_arr: [u8; 4032] = sk.try_into()
            .map_err(|_| IdentityError::Pqc("invalid sk length: expected 4032".into()))?;
        let signing_key = ml_dsa_65::MLDSA65SigningKey::new(sk_arr);
        let randomness = [0u8; 32];
        let sig = ml_dsa_65::sign(&signing_key, message, b"", randomness)
            .map_err(|e| IdentityError::Pqc(format!("sign failed: {:?}", e)))?;
        Ok(sig.as_ref().to_vec())
    }

    pub fn ml_dsa_65_verify(pk: &[u8], message: &[u8], signature: &[u8]) -> Result<bool, IdentityError> {
        let pk_arr: [u8; 1952] = pk.try_into()
            .map_err(|_| IdentityError::Pqc("invalid pk length: expected 1952".into()))?;
        let sig_arr: [u8; 3309] = signature.try_into()
            .map_err(|_| IdentityError::Pqc("invalid sig length: expected 3309".into()))?;
        let vk = ml_dsa_65::MLDSA65VerificationKey::new(pk_arr);
        let sig = ml_dsa_65::MLDSA65Signature::new(sig_arr);
        match ml_dsa_65::verify(&vk, message, b"", &sig) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}