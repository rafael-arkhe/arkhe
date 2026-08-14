//! End-to-end Safe-Core test: a *sick* engine (feedforward-like loop that
//! broadcasts without reality-clamp) is served over gRPC, the watchdog
//! subscribes to the ticket stream and forces `DeepSleep` when the vibe
//! exceeds the threshold, and the clamp is verified on the wire: subsequent
//! frames are still processed but never broadcast.

use arkhe_recurrency::{ArousalRegime, GlobalWorkspace, PerceptionLoop, RecurrencyEngine, StateDaemon};
use arkhe_recurrency_daemon::proto::recurrency_service_client::RecurrencyServiceClient;
use arkhe_recurrency_daemon::proto::recurrency_service_server::RecurrencyServiceServer;
use arkhe_recurrency_daemon::proto::{ArousalRegime as ProtoRegime, ProcessRequest};
use arkhe_recurrency_daemon::RecurrencyGrpcService;
use arkhe_safe_core::{run_watchdog, RecurrencyPolicy};

const N: usize = 8;

async fn spawn_sick_daemon() -> String {
    // Sick loop: corrective feedback is zero, so the two-pass loop never
    // reduces error (weak closure) while the substrate's own dynamics still
    // self-determine the state (deep depth → Alert broadcasts it).
    let engine = RecurrencyEngine::new(
        PerceptionLoop {
            feedback_gain: 0.0,
            ..Default::default()
        },
        StateDaemon::new(ArousalRegime::Alert),
        GlobalWorkspace::new(0.3),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let mut server = tonic::transport::Server::builder();
    let router = server.add_service(RecurrencyServiceServer::new(RecurrencyGrpcService::new(engine)));
    tokio::spawn(async move {
        router
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    format!("http://{addr}")
}

fn smooth_frame() -> Vec<f64> {
    (0..N).map(|i| 0.5 + (i as f64) * 0.1).collect()
}

#[tokio::test]
async fn watchdog_clamps_a_confabulating_engine() {
    let endpoint = spawn_sick_daemon().await;
    let mut client = RecurrencyServiceClient::connect(endpoint.clone())
        .await
        .expect("connect");

    // The sick engine broadcasts content with zero reality-clamp.
    let probe = client
        .process_stimulus(ProcessRequest {
            stimulus_id: "probe".into(),
            embedding: smooth_frame(),
            regime: ProtoRegime::ArousalUnspecified.into(),
        })
        .await
        .expect("rpc")
        .into_inner();
    assert!(probe.access_granted, "sick Alert engine must broadcast (the disease)");
    assert!(probe.error_reduction < 1e-6, "sick loop must not reduce error");

    // Bring the watchdog online against the same daemon.
    let mut policy = RecurrencyPolicy::new(0.8, 4, 8);
    let mut watchdog = RecurrencyServiceClient::connect(endpoint.clone()).await.expect("watchdog connect");
    let watcher = tokio::spawn(async move {
        run_watchdog(&mut watchdog, &mut policy).await.expect("watchdog must run");
    });

    // Feed a stream of confabulated frames; the vibe climbs to 1.0 and the
    // watchdog forces DeepSleep, which stops the broadcast.
    let mut clamped_seen = false;
    for i in 0..16 {
        let r = client
            .process_stimulus(ProcessRequest {
                stimulus_id: format!("confab{i}"),
                embedding: smooth_frame(),
                regime: ProtoRegime::ArousalUnspecified.into(),
            })
            .await
            .expect("rpc")
            .into_inner();
        if !r.access_granted {
            clamped_seen = true;
            // The clamp silences the broadcast but must not stop the engine:
            // the ticket is still computed and returned, only not broadcast.
            assert!(r.tick > 0, "clamped frames must still be processed");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    watcher.abort();
    assert!(clamped_seen, "the anti-hallucination clamp must fire on the wire");
}

#[tokio::test]
async fn healthy_engine_never_triggers_the_watchdog() {
    // Default loop closes tightly: healthy broadcast → vibe stays ~0.
    let engine = RecurrencyEngine::new(
        PerceptionLoop::default(),
        StateDaemon::new(ArousalRegime::Alert),
        GlobalWorkspace::new(0.3),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let mut server = tonic::transport::Server::builder();
    let router = server.add_service(RecurrencyServiceServer::new(RecurrencyGrpcService::new(engine)));
    tokio::spawn(async move {
        router
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    let endpoint = format!("http://{addr}");

    let mut client = RecurrencyServiceClient::connect(endpoint.clone()).await.unwrap();
    let mut policy = RecurrencyPolicy::new(0.8, 4, 8);
    let mut watchdog = RecurrencyServiceClient::connect(endpoint.clone()).await.unwrap();
    let watcher = tokio::spawn(async move {
        run_watchdog(&mut watchdog, &mut policy).await.unwrap();
    });

    let mut clamped = false;
    for i in 0..16 {
        let r = client
            .process_stimulus(ProcessRequest {
                stimulus_id: format!("ok{i}"),
                embedding: smooth_frame(),
                regime: ProtoRegime::ArousalUnspecified.into(),
            })
            .await
            .unwrap()
            .into_inner();
        if !r.access_granted {
            clamped = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    watcher.abort();
    assert!(!clamped, "a healthy loop must never be silenced by Safe-Core");
}
