//! FI-032 — a chunk-sharing protocol between agents ("FIPS P2P": the
//! request/response messages are authenticated via this workspace's
//! FIPS-standardized hybrid signatures — ML-DSA-65 is FIPS 204, ML-KEM-1024
//! is FIPS 203 — not a new, separate meaning of "FIPS"). Reuses everything
//! already built in this crate rather than inventing a new subsystem:
//! [`ChunkRequest`]/[`ChunkResponse`] travel as a [`Message`] payload,
//! authenticated via [`sign_message`]/[`verify_message`] (FI-071), and
//! [`ChunkShareHandler`] is dispatched through [`crate::handler::dispatch`]
//! (FI-077), so a panicking `ChunkStore` implementation cannot escape a
//! request handler here any more than any other handler in this crate.
//!
//! This is a message-level exchange protocol, not a live transport: there
//! is no socket/connection code here, matching this crate's existing,
//! disclosed scope (see the README). A real deployment still needs to
//! decide how `Message` bytes actually travel between two agent processes
//! (e.g. over the `tls_1_3_only_client_config` in `tls.rs`) — that wiring
//! is a distinct, deliberately out-of-scope piece of work.

use arkhe_storage::{ChunkStore, Hash};

use crate::handler::MessageHandler;

/// Requests a single chunk by its content address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkRequest {
    pub address: Hash,
}

/// Errors decoding a [`ChunkRequest`] or [`ChunkResponse`] from bytes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChunkShareError {
    #[error("chunk request/response payload is truncated")]
    Truncated,
    #[error("chunk response has an unrecognized tag byte")]
    UnrecognizedTag,
}

impl ChunkRequest {
    /// Exactly 32 bytes: the requested address.
    pub fn encode(&self) -> Vec<u8> {
        self.address.to_vec()
    }

    /// Never panics on malformed input — see
    /// `decode_never_panics_on_arbitrary_bytes` below.
    pub fn decode(bytes: &[u8]) -> Result<Self, ChunkShareError> {
        let address: Hash = bytes.try_into().map_err(|_| ChunkShareError::Truncated)?;
        Ok(Self { address })
    }
}

/// The answer to a [`ChunkRequest`]: either the ciphertext stored under
/// `address`, or an explicit not-found — never a panic or a silently
/// dropped request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkResponse {
    Found { address: Hash, ciphertext: Vec<u8> },
    NotFound { address: Hash },
}

impl ChunkResponse {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            ChunkResponse::Found { address, ciphertext } => {
                let mut buf = Vec::with_capacity(1 + 32 + ciphertext.len());
                buf.push(1u8);
                buf.extend_from_slice(address);
                buf.extend_from_slice(ciphertext);
                buf
            }
            ChunkResponse::NotFound { address } => {
                let mut buf = Vec::with_capacity(1 + 32);
                buf.push(0u8);
                buf.extend_from_slice(address);
                buf
            }
        }
    }

    /// Never panics on malformed input — see
    /// `decode_never_panics_on_arbitrary_bytes` below.
    pub fn decode(bytes: &[u8]) -> Result<Self, ChunkShareError> {
        let (&tag, rest) = bytes.split_first().ok_or(ChunkShareError::Truncated)?;
        if rest.len() < 32 {
            return Err(ChunkShareError::Truncated);
        }
        let mut address: Hash = [0u8; 32];
        address.copy_from_slice(&rest[..32]);
        match tag {
            0 => Ok(ChunkResponse::NotFound { address }),
            1 => Ok(ChunkResponse::Found { address, ciphertext: rest[32..].to_vec() }),
            _ => Err(ChunkShareError::UnrecognizedTag),
        }
    }
}

/// Serves [`ChunkRequest`]s out of a `ChunkStore`. Dispatched via
/// [`crate::handler::dispatch`] like any other [`MessageHandler`] — a
/// `ChunkStore::get` implementation that panics is contained the same way
/// as any other handler-side panic (FI-077).
pub struct ChunkShareHandler<'a, S: ChunkStore> {
    store: &'a S,
}

impl<'a, S: ChunkStore> ChunkShareHandler<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store }
    }
}

impl<'a, S: ChunkStore> MessageHandler for ChunkShareHandler<'a, S> {
    type Response = ChunkResponse;
    type Error = ChunkShareError;

