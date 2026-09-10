use libp2p::Multiaddr;
use libp2p::PeerId;
use libp2p::Swarm;
use libp2p::SwarmBuilder;

use crate::behaviour::{Behaviour, BehaviourEvent};
use crate::identity::Identity;

pub struct NetworkConfig {
    pub identity: Identity,
    pub listen_addrs: Vec<String>,
}

impl NetworkConfig {
    pub fn new(identity: Identity, listen_addrs: Vec<String>) -> Self {
        Self { identity, listen_addrs }
    }
}

pub struct Network {
    swarm: Swarm<Behaviour>,
}

impl Network {
    pub fn new(config: NetworkConfig) -> Result<Self, String> {
        let keypair = config.identity.keypair().clone();
        let builder = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default as fn() -> libp2p::yamux::Config,
            )
            .map_err(|e| e.to_string())?
            .with_dns()
            .map_err(|e| e.to_string())?;
        let mut swarm = builder
            .with_behaviour(Behaviour::new)
            .map_err(|e| e.to_string())?
            .build();
        for address in &config.listen_addrs {
            let multiaddr: Multiaddr = address
                .parse()
                .map_err(|e| format!("multiaddr {address}: {e}"))?;
            swarm.listen_on(multiaddr).map_err(|e| e.to_string())?;
        }
        Ok(Self { swarm })
    }

    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    pub fn listen_addrs(&self) -> Vec<Multiaddr> {
        self.swarm.listeners().cloned().collect()
    }

    pub async fn wait_for_listener(&mut self, timeout: std::time::Duration) -> bool {
        use libp2p::futures::StreamExt;
        use libp2p::swarm::SwarmEvent;
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);
        loop {
            tokio::select! {
                _ = &mut sleep => return false,
                event = self.swarm.next() => {
                    match event {
                        Some(SwarmEvent::NewListenAddr { .. }) => return true,
                        Some(_) => {}
                        None => return false,
                    }
                }
            }
        }
    }

    pub fn publish(&mut self, topic: &str, data: Vec<u8>) -> Result<(), String> {
        use libp2p::gossipsub::{IdentTopic, TopicHash};
        let topic_hash: TopicHash = IdentTopic::new(topic).hash();
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic_hash, data)
            .map_err(|e| e.to_string())
            .map(|_| ())
    }

    pub fn subscribe(&mut self, topic: &str) -> Result<(), String> {
        use libp2p::gossipsub::IdentTopic;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&IdentTopic::new(topic))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn behaviour_mut(&mut self) -> &mut Behaviour {
        self.swarm.behaviour_mut()
    }

    pub async fn next_event(
        &mut self,
        timeout: std::time::Duration,
    ) -> Option<libp2p::swarm::SwarmEvent<BehaviourEvent>> {
        use libp2p::futures::StreamExt;
        tokio::time::timeout(timeout, self.swarm.next()).await.unwrap_or_default()
    }

    pub fn add_bootstrap_peer(&mut self, peer: PeerId, address: Multiaddr) {
        self.swarm.behaviour_mut().kad.add_address(&peer, address);
    }

    pub fn bootstrap_kademlia(&mut self) {
        let _ = self.swarm.behaviour_mut().kad.bootstrap();
    }

    pub fn put_record(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), String> {
        use libp2p::kad::{Quorum, Record, RecordKey};
        let record = Record::new(RecordKey::new(&key), value);
        self.swarm
            .behaviour_mut()
            .kad
            .put_record(record, Quorum::One)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn get_record(&mut self, key: Vec<u8>) -> Result<(), String> {
        use libp2p::kad::RecordKey;
        let _ = self
            .swarm
            .behaviour_mut()
            .kad
            .get_record(RecordKey::new(&key));
        Ok(())
    }

    pub fn dial(&mut self, address: Multiaddr) -> Result<(), String> {
        self.swarm.dial(address).map_err(|e| e.to_string())
    }
}