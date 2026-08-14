//! Quantum sealing of the time chain.
use crate::block::BlockHash;
use arkhe_pqc::sign::{quantum_sign, quantum_verify, SignatureError};

pub struct SealedBlock {
    pub hash: BlockHash,
    pub public: Vec<u8>,
    pub signature: Vec<u8>,
}

pub fn quantize_block(hash: &BlockHash) -> SealedBlock {
    let sig = quantum_sign(&hash.0);
    SealedBlock { hash: *hash, public: sig.public, signature: sig.signature }
}

pub fn quantum_verify_block(sealed: &SealedBlock) -> Result<(), SignatureError> {
    quantum_verify(&sealed.public, &sealed.hash.0, &sealed.signature)
}

#[cfg(test)]
mod tests_quantum {
    use super::*;
    use crate::block::hash_f64s;

    #[test]
    fn seal_binds_to_block_hash() {
        let h = hash_f64s(&[1.0, 2.0, 3.0]);
        let sealed = quantize_block(&BlockHash(h));
        quantum_verify_block(&sealed).expect("sealed block must verify");
    }

    #[test]
    fn seal_rejects_edited_hash() {
        let h = hash_f64s(&[1.0, 2.0, 3.0]);
        let sealed = quantize_block(&BlockHash(h));
        let mut forged = sealed.hash;
        forged.0[0] ^= 0x01;
        assert!(quantum_verify(&sealed.public, &forged.0, &sealed.signature).is_err());
    }
}
