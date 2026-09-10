use libp2p::identity::Keypair as LpKeypair;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

pub type Peer = PeerId;

pub struct Identity {
    keypair: LpKeypair,
    peer_id: PeerId,
}

/// Debug seguro: expõe apenas o PeerId (deriva da chave PÚBLICA), nunca o par.
impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("peer_id", &self.peer_id.to_base58())
            .finish()
    }
}

impl Identity {
    pub fn generate_ed25519() -> Self {
        let keypair = LpKeypair::generate_ed25519();
        let peer_id = keypair.public().to_peer_id();
        Self { keypair, peer_id }
    }

    pub fn from_protobuf(bytes: &[u8]) -> Result<Self, String> {
        let keypair = LpKeypair::from_protobuf_encoding(bytes).map_err(|e| e.to_string())?;
        let peer_id = keypair.public().to_peer_id();
        Ok(Self { keypair, peer_id })
    }

    pub fn to_protobuf(&self) -> Result<Vec<u8>, String> {
        self.keypair.to_protobuf_encoding().map_err(|e| e.to_string())
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub fn keypair(&self) -> &LpKeypair {
        &self.keypair
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        self.keypair.sign(data).map_err(|e| e.to_string())
    }

    pub fn verify(&self, data: &[u8], signature: &[u8]) -> bool {
        self.keypair.public().verify(data, signature)
    }
}

impl Clone for Identity {
    fn clone(&self) -> Self {
        Self {
            keypair: LpKeypair::from_protobuf_encoding(
                &self.keypair.to_protobuf_encoding().expect("keypair encodes"),
            )
            .expect("keypair decodes"),
            peer_id: self.peer_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub node_type: String,
    pub capabilities: Vec<String>,
    pub addresses: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_roundtrip_protobuf() {
        let identity = Identity::generate_ed25519();
        let bytes = identity.to_protobuf().unwrap();
        let restored = Identity::from_protobuf(&bytes).unwrap();
        assert_eq!(identity.peer_id(), restored.peer_id());
    }

    #[test]
    fn peer_id_is_stable_for_keypair() {
        let identity = Identity::generate_ed25519();
        let peer_id_string = identity.peer_id().to_base58();
        assert!(peer_id_string.starts_with("12D3Koo"));
    }

    #[test]
    fn sign_verify_roundtrip() {
        let identity = Identity::generate_ed25519();
        let data = b"catedral-os";
        let signature = identity.sign(data).unwrap();
        assert!(identity.verify(data, &signature));
    }

    #[test]
    fn wrong_key_cannot_verify() {
        let alice = Identity::generate_ed25519();
        let bob = Identity::generate_ed25519();
        let signature = alice.sign(b"data").unwrap();
        assert!(!bob.verify(b"data", &signature));
    }

    #[test]
    fn tampered_data_rejected() {
        let identity = Identity::generate_ed25519();
        let signature = identity.sign(b"original").unwrap();
        assert!(!identity.verify(b"tampered", &signature));
    }
}