//! Camada ARKHE — invariantes fundamentais da Catedral OS.
//!
//! Invariantes: **I619** (BPU — Break-Point Unit), **I622** (Sharding),
//! **I623** (Energia), **I624** (Chaves).
//!
//! ## Bloco 1057 — I624 cripto-vinculado
//!
//! Desde o bloco 1057 o cofre I624 deixa de guardar **nomes** de chave e passa a
//! guardar **identidades reais** (`arkhe_p2p::identity::Identity`, Ed25519 ->
//! PeerId da malha libp2p 0.56.0, blocos 1055/1056):
//!
//! - `register_key(key_id, &Identity)` — vincula um `key_id` à chave Ed25519.
//! - `verify_peer_binding(key_id, peer_id_bytes)` — prova que o PeerId
//!   apresentado pela rede É o dono registado do `key_id` (I624).
//! - `verify_signed_message(key_id, &P2PMessage, now, max_age)` — prova que o
//!   `P2PMessage` foi assinado pela chave registada e está dentro da janela de
//!   frescor (anti-replay).
//!
//! ## Nota de honestidade
//!
//! O veredor opera sobre um estado canónico em memória (reservas disponíveis no
//! arranque). A integração com os governance crates reais pertence aos substratos
//! ancorados fora do escopo deste bloco — aqui a camada valida a **lógica** de
//! cada invariante contra o estado declarado. O cofre guarda identidades de teste
//! geradas para `master`/`backup`; a persistência/segurança do armazenamento das
//! chaves pertence a um HSM/substrato real (fora deste bloco).

use arkhe_p2p::identity::Identity;
use arkhe_p2p::messages::P2PMessage;

/// Transação avaliada pela camada ARKHE.
#[derive(Debug, Clone)]
pub struct ArkheTransaction {
    /// Payload calldata.
    pub data: Vec<u8>,
    /// Chain destino.
    pub chain: String,
    /// Requer BPU (I619).
    pub requires_bpu: bool,
    /// Requer sharding (I622).
    pub requires_sharding: bool,
    /// Estimativa de energia requerida (I623).
    pub energy_estimate: u64,
    /// Identificador da chave a usar (I624).
    pub key_id: String,
}

impl ArkheTransaction {
    /// Constrói a transação ARKHE.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        data: Vec<u8>,
        chain: String,
        requires_bpu: bool,
        requires_sharding: bool,
        energy_estimate: u64,
        key_id: String,
    ) -> Self {
        Self {
            data,
            chain,
            requires_bpu,
            requires_sharding,
            energy_estimate,
            key_id,
        }
    }
}

/// Entrada do cofre I624: um `key_id` vinculado a uma identidade Ed25519 da
/// malha (o PeerId deriva deterministicamente da chave pública).
#[derive(Debug)]
struct KeyEntry {
    key_id: String,
    identity: Identity,
}

/// Erros da camada ARKHE — cada variante mapeia uma violação de invariante.
#[derive(Debug)]
pub enum ArkheError {
    /// I619 violado: BPU indisponível.
    BpuNotAvailable,
    /// I622 violado: sharding indisponível.
    ShardingNotAvailable,
    /// I623 violado: energia insuficiente.
    InsufficientEnergy {
        /// Energia mínima requerida.
        min: u64,
        /// Energia actualmente disponível.
        actual: u64,
    },
    /// I624 violado: chave não encontrada no cofre.
    KeyNotFound,
    /// I624 violado: o PeerId apresentado não corresponde à chave registada.
    KeyPeerMismatch,
    /// I624 violado: assinatura inválida para a chave registada.
    KeySignatureInvalid,
    /// I624 violado: mensagem fora da janela de frescor (anti-replay).
    KeyMessageStale,
}

impl std::fmt::Display for ArkheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArkheError::BpuNotAvailable => write!(f, "I619 VIOLADO: BPU não disponível"),
            ArkheError::ShardingNotAvailable => write!(f, "I622 VIOLADO: sharding não disponível"),
            ArkheError::InsufficientEnergy { min, actual } => {
                write!(f, "I623 VIOLADO: energia insuficiente (mín: {min}, actual: {actual})")
            }
            ArkheError::KeyNotFound => write!(f, "I624 VIOLADO: chave não encontrada"),
            ArkheError::KeyPeerMismatch => {
                write!(f, "I624 VIOLADO: PeerId não corresponde à chave registada")
            }
            ArkheError::KeySignatureInvalid => {
                write!(f, "I624 VIOLADO: assinatura inválida para a chave registada")
            }
            ArkheError::KeyMessageStale => {
                write!(f, "I624 VIOLADO: mensagem fora da janela de frescor")
            }
        }
    }
}

