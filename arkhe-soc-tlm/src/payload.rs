//! Canonical AOTB payload — the single source of truth shared between the SoC
//! signer and any verifier (smartphone / parachain pallet).
//!
//! Layout (136 bytes, fixed):
//!   0x00  session_id     16   bytes
//!   0x10  sequence        8   u64 little-endian
//!   0x18  nonce           8   u64 little-endian
//!   0x20  proof_hash     32   bytes
//!   0x40  domain_values  64   8 × f64 big-endian (IEEE-754 network order)
//!   0x80  weights         8   u8
//!                       ----
//!                        136
//! The signature (Ed25519, 64 bytes) is transmitted separately, never inside
//! the payload. The payload is what gets signed and hashed.

pub const DOMAIN_NODES: usize = 8;
pub const PAYLOAD_SIZE: usize = 16 + 8 + 8 + 32 + 8 * DOMAIN_NODES + DOMAIN_NODES; // 136

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AotbPayload {
    pub session_id: [u8; 16],
    pub sequence: u64, // LE
    pub nonce: u64,    // LE
    pub proof_hash: [u8; 32],
    pub domain_values: [f64; DOMAIN_NODES], // BE
    pub weights: [u8; DOMAIN_NODES],
}

impl AotbPayload {
    /// Deterministic 136-byte canonical serialization (integers LE, f64 BE).
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(PAYLOAD_SIZE);
        b.extend_from_slice(&self.session_id);
        b.extend_from_slice(&self.sequence.to_le_bytes());
        b.extend_from_slice(&self.nonce.to_le_bytes());
        b.extend_from_slice(&self.proof_hash);
        for v in &self.domain_values {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.extend_from_slice(&self.weights);
        debug_assert_eq!(b.len(), PAYLOAD_SIZE);
        b
    }

    /// SHA-256 over the canonical bytes.
    pub fn hash(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        Sha256::digest(self.to_canonical_bytes()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_length_is_136() {
        let p = AotbPayload {
            session_id: [1; 16],
            sequence: 7,
            nonce: 7,
            proof_hash: [2; 32],
            domain_values: [0.5; DOMAIN_NODES],
            weights: [100; DOMAIN_NODES],
        };
        assert_eq!(p.to_canonical_bytes().len(), PAYLOAD_SIZE);
        assert_eq!(PAYLOAD_SIZE, 136);
    }

    #[test]
    fn field_offsets_are_stable() {
        let p = AotbPayload {
            session_id: [0xAB; 16],
            sequence: 0x0102_0304_0506_0708,
            nonce: 0,
            proof_hash: [0; 32],
            domain_values: [0.0; DOMAIN_NODES],
            weights: [0; DOMAIN_NODES],
        };
        let b = p.to_canonical_bytes();
        assert_eq!(&b[0..16], &[0xAB; 16]); // session_id at 0x00
        assert_eq!(&b[16..24], &0x0102_0304_0506_0708u64.to_le_bytes()); // seq LE at 0x10
        assert_eq!(&b[0x40..0x48], &0.0f64.to_be_bytes()); // first f64 BE at 0x40
    }
}
