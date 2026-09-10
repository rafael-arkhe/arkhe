//! Bloco 1057 — Fechamento do mapeamento I624: cofre de chaves cripto-vinculado.
//!
//! Provas de que o `ArkheVerifier` (arkhe-blink-bridge) guarda identidades
//! Ed25519 do crate de rede `arkhe-p2p` (PeerId da malha libp2p 0.56.0) e
//! verifica, na prova de I624: (a) o PeerId apresentado é o dono registado do
//! `key_id`; (b) a assinatura do `P2PMessage` foi feita pela chave registada;
//! (c) a mensagem está dentro da janela de frescor (anti-replay).
//!
//! Precedente de honestidade: usa TIPOS reais de `arkhe-p2p` (mesmo crate cuja
//! malha foi verificada no bloco 1056) — nenhuma chave/PeerId é fabricado.

use arkhe_blink_bridge::{
    AnthActionType, AnthRiskLevel, ArkheError, ArkheVerifier, UnifiedBlinkBridge, UnifiedTransaction,
};
use arkhe_p2p::identity::Identity;
use arkhe_p2p::messages::{Payload, P2PMessage, Transaction};

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("post-epoch")
        .as_secs()
}

fn message_from(identity: &Identity, timestamp: u64) -> P2PMessage {
    P2PMessage::new(
        &identity.peer_id().to_bytes(),
        timestamp,
        Payload::Transaction(Transaction {
            tx_data: b"i624-binding".to_vec(),
            chain: "ethereum".to_string(),
        }),
    )
    .sign(identity)
}

fn unified_tx(key_id: &str) -> UnifiedTransaction {
    UnifiedTransaction::new(
        b"test_tx_data".to_vec(),
        "ethereum".to_string(),
        30_000_000_000,
        1_000_000_000,
        true,
        true,
        Some("flashbots".to_string()),
        AnthActionType::Arbitrage,
        AnthRiskLevel::Medium,
        true,
        true,
        500_000,
        key_id.to_string(),
    )
}

#[test]
fn registered_peer_is_bound_and_signed_message_verifies() {
    let mut vault = ArkheVerifier::new();
    let node = Identity::generate_ed25519();
    vault.register_key("node-a", &node);

    // (a) PeerId do dono registado.
    assert!(vault.verify_peer_binding("node-a", &node.peer_id().to_bytes()).is_ok());
    assert_eq!(vault.peer_id_bytes("node-a"), Some(node.peer_id().to_bytes()));

    // (b+c) mensagem assinada pela chave registada, dentro da janela.
    let message = message_from(&node, now());
    assert!(vault.verify_signed_message("node-a", &message, now(), 60).is_ok());
}

#[test]
fn attacker_peer_is_rejected_at_binding() {
    let mut vault = ArkheVerifier::new();
    let node = Identity::generate_ed25519();
    let attacker = Identity::generate_ed25519();
    vault.register_key("node-a", &node);

    assert!(matches!(
        vault.verify_peer_binding("node-a", &attacker.peer_id().to_bytes()),
        Err(ArkheError::KeyPeerMismatch)
    ));

    let message = message_from(&attacker, now());
    assert!(matches!(
        vault.verify_signed_message("node-a", &message, now(), 60),
        Err(ArkheError::KeyPeerMismatch)
    ));
}

#[test]
fn tampered_message_from_bound_peer_is_rejected() {
    let mut vault = ArkheVerifier::new();
    let node = Identity::generate_ed25519();
    vault.register_key("node-a", &node);

    let mut message = message_from(&node, now());
    if let Payload::Transaction(tx) = &mut message.payload {
        tx.chain = "solana".to_string();
    }

    assert!(matches!(
        vault.verify_signed_message("node-a", &message, now(), 60),
        Err(ArkheError::KeySignatureInvalid)
    ));
}

#[test]
fn stale_message_from_bound_peer_is_rejected() {
    let mut vault = ArkheVerifier::new();
    let node = Identity::generate_ed25519();
    vault.register_key("node-a", &node);

    let message = message_from(&node, now() - 3600);
    assert!(matches!(
        vault.verify_signed_message("node-a", &message, now(), 60),
        Err(ArkheError::KeyMessageStale)
    ));
}

#[test]
fn blink_transaction_key_references_registered_key() {
    // A ponte inteira (16 invariantes) aceita uma transação cujo `key_id` aponta
    // para uma identidade Ed25519 da malha registada no cofre — I624 satisfeito
    // via verify_key no fluxo process_transaction (bloco 1057).
    let mut bridge = UnifiedBlinkBridge::new();
    let node = Identity::generate_ed25519();
    bridge.register_key("node-a", &node);

    let receipt = bridge.process_transaction(unified_tx("node-a")).unwrap();
    assert!(receipt.arkhe_verified);
    assert!(receipt.privacy_guaranteed);
}

#[test]
fn blink_transaction_rejects_unregistered_key() {
    // Mesmo fluxo, key_id fora do cofre: I624 violado na camada ARKHE.
    let mut bridge = UnifiedBlinkBridge::new();
    let err = bridge.process_transaction(unified_tx("ghost")).unwrap_err();
    assert!(matches!(err, arkhe_blink_bridge::BridgeError::Arkhe(ArkheError::KeyNotFound)));
}