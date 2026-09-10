use arkhe_p2p::identity::Identity;
use arkhe_p2p::messages::{Payload, Transaction};
use arkhe_p2p::network::{Network, NetworkConfig};
use arkhe_p2p::P2PMessage;

#[tokio::test]
async fn network_builds_and_listens() {
    let config = NetworkConfig::new(Identity::generate_ed25519(), vec![
        "/ip4/127.0.0.1/tcp/0".to_string(),
    ]);
    let mut network = Network::new(config).expect("network builds");
    assert!(
        network.wait_for_listener(std::time::Duration::from_secs(10)).await,
        "listener address should appear within timeout"
    );
    assert_eq!(network.listen_addrs().len(), 1);
    assert!(!network.listen_addrs()[0].to_string().is_empty());
    assert!(network.local_peer_id().to_base58().starts_with("12D3Koo"));
}

#[tokio::test]
async fn signed_transaction_roundtrip_across_identity() {
    let alice = Identity::generate_ed25519();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("post-epoch")
        .as_secs();
    let message = P2PMessage::new(
        &alice.peer_id().to_bytes(),
        timestamp,
        Payload::Transaction(Transaction {
            tx_data: vec![9, 9, 9],
            chain: "base".to_string(),
        }),
    )
    .sign(&alice);

    assert!(message.verify(&alice.keypair().public()));
    assert!(message.is_fresh(timestamp + 1, 60));
}