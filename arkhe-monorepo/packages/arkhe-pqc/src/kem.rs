//! FIPS 203 ML-KEM lattice key encapsulation (formerly CRYSTALS-Kyber).

use ml_kem::{
    MlKem768, KeyInit,
    kem::{Ciphertext, Decapsulate, Encapsulate, Kem, KeyExport, TryKeyInit},
};

#[derive(Debug)]
pub enum KemError {
    InvalidEncoding,
}

impl std::fmt::Display for KemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "quantum kem error")
    }
}

impl std::error::Error for KemError {}

/// A serialized ML-KEM-768 keypair.
pub struct QuantumKem {
    pub decapsulation: Vec<u8>,
    pub encapsulation: Vec<u8>,
}

/// Generate a fresh ML-KEM-768 (decapsulation, encapsulation) keypair.
pub fn generate_kem_keypair() -> QuantumKem {
    let (dk, ek) = MlKem768::generate_keypair();
    QuantumKem {
        decapsulation: dk.to_bytes().as_slice().to_vec(),
        encapsulation: ek.to_bytes().as_slice().to_vec(),
    }
}

/// Encapsulate a shared secret to the holder of `encapsulation_key`.
/// Returns (ciphertext, shared_secret).
pub fn quantum_encapsulate(encapsulation_key: &[u8]) -> Result<(Vec<u8>, Vec<u8>), KemError> {
    let ek = ml_kem::EncapsulationKey768::new_from_slice(encapsulation_key)
        .map_err(|_| KemError::InvalidEncoding)?;
    let (ct, shared) = ek.encapsulate();
    Ok((ct.as_slice().to_vec(), shared.as_slice().to_vec()))
}

/// Decapsulate the shared secret from `ciphertext` using `decapsulation_key`.
pub fn quantum_decapsulate(
    decapsulation_key: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, KemError> {
    let dk = ml_kem::DecapsulationKey768::new_from_slice(decapsulation_key)
        .map_err(|_| KemError::InvalidEncoding)?;
    let ct = Ciphertext::<MlKem768>::try_from(ciphertext).map_err(|_| KemError::InvalidEncoding)?;
    Ok(dk.decapsulate(&ct).as_slice().to_vec())
}


