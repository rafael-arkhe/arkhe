use serde::{Deserialize, Serialize};

use crate::identity::Identity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handshake {
    pub public_key: Vec<u8>,
    pub node_type: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub tx_data: Vec<u8>,
    pub chain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub key: String,
    pub query_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub data: Vec<u8>,
    pub success: bool,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Payload {
    Handshake(Handshake),
    Transaction(Transaction),
    Query(Query),
    Response(Response),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PMessage {
    pub signature: Vec<u8>,
    pub peer_id: Vec<u8>,
    pub timestamp: u64,
    pub payload: Payload,
}

impl P2PMessage {
    pub fn new(peer_id: &[u8], timestamp: u64, payload: Payload) -> Self {
        Self {
            signature: Vec::new(),
            peer_id: peer_id.to_vec(),
            timestamp,
            payload,
        }
    }

    pub fn canonical(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.peer_id);
        bytes.extend_from_slice(&serde_json::to_vec(&self.payload).expect("payload serializes"));
        bytes
    }

    pub fn sign(mut self, identity: &Identity) -> Self {
        let bearer = identity;
        let canonical = self.canonical();
        self.signature = bearer.sign(&canonical).expect("signature over canonical bytes");
        self
    }

    pub fn verify(&self, public_key: &libp2p::identity::PublicKey) -> bool {
        public_key.verify(&self.canonical(), &self.signature)
    }

    pub fn is_fresh(&self, now: u64, max_age_secs: u64) -> bool {
        now >= self.timestamp && now - self.timestamp <= max_age_secs
    }

    pub fn validate(
        &self,
        public_key: &libp2p::identity::PublicKey,
        now: u64,
        max_age_secs: u64,
    ) -> Result<(), String> {
        if !self.verify(public_key) {
            return Err("assinatura invalida".to_string());
        }
        if !self.is_fresh(now, max_age_secs) {
            return Err("mensagem fora da janela temporal".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("post-epoch")
            .as_secs()
    }

    fn transaction(identity: &Identity) -> P2PMessage {
        let payload = Payload::Transaction(Transaction {
            tx_data: vec![1, 2, 3],
            chain: "ethereum".to_string(),
        });
        P2PMessage::new(&identity.peer_id().to_bytes(), now(), payload).sign(identity)
    }

    #[test]
    fn signed_message_verifies() {
        let identity = Identity::generate_ed25519();
        let message = transaction(&identity);
        assert!(message.verify(&identity.keypair().public()));
    }

    #[test]
    fn tampered_payload_rejected() {
        let identity = Identity::generate_ed25519();
        let mut message = transaction(&identity);
        match &mut message.payload {
            Payload::Transaction(tx) => tx.chain = "solana".to_string(),
            _ => unreachable!(),
        }
        assert!(!message.verify(&identity.keypair().public()));
    }

    #[test]
    fn stale_message_rejected() {
        let identity = Identity::generate_ed25519();
        let payload = Payload::Query(Query {
            key: "bloco-1055".to_string(),
            query_type: "dht_get".to_string(),
        });
        let message = P2PMessage::new(&identity.peer_id().to_bytes(), now() - 1000, payload).sign(&identity);
        assert!(!message.is_fresh(now(), 60));
    }

    #[test]
    fn future_timestamp_rejected() {
        let identity = Identity::generate_ed25519();
        let payload = Payload::Query(Query {
            key: "bloco-1055".to_string(),
            query_type: "dht_get".to_string(),
        });
        let message = P2PMessage::new(&identity.peer_id().to_bytes(), now() + 1000, payload).sign(&identity);
        assert!(!message.is_fresh(now(), 60));
    }
}