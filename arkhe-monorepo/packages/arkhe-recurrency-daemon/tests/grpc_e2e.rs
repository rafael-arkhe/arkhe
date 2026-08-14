//! End-to-end smoke test: spins the tonic server on an ephemeral port and
//! round-trips real gRPC calls through the wire, using the generated client.
//!
//! This proves the HTTP/2 contract of `proto/recurrency.proto` works, not just
//! the handler in isolation.

use arkhe_recurrency::{ArousalRegime, GlobalWorkspace, PerceptionLoop, RecurrencyEngine, StateDaemon};
use arkhe_recurrency_daemon::proto::recurrency_service_client::RecurrencyServiceClient;
use arkhe_recurrency_daemon::proto::{ArousalRegime as ProtoRegime, ProcessRequest, RegimeRequest};
use arkhe_recurrency_daemon::RecurrencyGrpcService;

async fn spawn_server() -> String {
    let engine = RecurrencyEngine::new(
        PerceptionLoop::default(),
        StateDaemon::new(ArousalRegime::Alert),
        GlobalWorkspace::new(0.3),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral bind");
    let addr = listener.local_addr().expect("bound address");
    let mut server = tonic::transport::Server::builder();
    let router = server.add_service(
        arkhe_recurrency_daemon::proto::recurrency_service_server::RecurrencyServiceServer::new(
            RecurrencyGrpcService::new(engine),
        ),
    );
    tokio::spawn(async move {
        router
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .expect("server must serve");
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn process_stimulus_round_trips_over_http2() {
    let endpoint = spawn_server().await;
    let mut client = RecurrencyServiceClient::connect(endpoint)
        .await
        .expect("connect");

    let resp = client
        .process_stimulus(ProcessRequest {
            stimulus_id: "http2_s0".into(),
            embedding: (0..8).map(|i| 0.5 + (i as f64) * 0.1).collect(),
            regime: ProtoRegime::ArousalUnspecified.into(),
        })
        .await
        .expect("rpc must succeed")
        .into_inner();

    assert_eq!(resp.stimulus_id, "http2_s0");
    assert!(resp.local_loop_closed, "default loop must close over the wire");
    assert!(resp.error_reduction > 0.0);
    assert!(resp.access_granted, "Alert regime must broadcast");
    assert_eq!(resp.tick, 1, "first frame on an Alert daemon ticks to 1");
}

#[tokio::test]
async fn set_regime_forces_deep_sleep_over_http2() {
    let endpoint = spawn_server().await;
    let mut client = RecurrencyServiceClient::connect(endpoint)
        .await
        .expect("connect");

    let set = client
        .set_regime(RegimeRequest {
            regime: ProtoRegime::ArousalDeepSleep.into(),
        })
        .await
        .expect("rpc must succeed")
        .into_inner();
    assert_eq!(set.previous, ProtoRegime::ArousalAlert as i32);
    assert_eq!(set.current, ProtoRegime::ArousalDeepSleep as i32);

    // After the override, frames are still processed but never broadcast.
    let resp = client
        .process_stimulus(ProcessRequest {
            stimulus_id: "http2_s1".into(),
            embedding: (0..8).map(|i| 0.5 + (i as f64) * 0.1).collect(),
            regime: ProtoRegime::ArousalUnspecified.into(),
        })
        .await
        .expect("rpc must succeed")
        .into_inner();
    assert!(resp.local_loop_closed, "content must still be processed");
    assert!(!resp.access_granted, "DeepSleep must suppress broadcast");
}
