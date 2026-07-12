//! FI-071 (authenticated messages) + FI-077 (no message causes a panic).
//!
//! [`Message::decode`] is written so that every slicing operation is
//! preceded by an explicit bounds check — no operation that could panic on
//! attacker-controlled input (`bytes[a..b]` without checking `b <=
//! bytes.len()` first, `.unwrap()` on a length that depends on the input).
//! See `decode_never_panics_on_arbitrary_bytes` for a battery of malformed
//! inputs run through `std::panic::catch_unwind` to confirm this directly,
//! not just by code inspection.

use arkhe_crypto_pqc::{HybridKeyPair, HybridSignature, HybridVerifyingKey, PqcSignatureError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub sender_id: String,
    pub nonce: u64,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SignedMessage {
    pub message: Message,
    pub signature: HybridSignature,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MessageError {
    #[error("message is truncated: expected {expected} more bytes, only {remaining} remain")]
    Truncated { expected: usize, remaining: usize },
    #[error("sender_id is not valid UTF-8")]
    InvalidSenderId,
}

impl Message {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + self.sender_id.len() + 8 + 8 + self.payload.len());
        buf.extend_from_slice(&(self.sender_id.len() as u32).to_be_bytes());
        buf.extend_from_slice(self.sender_id.as_bytes());
        buf.extend_from_slice(&self.nonce.to_be_bytes());
        buf.extend_from_slice(&(self.payload.len() as u64).to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// FI-077: returns `Err` for any malformed input, never panics —
    /// including a `sender_id`/`payload` length prefix claiming more bytes
    /// than actually follow, truncated length prefixes themselves, and
    /// non-UTF-8 `sender_id` bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
        let mut offset = 0usize;

        let sender_id_len = read_u32(bytes, &mut offset)? as usize;
        let sender_id_bytes = read_slice(bytes, &mut offset, sender_id_len)?;
        let sender_id = String::from_utf8(sender_id_bytes.to_vec()).map_err(|_| MessageError::InvalidSenderId)?;

        let nonce = read_u64(bytes, &mut offset)?;

        let payload_len = read_u64(bytes, &mut offset)? as usize;
        let payload = read_slice(bytes, &mut offset, payload_len)?.to_vec();

        Ok(Message { sender_id, nonce, payload })
    }
}

fn read_slice<'a>(bytes: &'a [u8], offset: &mut usize, len: usize) -> Result<&'a [u8], MessageError> {
    let remaining = bytes.len().saturating_sub(*offset);
    let end = match offset.checked_add(len) {
        Some(end) if end <= bytes.len() => end,
        _ => return Err(MessageError::Truncated { expected: len, remaining }),
    };
    let slice = &bytes[*offset..end];
    *offset = end;
    Ok(slice)
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, MessageError> {
    let slice = read_slice(bytes, offset, 4)?;
    // Safe: read_slice guarantees `slice.len() == 4` on success.
    Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], offset: &mut usize) -> Result<u64, MessageError> {
    let slice = read_slice(bytes, offset, 8)?;
    // Safe: read_slice guarantees `slice.len() == 8` on success.
    let mut arr = [0u8; 8];
    arr.copy_from_slice(slice);
    Ok(u64::from_be_bytes(arr))
}

/// FI-071: signs `message`'s canonical encoding.
pub fn sign_message(keypair: &HybridKeyPair, message: &Message) -> Result<SignedMessage, PqcSignatureError> {
    let signature = keypair.signing_key.sign(&message.encode())?;
    Ok(SignedMessage { message: message.clone(), signature })
}

/// FI-071: verifies `signed.signature` over `signed.message`'s canonical
/// encoding under `verifying_key`.
pub fn verify_message(verifying_key: &HybridVerifyingKey, signed: &SignedMessage) -> Result<(), PqcSignatureError> {
    verifying_key.verify(&signed.message.encode(), &signed.signature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_crypto_pqc::generate_hybrid_keypair;

    fn sample() -> Message {
        Message { sender_id: "vm-1".to_string(), nonce: 1, payload: b"hello".to_vec() }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let msg = sample();
        let decoded = Message::decode(&msg.encode()).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn decode_never_panics_on_arbitrary_bytes() {
        let cases: Vec<Vec<u8>> = vec![
            vec![],
            vec![0xFF],
            vec![0xFF, 0xFF, 0xFF, 0xFF],
            vec![0, 0, 0, 0],
            vec![0, 0, 0, 5, b'h', b'e'], // declares 5-byte sender_id, only 2 follow
            vec![255u8; 4].into_iter().chain(std::iter::repeat(0u8).take(2)).collect(),
            (0..300u32).map(|i| (i % 256) as u8).collect(),
            vec![0, 0, 0, 1, 0xFF], // 1-byte sender_id that isn't valid UTF-8 continuation on its own... still must not panic
        ];
        for case in cases {
            let result = std::panic::catch_unwind(|| Message::decode(&case));
            assert!(result.is_ok(), "Message::decode panicked on input: {case:?}");
        }
    }

    #[test]
    fn truncated_length_prefix_is_rejected_not_panicked() {
        // Only 2 bytes total — not even enough for the u32 sender_id_len prefix.
        assert!(matches!(Message::decode(&[0, 0]), Err(MessageError::Truncated { .. })));
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = sample();
        let signed = sign_message(&kp, &msg).unwrap();
        assert!(verify_message(&kp.verifying_key, &signed).is_ok());
    }

    #[test]
    fn tampered_message_fails_verification() {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = sample();
        let mut signed = sign_message(&kp, &msg).unwrap();
        signed.message.nonce += 1; // tamper after signing
        assert!(verify_message(&kp.verifying_key, &signed).is_err());
    }
}
