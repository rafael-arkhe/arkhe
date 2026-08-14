//! Large (40-node) in-process consensus finality + persistent shadow storage.

use arkhe_timechain::block::TimeBlock;
use arkhe_timechain::consensus::loop_closure;
use arkhe_timechain::retro::EchoSignal;
use arkhe_timechain::{BlockHash, PersistentStore, Snapshot};

#[test]
fn forty_node_network_finalises_by_constructive_majority() {
    let node_count = 40;
    // Honest nodes all observe the same topology -> phases cluster near 0.6.
    let candidate = TimeBlock::new(
        99,
        BlockHash([0u8; 32]),
        0,
        10.0,
        10.7,
        0.6,
        1.0,
        vec![0.1, 0.04],
    );
    let echoes: Vec<EchoSignal> = (0..node_count)
        .map(|i| {
            // 5% lie — out-of-phase attack; rest honest.
            let phase = if i % 20 == 0 { std::f64::consts::PI } else { 0.61 };
            EchoSignal::new(i as u64, 99, 99, 0.0, 4.0, phase, 0.4, 0.5, vec![])
        })
        .collect();

    let f = loop_closure(&candidate, &echoes, 0.5, 0.6);
    assert!(f.block_final, "majority (38/40) constructive => final: {f:?}");
    assert_eq!(f.total_echoes, 40);
    assert_eq!(f.constructive_votes, 38);
    assert!(f.mean_interference > 0.0);
}

#[test]
fn persistent_store_roundtrip() {
    use arkhe_timechain::utxo::FluxLedger;
    let dir = std::env::temp_dir().join(format!("arkhe_store_{}", std::process::id()));
    let store = PersistentStore::open(&dir).unwrap();

    // Persist a UTXO-ledger history as a ledger-typed snapshot.
    let ledger0 = FluxLedger::new();
    let snap = Snapshot {
        height: 3,
        payload: ledger0.clone(),
        written_at: 0.25,
    };
    store.save(&snap).unwrap();

    let snap2 = Snapshot {
        height: 4,
        payload: ledger0,
        written_at: 0.5,
    };
    store.save(&snap2).unwrap();

    let heights = store.list_heights().unwrap();
    assert_eq!(heights, vec![3, 4]);

    let loaded: Snapshot<FluxLedger> = store.load(4).unwrap();
    assert_eq!(loaded.height, 4);
    assert_eq!(loaded.payload.unspent_count(), 0);
    assert!((loaded.written_at - 0.5).abs() < 1e-9);

    let _ = std::fs::remove_dir_all(&dir);
    assert!(store.list_heights().is_err(), "gone dir => error");
}