    fn handle(&self, msg: &crate::message::SignedMessage) -> Result<Self::Response, Self::Error> {
        let request = ChunkRequest::decode(&msg.message.payload)?;
        match self.store.get(&request.address) {
            Ok(ciphertext) => Ok(ChunkResponse::Found { address: request.address, ciphertext }),
            Err(_) => Ok(ChunkResponse::NotFound { address: request.address }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::dispatch;
    use crate::message::sign_message;
    use arkhe_crypto_pqc::generate_hybrid_keypair;
    use arkhe_storage::{chk_encode, InMemoryStore};

    #[test]
    fn request_encode_decode_roundtrip() {
        let req = ChunkRequest { address: [7u8; 32] };
        assert_eq!(ChunkRequest::decode(&req.encode()).unwrap(), req);
    }

    #[test]
    fn response_found_encode_decode_roundtrip() {
        let resp = ChunkResponse::Found { address: [1u8; 32], ciphertext: b"hello chunk".to_vec() };
        assert_eq!(ChunkResponse::decode(&resp.encode()).unwrap(), resp);
    }

    #[test]
    fn response_not_found_encode_decode_roundtrip() {
        let resp = ChunkResponse::NotFound { address: [2u8; 32] };
        assert_eq!(ChunkResponse::decode(&resp.encode()).unwrap(), resp);
    }

    #[test]
    fn decode_never_panics_on_arbitrary_bytes() {
        let cases: Vec<Vec<u8>> = vec![
            vec![],
            vec![0xFF],
            vec![5u8; 10],
            vec![0u8; 33],
            vec![1u8; 32],
            (0..300u32).map(|i| (i % 256) as u8).collect(),
        ];
        for case in &cases {
            let r1 = std::panic::catch_unwind(|| ChunkRequest::decode(case));
            assert!(r1.is_ok(), "ChunkRequest::decode panicked on {case:?}");
            let r2 = std::panic::catch_unwind(|| ChunkResponse::decode(case));
            assert!(r2.is_ok(), "ChunkResponse::decode panicked on {case:?}");
        }
    }

    fn signed_request_message(address: Hash) -> crate::message::SignedMessage {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = crate::message::Message {
            sender_id: "agent-a".to_string(),
            nonce: 1,
            payload: ChunkRequest { address }.encode(),
        };
        sign_message(&kp, &msg).unwrap()
    }

    #[test]
    fn handler_serves_a_stored_chunk() {
        let mut store = InMemoryStore::new();
        let block = chk_encode(b"a real stored chunk");
        store.put(block.address, block.ciphertext.clone()).unwrap();

        let handler = ChunkShareHandler::new(&store);
        let signed = signed_request_message(block.address);
        let response = dispatch(&handler, &signed).unwrap();

        assert_eq!(response, ChunkResponse::Found { address: block.address, ciphertext: block.ciphertext });
    }

    #[test]
    fn handler_reports_not_found_for_a_missing_chunk() {
        let store = InMemoryStore::new();
        let handler = ChunkShareHandler::new(&store);
        let missing = [9u8; 32];
        let signed = signed_request_message(missing);
        let response = dispatch(&handler, &signed).unwrap();
        assert_eq!(response, ChunkResponse::NotFound { address: missing });
    }

    struct PanickingStore;
    impl ChunkStore for PanickingStore {
        fn put(&mut self, _address: Hash, _ciphertext: Vec<u8>) -> Result<(), arkhe_storage::StorageError> {
            unreachable!()
        }
        fn get(&self, _address: &Hash) -> Result<Vec<u8>, arkhe_storage::StorageError> {
            panic!("store backend is on fire")
        }
        fn len(&self) -> usize {
            0
        }
    }

    #[test]
    fn a_panicking_store_does_not_abort_dispatch() {
        let store = PanickingStore;
        let handler = ChunkShareHandler::new(&store);
        let signed = signed_request_message([0u8; 32]);
        let result = std::panic::catch_unwind(|| dispatch(&handler, &signed));
        assert!(result.is_ok(), "dispatch() itself must not panic even when the handler's store does");
        assert!(matches!(result.unwrap(), Err(crate::handler::DispatchError::Panic(_))));
    }
}
