#![deny(unsafe_code)]

//! ARKHE **Timechain** — closing the loop around the MHD phase-field and the
//! Shadow with a real P2P overlay.
//!
//! Each node owns a private [`arkhe_mhd::EvoField`]. Nodes echo a compressed
//! prediction of their topology along the `v_eco(k)` dispersion relation
//! ([`retro`]); a block becomes final when a majority of those echoes interfere
//! constructively ([`consensus`] — "loop closure"); balances live as
//! flux-tube UTXOs whose spends are magnetic reconnections ([`utxo`]); and the
//! hottest primitives (helicity + SVD) are behind a hardware-acceleration trait
//! ([`accel`]).
//!
//! Everything is pure Rust (no BLAS/LAPACK) so it builds on Windows.

pub mod accel;
pub mod block;
pub mod consensus;
pub mod net;
pub mod retro;
pub mod storage;
pub mod utxo;
pub mod quantum;

pub use block::{hash_f64s, BlockHash, TimeBlock};
pub use net::{
    decode_echo, encode_echo, udp_recv_echo, udp_send_echo, Node, Overlay, PeerId,
};
pub use retro::{Dispersion, EchoSignal};
pub use storage::{PersistentStore, Snapshot, StorageError};
pub use utxo::{FluxLedger, Utxo, UtxoError};
pub use quantum::{quantize_block, quantum_verify_block, SealedBlock};

/// A one-stop entry: spin `n` nodes, exchange their echoes in the overlay, and
/// report the coherent subset (used by the demo and tests).
pub fn simulate_network(n: usize, seed_base: u64) -> (Overlay, Vec<f64>) {
    use arkhe_mhd::PlasmaConfig;
    let dispersion = Dispersion::new(2.0, 4.0);
    let mut overlay = Overlay::new(dispersion);
    let config = PlasmaConfig::new(10, 8, 1.0, 0.8, 1e-3)
        .expect("network field grid is large enough");
    let mut nodes: Vec<Node> = Vec::with_capacity(n);
    for i in 0..n {
        let mut node = Node::new(i as u64, config.clone(), seed_base + i as u64);
        node.evolve_steps(3, node.field.config.max_stable_dt() * 0.9);
        overlay.register(node.id);
        nodes.push(node);
    }
    for node in &nodes {
        overlay.relay(node.emit_echo(0.0, 8.0, 0.5));
    }
    let coherent = overlay.coherent_peers();
    (overlay, coherent.iter().map(|p| p.0 as f64).collect())
}

pub const VERSION: &str = "0.1.0-ARKHE-TIMECHAIN-2026-08-02";
