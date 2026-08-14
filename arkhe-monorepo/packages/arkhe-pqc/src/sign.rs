//! FIPS 204 ML-DSA lattice digital signatures (formerly CRYSTALS-Dilithium).

use ml_dsa::{
    EncodedVerifyingKey, Generate, Keypair, MlDsa87, Signer, Signature as MlsSignature, SigningKey,
    Verifier, VerifyingKey, signature::SignatureEncoding,
};
use ml_kem::KeyExport;

#[derive(Debug)]
pub enum SignatureError {
    InvalidPublicKey,
    InvalidSignature,
    Mismatch,
}

impl std::fmt::Display for SignatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "quantum signature error: {self:?}")
    }
}

impl std::error::Error for SignatureError {}

/// ML-DSA signature bundle: serialized public key, signature, and message.
#[derive(Debug, Clone)]
pub struct QuantumSignature {
    pub public: Vec<u8>,
    pub signature: Vec<u8>,
    pub message: Vec<u8>,
}

fn sign_with_key(sk: &SigningKey<MlDsa87>, message: &[u8]) -> QuantumSignature {
    let sig = sk.sign(message);
    QuantumSignature {
        public: sk.verifying_key().to_bytes().to_vec(),
        signature: sig.to_vec(),
        message: message.to_vec(),
    }
}

/// Generate a fresh ML-DSA-87 signing key and sign `message`.
pub fn quantum_sign(message: &[u8]) -> QuantumSignature {
    let sk = SigningKey::<MlDsa87>::generate();
    sign_with_key(&sk, message)
}

/// Verify `sig` over `message` against serialized public key `public`.
pub fn quantum_verify(
    public: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), SignatureError> {
let enc = EncodedVerifyingKey::<MlDsa87>::try_from(public)
        .map_err(|_| SignatureError::InvalidPublicKey)?;
    let pk: VerifyingKey<MlDsa87> = VerifyingKey::decode(&enc);
    let sig = MlsSignature::try_from(signature).map_err(|_| SignatureError::InvalidSignature)?;
    pk.verify(message, &sig).map_err(|_| SignatureError::Mismatch)
}

/// Ergonomic verify bound to a QuantumSignature.
pub fn quantum_verify_sig(s: &QuantumSignature) -> Result<(), SignatureError> {
    quantum_verify(&s.public, &s.message, &s.signature)
}

/// Reusable signer holding an ML-DSA-87 signing key.
pub struct QuantumSigner {
    signing_key: SigningKey<MlDsa87>,
}

impl QuantumSigner {
    pub fn new() -> Self {
        Self { signing_key: SigningKey::<MlDsa87>::generate() }
    }

    pub fn sign(&self, message: &[u8]) -> QuantumSignature {
        sign_with_key(&self.signing_key, message)
    }

    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), SignatureError> {
        quantum_verify(&self.public(), message, signature)
    }

    pub fn public(&self) -> Vec<u8> {
        self.signing_key.verifying_key().to_bytes().to_vec()
    }
}

impl Default for QuantumSigner {
    fn default() -> Self {
        Self::new()
    }
}

