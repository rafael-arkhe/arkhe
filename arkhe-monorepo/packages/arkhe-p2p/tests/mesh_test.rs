use std::time::{Duration, Instant};

use libp2p::multiaddr::Protocol;

use arkhe_p2p::behaviour::BehaviourEvent;
use arkhe_p2p::identity::Identity;
use arkhe_p2p::messages::{Payload, P2PMessage, Transaction};
use arkhe_p2p::network::{Network, NetworkConfig};

const TOPIC: &str = "/arkhe/blocks/1";
const MAX_AGE_SECS: u64 = 60;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("post-epoch")
        .as_secs()
}

fn encode_message(message: &P2PMessage) -> Vec<u8> {
    serde_json::to_vec(message).expect("message serializes")
}

fn decode_message(bytes: &[u8]) -> Option<P2PMessage> {
    serde_json::from_slice(bytes).ok()
}

async fn build_node() -> (Network, Identity) {
    let identity = Identity::generate_ed25519();
    let network = Network::new(NetworkConfig::new(identity.clone(), vec![
        "/ip4/127.0.0.1/tcp/0".to_string(),
    ]))
    .expect("network builds");
    (network, identity)
}

enum Flow {
    Continue,
    Done,
}

async fn pump_pair(
    a: &mut Network,
    b: &mut Network,
    deadline: Instant,
    on_event: impl FnMut(bool, &libp2p::swarm::SwarmEvent<BehaviourEvent>) -> Flow,
) -> Result<Flow, String> {
    let mut on_event = on_event;
    loop {
        if Instant::now() >= deadline {
            return Ok(Flow::Continue);
        }
        tokio::select! {
            event = a.next_event(Duration::from_millis(200)) => {
                let Some(event) = event else { continue };
                match on_event(true, &event) {
                    Flow::Continue => {}
                    Flow::Done => return Ok(Flow::Done),
                }
            }
            event = b.next_event(Duration::from_millis(200)) => {
                let Some(event) = event else { continue };
                match on_event(false, &event) {
                    Flow::Continue => {}
                    Flow::Done => return Ok(Flow::Done),
                }
            }
        }
    }
}

async fn connect_b_to_a(a: &mut Network, b: &mut Network, deadline: Instant) -> Result<(), String> {
    let a_peer = a.local_peer_id();
    let mut address = a.listen_addrs()[0].clone();
    address.push(Protocol::P2p(a_peer));
    b.dial(address.clone()).map_err(|e| format!("dial: {e}"))?;

    let outcome = pump_pair(a, b, deadline, |_, event| match event {
        libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. } if *peer_id == a_peer => {
            Flow::Done
        }
        _ => Flow::Continue,
    })
    .await?;
    match outcome {
        Flow::Done => Ok(()),
        _ => Err("connection to A not established in time".to_string()),
    }
}

async fn wait_for_established(
    a: &mut Network,
    b: &mut Network,
    peer: libp2p::PeerId,
    deadline: Instant,
) -> Result<(), String> {
    let outcome = pump_pair(a, b, deadline, move |_, event| match event {
        libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. } if *peer_id == peer => {
            Flow::Done
        }
        _ => Flow::Continue,
    })
    .await?;
    match outcome {
        Flow::Done => Ok(()),
        _ => Err("node did not observe the established connection in time".to_string()),
    }
}

#[tokio::test]
async fn two_nodes_dial_and_connect() {
    let (mut a, _) = build_node().await;
    assert!(
        a.wait_for_listener(Duration::from_secs(10)).await,
        "A should obtain a listener address"
    );
    let (mut b, _) = build_node().await;

    let deadline = Instant::now() + Duration::from_secs(10);
    connect_b_to_a(&mut a, &mut b, deadline)
        .await
        .expect("B connects to A over TCP + noise + yamux");

    let b_peer = b.local_peer_id();
    wait_for_established(&mut a, &mut b, b_peer, deadline)
        .await
        .expect("A observes the connection to B");
}