impl std::error::Error for ArkheError {}

/// Verificador dos invariantes ARKHE (I619, I622, I623, I624).
#[derive(Debug)]
pub struct ArkheVerifier {
    bpu_available: bool,
    sharding_available: bool,
    energy_reserve: u64,
    keys: Vec<KeyEntry>,
}

impl ArkheVerifier {
    /// Constrói o veredor com o estado canónico: BPU/sharding disponíveis,
    /// reserva de energia de 1.000.000 e chaves `master`/`backup` vinculadas a
    /// identidades Ed25519 geradas localmente (I624 cofre cripto-vinculado).
    pub fn new() -> Self {
        Self {
            bpu_available: true,
            sharding_available: true,
            energy_reserve: 1_000_000,
            keys: vec![
                KeyEntry {
                    key_id: "master".to_string(),
                    identity: Identity::generate_ed25519(),
                },
                KeyEntry {
                    key_id: "backup".to_string(),
                    identity: Identity::generate_ed25519(),
                },
            ],
        }
    }

    /// I624 — vincula (ou substitui) um `key_id` a uma identidade Ed25519 da malha.
    pub fn register_key(&mut self, key_id: &str, identity: &Identity) {
        if let Some(entry) = self.keys.iter_mut().find(|e| e.key_id == key_id) {
            entry.identity = identity.clone();
        } else {
            self.keys.push(KeyEntry {
                key_id: key_id.to_string(),
                identity: identity.clone(),
            });
        }
    }

    /// I619 — a Break-Point Unit deve estar disponível.
    pub fn verify_bpu(&self) -> Result<(), ArkheError> {
        if !self.bpu_available {
            return Err(ArkheError::BpuNotAvailable);
        }
        Ok(())
    }

    /// I622 — o sharding deve estar disponível.
    pub fn verify_sharding(&self) -> Result<(), ArkheError> {
        if !self.sharding_available {
            return Err(ArkheError::ShardingNotAvailable);
        }
        Ok(())
    }

    /// I623 — a reserva de energia deve cobrir a estimativa requerida.
    pub fn verify_energy(&self, required: u64) -> Result<(), ArkheError> {
        if required > self.energy_reserve {
            return Err(ArkheError::InsufficientEnergy {
                min: required,
                actual: self.energy_reserve,
            });
        }
        Ok(())
    }

    /// I624 — a chave referenciada deve existir no cofre.
    pub fn verify_key(&self, key_id: &str) -> Result<(), ArkheError> {
        self.key_entry(key_id).map(|_| ())
    }

    /// I624 — bytes PeerId da identidade registada sob `key_id`.
    pub fn peer_id_bytes(&self, key_id: &str) -> Option<Vec<u8>> {
        self.key_entry(key_id)
            .ok()
            .map(|e| e.identity.peer_id().to_bytes())
    }

    /// I624 — o PeerId apresentado pela rede deve ser o dono registado do `key_id`.
    pub fn verify_peer_binding(&self, key_id: &str, peer_id_bytes: &[u8]) -> Result<(), ArkheError> {
        let entry = self.key_entry(key_id)?;
        if entry.identity.peer_id().to_bytes() != peer_id_bytes {
            return Err(ArkheError::KeyPeerMismatch);
        }
        Ok(())
    }

    /// I624 — valida um `P2PMessage` da malha: PeerId do emissor, assinatura e
    /// frescor. Prova que a mensagem foi assinada pela chave registada.
    pub fn verify_signed_message(
        &self,
        key_id: &str,
        message: &P2PMessage,
        now: u64,
        max_age_secs: u64,
    ) -> Result<(), ArkheError> {
        let entry = self.key_entry(key_id)?;
        self.verify_peer_binding(key_id, &message.peer_id)?;
        match message.validate(&entry.identity.keypair().public(), now, max_age_secs) {
            Ok(()) => Ok(()),
            Err(reason) => {
                if reason.contains("janela") {
                    Err(ArkheError::KeyMessageStale)
                } else {
                    Err(ArkheError::KeySignatureInvalid)
                }
            }
        }
    }

    /// I624 — verificação do cofre: resolve a entrada registada para `key_id`.
    fn key_entry(&self, key_id: &str) -> Result<&KeyEntry, ArkheError> {
        self.keys
            .iter()
            .find(|e| e.key_id == key_id)
            .ok_or(ArkheError::KeyNotFound)
    }

