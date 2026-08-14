//! The P2P overlay and its UDP echo transport.
//!
//! Each node runs its **own** EvoField and shares an echo with its peers. The
//! [`Overlay`] models the peer graph; the UDP helpers exchange real
//! `bincode`-encoded echoes over the wire.

use arkhe_mhd::{helicity, EvoField, PlasmaConfig};
use serde::{Deserialize, Serialize};

use crate::retro::{Dispersion, EchoSignal};

/// A peer identity — the network address of a node (a UDP `SocketAddr` fits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(pub u64);

/// A network participant owning its own private EvoField.
pub struct Node {
    pub id: PeerId,
    pub field: EvoField,
    pub height: u64,
}

impl Node {
    /// Create a node with its own seeded phase field.
    pub fn new(id: u64, config: PlasmaConfig, seed: u64) -> Self {
        Self {
            id: PeerId(id),
            field: EvoField::with_noise(config, seed),
            height: 0,
        }
    }

    /// Evolve the node's field by `steps` explicit-Euler time steps.
    pub fn evolve_steps(&mut self, steps: usize, dt: f64) {
        for _ in 0..steps {
            self.field.advance(dt);
        }
        self.height += steps as u64;
    }

    /// The node's genuine topology invariant (the Chern–Simons helicity).
    pub fn helicity(&self, iterations: usize) -> f64 {
        helicity(
            (&self.field.omega_x, &self.field.omega_y, &self.field.omega_z),
            self.field.config.dx(),
            iterations,
        )
    }

    /// Emit a synthetic echo of the node's current topology.
    ///
    /// The echo phase is the wrapped helicity phase; the pattern is a trailing
    /// spectrum carried along the dispersion channel.
    pub fn emit_echo(&self, emitted_at: f64, wavenumber: f64, strength: f64) -> EchoSignal {
        let h = self.helicity(12);
        let phase = h.rem_euclid(std::f64::consts::TAU);
        EchoSignal::new(
            self.id.0,
            self.height,
            self.height,
            emitted_at,
            wavenumber,
            phase,
            h,
            strength,
            vec![strength, strength * 0.5, strength * 0.25],
        )
    }
}

/// An in-process peer topology that relays echoes between nodes.
pub struct Overlay {
    /// The shared dispersion relation.
    pub dispersion: Dispersion,
    /// Registered node identities.
    pub peers: Vec<PeerId>,
    /// Echoes received so far.
    echoes: Vec<EchoSignal>,
}

impl Overlay {
    /// An empty overlay bound to a `dispersion` relation.
    pub fn new(dispersion: Dispersion) -> Self {
        Self {
            dispersion,
            peers: Vec::new(),
            echoes: Vec::new(),
        }
    }

    /// Register a peer.
    pub fn register(&mut self, id: PeerId) {
        if !self.peers.contains(&id) {
            self.peers.push(id);
        }
    }

    /// Relay (gossip) one echo into the network store.
    pub fn relay(&mut self, echo: EchoSignal) {
        self.echoes.push(echo);
    }

    /// Everything received so far.
    pub fn echos(&self) -> &[EchoSignal] {
        &self.echoes
    }

    /// The subset of peers whose echo phase agrees with the network mean.
    pub fn coherent_peers(&self) -> Vec<PeerId> {
        if self.echoes.is_empty() {
            return Vec::new();
        }
        let mean = self.echoes.iter().map(|e| e.phase).sum::<f64>() / self.echoes.len() as f64;
        self.echoes
            .iter()
            .filter(|e| (e.phase - mean).cos() > 0.0)
            .map(|e| PeerId(e.node_id))
            .collect()
    }
}

/// Encode an echo for the wire (bincode).
pub fn encode_echo(echo: &EchoSignal) -> Vec<u8> {
    bincode::serialize(echo).expect("an EchoSignal must be serialisable")
}

/// Decode a wire echo.
pub fn decode_echo(bytes: &[u8]) -> Result<EchoSignal, bincode::Error> {
    bincode::deserialize(bytes)
}

/// Send a single encoded echo to a peer over UDP (best-effort).
pub async fn udp_send_echo(
    socket: &tokio::net::UdpSocket,
    peer: &std::net::SocketAddr,
    echo: &EchoSignal,
) -> std::io::Result<usize> {
    socket.send_to(&encode_echo(echo), *peer).await
}

/// Receive a single echo (and its origin) over UDP.
pub async fn udp_recv_echo(
    socket: &tokio::net::UdpSocket,
) -> std::io::Result<(EchoSignal, std::net::SocketAddr)> {
    let mut buf = [0u8; 4096];
    let (n, from) = socket.recv_from(&mut buf).await?;
    bincode::deserialize(&buf[..n])
        .map(|e| (e, from))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_echoes_form_a_coherent_network() {
        let mut overlay = Overlay::new(Dispersion::new(2.0, 4.0));
        let config = PlasmaConfig::new(10, 8, 1.0, 0.8, 1e-3).unwrap();
        let mut nodes: Vec<Node> = (0..5).map(|i| Node::new(i, config.clone(), i)).collect();
        for n in nodes.iter_mut() {
            n.evolve_steps(3, n.field.config.max_stable_dt() * 0.9);
            overlay.register(n.id);
        }
        for n in &nodes {
            overlay.relay(n.emit_echo(0.0, 8.0, 0.5));
        }
        assert_eq!(overlay.echos().len(), 5);
        assert_eq!(overlay.peers.len(), 5);
        assert!(overlay.coherent_peers().len() <= overlay.peers.len());
    }

    #[test]
    fn coherent_majority_reported() {
        // Deterministic phases: 4 share ~0, 1 is anti-phase.
        let mut overlay = Overlay::new(Dispersion::new(2.0, 4.0));
        for (id, phase) in [(1u64, 0.1), (2, 0.12), (3, 0.09), (4, 0.11), (5, std::f64::consts::PI)] {
            let e = EchoSignal::new(id, 1, 1, 0.0, 4.0, phase, 0.2, 0.5, vec![]);
            overlay.register(PeerId(id));
            overlay.relay(e);
        }
        let coherent = overlay.coherent_peers();
        assert_eq!(coherent.len(), 4);
    }
}