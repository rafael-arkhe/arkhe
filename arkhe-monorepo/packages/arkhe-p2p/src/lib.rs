#![deny(unsafe_code)]

pub mod behaviour;
pub mod identity;
pub mod messages;
pub mod network;

pub use identity::{Identity, PeerInfo};
pub use messages::{Handshake, P2PMessage, Payload, Query, Response, Transaction};
pub use behaviour::{Behaviour, BehaviourEvent};
pub use network::{Network, NetworkConfig};