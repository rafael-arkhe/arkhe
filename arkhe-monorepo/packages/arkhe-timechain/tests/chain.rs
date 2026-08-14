//! End-to-end Timechain integration tests: simulate + finality + UTXO + UDP.

use arkhe_timechain::block::TimeBlock;
use arkhe_timechain::encode_echo;
use arkhe_timechain::retro::EchoSignal;
use arkhe_timechain::utxo::FluxLedger;
use arkhe_timechain::{consensus, BlockHash, simulate_network};

#[test]
fn full_network_simulates_and_echoes() {
    let (overlay, _coherent) = simulate_network(6, 7);
    assert_eq!(overlay.peers.len(), 6, "six nodes register");
    assert_eq!(overlay.echos().len(), 6, "each node shares one echo");
}

#[test]
fn loop_closure_finality_in_network() {
    let candidate = TimeBlock::new(
        3,
        BlockHash([0u8; 32]),
        1,
        1.0,
        1.4,
        0.3,
        1.0,
        vec![0.1, 0.04],
    );
    // All 5 echoed nodes agree with the candidate's phase -> majority final.
    let echoes: Vec<EchoSignal> = (0..5)
        .map(|i| EchoSignal::new(i, 3, 3, 0.0, 4.0, 0.31, 0.4, 0.5, vec![]))
        .collect();
    let res = consensus::loop_closure(&candidate, &echoes, 0.5, 0.6);
    assert!(res.block_final, "5/5 constructive majority finalises: {res:?}");
    assert_eq!(res.total_echoes, 5);
}

#[test]
fn utxo_ledger_reconnects_flux() {
    let mut ledger = FluxLedger::new();
    ledger.mint([9u8; 32], 0, 200.0, 0.40, 10);
    let txid = ledger
        .spend(&[0], &[(120.0, 0.25), (80.0, 0.15)], 11, 1e-9)
        .unwrap();
    assert_eq!(txid.len(), 32);
    assert_eq!(ledger.unspent_count(), 2);
}

#[tokio::test]
async fn udp_echo_roundtrip_loopback() {
    use arkhe_mhd::PlasmaConfig;
    use arkhe_timechain::{decode_echo, udp_recv_echo, udp_send_echo};
    use arkhe_timechain::Node;

    let config = PlasmaConfig::new(10, 8, 1.0, 0.8, 1e-3).unwrap();
    let node = Node::new(0, config, 1);
    let echo = node.emit_echo(0.0, 8.0, 0.5);

    let sa = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let sb = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let b_addr = sb.local_addr().unwrap();

    udp_send_echo(&sa, &b_addr, &echo).await.unwrap();
    let (got, _from) = udp_recv_echo(&sb).await.unwrap();

    assert_eq!(got.node_id, echo.node_id);
    assert_eq!(got.strength, echo.strength);
    assert_eq!(got.wavenumber, echo.wavenumber);
    assert_eq!(
        decode_echo(&encode_echo(&echo)).unwrap().phase,
        echo.phase
    );
}