#[tokio::test]
async fn gossipsub_signed_publish_subscribe_with_anti_replay() {
    let (mut a, a_identity) = build_node().await;
    assert!(
        a.wait_for_listener(Duration::from_secs(10)).await,
        "A should obtain a listener address"
    );
    let (mut b, _) = build_node().await;
    a.subscribe(TOPIC).expect("A subscribes");
    b.subscribe(TOPIC).expect("B subscribes");

    let deadline = Instant::now() + Duration::from_secs(15);
    connect_b_to_a(&mut a, &mut b, deadline)
        .await
        .expect("mesh connection");

    let b_peer = b.local_peer_id();
    wait_for_established(&mut a, &mut b, b_peer, deadline)
        .await
        .expect("A observes the connection to B");

    let subscribed = pump_pair(&mut a, &mut b, deadline, |_, event| {
        let libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(
            libp2p::gossipsub::Event::Subscribed { peer_id, .. },
        )) = event
        else {
            return Flow::Continue;
        };
        if *peer_id == b_peer {
            Flow::Done
        } else {
            Flow::Continue
        }
    })
    .await
    .expect("pump runs");
    assert!(
        matches!(subscribed, Flow::Done),
        "A should learn that B subscribed to the topic"
    );

    let a_public = a_identity.keypair().public();
    let a_peer = a_identity.peer_id();

    let started = Instant::now();
    for i in 0..10u64 {
        let message = P2PMessage::new(
            &a_peer.to_bytes(),
            now(),
            Payload::Transaction(Transaction {
                tx_data: vec![i as u8, 1],
                chain: "ethereum".to_string(),
            }),
        )
        .sign(&a_identity);
        a.publish(TOPIC, encode_message(&message))
            .map_err(|e| format!("publish {i}: {e}"))
            .expect("publish succeeds");
    }
    let stale = {
        let message = P2PMessage::new(
            &a_peer.to_bytes(),
            now() - 10 * MAX_AGE_SECS,
            Payload::Transaction(Transaction {
                tx_data: vec![200],
                chain: "ethereum".to_string(),
            }),
        )
        .sign(&a_identity);
        encode_message(&message)
    };
    a.publish(TOPIC, stale.clone())
        .map_err(|e| format!("publish stale: {e}"))
        .expect("stale publish succeeds");

    let mut accepted = 0u64;
    let mut rejected_stale = 0u64;
    let mut malformed = 0u64;
    let mut first_delivery: Option<Instant> = None;
    let target = 11u64;

    let outcome = pump_pair(&mut a, &mut b, deadline, |is_local, event| {
        if is_local {
            return Flow::Continue;
        }
        let libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(
            libp2p::gossipsub::Event::Message { message, .. },
        )) = event
        else {
            return Flow::Continue;
        };
        if message.source != Some(a_peer) {
            return Flow::Continue;
        }
        let Some(parsed) = decode_message(&message.data) else {
            malformed += 1;
            return Flow::Continue;
        };
        if first_delivery.is_none() {
            first_delivery = Some(Instant::now());
        }
        match parsed.validate(&a_public, now(), MAX_AGE_SECS) {
            Ok(()) => accepted += 1,
            Err(_) => rejected_stale += 1,
        }
        if accepted + rejected_stale + malformed >= target {
            Flow::Done
        } else {
            Flow::Continue
        }
    })
    .await
    .expect("pump runs");

    assert!(matches!(outcome, Flow::Done), "B should receive all 11 messages");
    assert_eq!(accepted, 10, "all fresh messages accepted");
    assert_eq!(rejected_stale, 1, "the stale message is rejected by anti-replay");
    assert_eq!(malformed, 0, "no malformed payloads");
    assert!(
        first_delivery.is_some(),
        "delivery latency was observed"
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "propagation within the deadline was {elapsed:?}"
    );
}

#[tokio::test]
async fn kademlia_bootstrap_and_record_roundtrip() {
    let (mut a, _) = build_node().await;
    assert!(
        a.wait_for_listener(Duration::from_secs(10)).await,
        "A should obtain a listener address"
    );
    let (mut b, _) = build_node().await;
    assert!(
        b.wait_for_listener(Duration::from_secs(10)).await,
        "B should obtain a listener address"
    );

    let deadline = Instant::now() + Duration::from_secs(15);
    connect_b_to_a(&mut a, &mut b, deadline)
        .await
        .expect("mesh connection");

    let a_peer = a.local_peer_id();
    let b_peer = b.local_peer_id();
    wait_for_established(&mut a, &mut b, b_peer, deadline)
        .await
        .expect("A observes the connection to B");

    let a_addr = a.listen_addrs()[0].clone();
    let b_addr = b.listen_addrs()[0].clone();

    a.add_bootstrap_peer(b_peer, b_addr);
    b.add_bootstrap_peer(a_peer, a_addr);
    a.bootstrap_kademlia();
    b.bootstrap_kademlia();

    let mut a_bootstrapped = false;
    let mut b_bootstrapped = false;
    let outcome = pump_pair(&mut a, &mut b, deadline, |is_a, event| {
        let libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::Kad(ev)) = event else {
            return Flow::Continue;
        };
        if let libp2p::kad::Event::OutboundQueryProgressed { result, .. } = ev {
            if matches!(result, libp2p::kad::QueryResult::Bootstrap(Ok(_))) {
                if is_a {
                    a_bootstrapped = true;
                } else {
                    b_bootstrapped = true;
                }
            }
        }
        if a_bootstrapped && b_bootstrapped {
            Flow::Done
        } else {
            Flow::Continue
        }
    })
    .await
    .expect("pump runs");
    assert!(
        matches!(outcome, Flow::Done),
        "both nodes should complete Kademlia bootstrap"
    );

    let key = format!("bloco-1056-{}", b_peer.to_base58()).into_bytes();
    let value = b"catedral-os-malha-operacional".to_vec();

    b.put_record(key.clone(), value.clone()).expect("put_record starts");
    let mut stored = false;
    let outcome = pump_pair(&mut a, &mut b, deadline, |is_a, event| {
        if is_a {
            return Flow::Continue;
        }
        if let libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::Kad(
            libp2p::kad::Event::OutboundQueryProgressed {
                result: libp2p::kad::QueryResult::PutRecord(Ok(_)),
                ..
            },
        )) = event
        {
            stored = true;
        }
        if stored {
            Flow::Done
        } else {
            Flow::Continue
        }
    })
    .await
    .expect("pump runs");
    assert!(
        matches!(outcome, Flow::Done),
        "B should observe the put_record outcome"
    );

    a.get_record(key.clone()).expect("get_record starts");
    let mut found: Option<Vec<u8>> = None;
    let outcome = pump_pair(&mut a, &mut b, deadline, |is_a, event| {
        if !is_a {
            return Flow::Continue;
        }
        if let libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::Kad(
            libp2p::kad::Event::OutboundQueryProgressed {
                result: libp2p::kad::QueryResult::GetRecord(Ok(
                    libp2p::kad::GetRecordOk::FoundRecord(peer_record),
                )),
                ..
            },
        )) = event
        {
            found = Some(peer_record.record.value.clone());
        }
        if found.is_some() {
            Flow::Done
        } else {
            Flow::Continue
        }
    })
    .await
    .expect("pump runs");
    assert!(matches!(outcome, Flow::Done), "A should find the record via DHT");
    assert_eq!(found.expect("record value"), value, "record value round-trips");
}