    /// Verifica todos os invariantes ARKHE condicionados pela transação.
    pub fn verify_all(&self, tx: &ArkheTransaction) -> Result<(), ArkheError> {
        if tx.requires_bpu {
            self.verify_bpu()?;
        }
        if tx.requires_sharding {
            self.verify_sharding()?;
        }
        self.verify_energy(tx.energy_estimate)?;
        self.verify_key(&tx.key_id)?;
        Ok(())
    }
}

impl Default for ArkheVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tx() -> ArkheTransaction {
        ArkheTransaction::new(
            b"data".to_vec(),
            "ethereum".to_string(),
            true,
            true,
            500_000,
            "master".to_string(),
        )
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("post-epoch")
            .as_secs()
    }

    fn signed_message(
        identity: &Identity,
        timestamp: u64,
        tamper: bool,
    ) -> P2PMessage {
        use arkhe_p2p::messages::{Payload, Transaction};
        let mut message = P2PMessage::new(
            &identity.peer_id().to_bytes(),
            timestamp,
            Payload::Transaction(Transaction {
                tx_data: vec![7, 7, 7],
                chain: "ethereum".to_string(),
            }),
        )
        .sign(identity);
        if tamper {
            if let Payload::Transaction(tx) = &mut message.payload {
                tx.chain = "solana".to_string();
            }
        }
        message
    }

    #[test]
    fn i623_insufficient_energy_rejected() {
        let v = ArkheVerifier::new();
        let mut tx = sample_tx();
        tx.energy_estimate = 5_000_000;
        assert!(matches!(
            v.verify_all(&tx),
            Err(ArkheError::InsufficientEnergy { min: 5_000_000, actual: 1_000_000 })
        ));
    }

    #[test]
    fn i624_unknown_key_rejected() {
        let v = ArkheVerifier::new();
        let mut tx = sample_tx();
        tx.key_id = "ghost".to_string();
        assert!(matches!(v.verify_all(&tx), Err(ArkheError::KeyNotFound)));
    }

    #[test]
    fn all_invariants_pass_on_header_state() {
        let v = ArkheVerifier::new();
        assert!(v.verify_all(&sample_tx()).is_ok());
    }

    #[test]
    fn i624_registered_peer_binding_matches() {
        let mut v = ArkheVerifier::new();
        let node = Identity::generate_ed25519();
        v.register_key("node-a", &node);
        assert!(v.verify_peer_binding("node-a", &node.peer_id().to_bytes()).is_ok());
        assert_eq!(v.peer_id_bytes("node-a"), Some(node.peer_id().to_bytes()));
    }

    #[test]
    fn i624_unknown_peer_binding_rejected() {
        let v = ArkheVerifier::new();
        let stranger = Identity::generate_ed25519();
        assert!(matches!(
            v.verify_peer_binding("master", &stranger.peer_id().to_bytes()),
            Err(ArkheError::KeyPeerMismatch)
        ));
    }

    #[test]
    fn i624_signed_message_from_registered_key_accepted() {
        let mut v = ArkheVerifier::new();
        let node = Identity::generate_ed25519();
        v.register_key("node-a", &node);
        let message = signed_message(&node, now(), false);
        assert!(v.verify_signed_message("node-a", &message, now(), 60).is_ok());
    }

    #[test]
    fn i624_tampered_message_rejected() {
        let mut v = ArkheVerifier::new();
        let node = Identity::generate_ed25519();
        v.register_key("node-a", &node);
        let message = signed_message(&node, now(), true);
        assert!(matches!(
            v.verify_signed_message("node-a", &message, now(), 60),
            Err(ArkheError::KeySignatureInvalid)
        ));
    }

    #[test]
    fn i624_stale_message_rejected() {
        let mut v = ArkheVerifier::new();
        let node = Identity::generate_ed25519();
        v.register_key("node-a", &node);
        let message = signed_message(&node, now() - 1000, false);
        assert!(matches!(
            v.verify_signed_message("node-a", &message, now(), 60),
            Err(ArkheError::KeyMessageStale)
        ));
    }

    #[test]
    fn i624_wrong_signer_rejected() {
        let mut v = ArkheVerifier::new();
        let node = Identity::generate_ed25519();
        let attacker = Identity::generate_ed25519();
        v.register_key("node-a", &node);
        let message = signed_message(&attacker, now(), false);
        assert!(matches!(
            v.verify_signed_message("node-a", &message, now(), 60),
            Err(ArkheError::KeyPeerMismatch)
        ));
    }
}