//! FI-071/FI-075/FI-077/FI-078 — a minimal, scoped slice of networking
//! invariants. **Not** a full P2P/transport stack: no socket handling, no
//! connection lifecycle, no peer discovery. See the README for exactly
//! what is and isn't here and why.

#![deny(unsafe_code)]

pub mod handler;
pub mod message;
pub mod nonce;
pub mod tls;

pub use handler::{dispatch, DispatchError, MessageHandler};
pub use message::{sign_message, verify_message, Message, MessageError, SignedMessage};
pub use nonce::MessageNonceTracker;
pub use tls::tls_1_3_only_client_config;
