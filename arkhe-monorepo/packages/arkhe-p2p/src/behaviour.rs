use libp2p::gossipsub::{Behaviour as Gossipsub, MessageAuthenticity};
use libp2p::identify::{Behaviour as Identify, Config as IdentifyConfig};
use libp2p::identity::Keypair;
use libp2p::kad::{store::MemoryStore, Behaviour as KadBehaviour, Mode};
use libp2p::swarm::NetworkBehaviour;

#[derive(NetworkBehaviour)]
pub struct Behaviour {
    pub kad: KadBehaviour<MemoryStore>,
    pub gossipsub: Gossipsub,
    pub identify: Identify,
}

impl Behaviour {
    pub fn new(
        key: &Keypair,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let peer_id = key.public().to_peer_id();
        let mut kad = KadBehaviour::new(peer_id, MemoryStore::new(peer_id));
        kad.set_mode(Some(Mode::Server));
        let gossipsub_config = libp2p::gossipsub::Config::default();
        let gossipsub = Gossipsub::new(MessageAuthenticity::Signed(key.clone()), gossipsub_config)?;
        let identify_config = IdentifyConfig::new("arkhe/1.0".to_string(), key.public());
        let identify = Identify::new(identify_config);
        Ok(Self { kad, gossipsub, identify })
    }
}