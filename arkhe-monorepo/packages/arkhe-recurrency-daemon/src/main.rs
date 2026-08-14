//! Executable entrypoint of the ARKHE Recurrency daemon.
//!
//! Binds the [`RecurrencyGrpcService`] to `RECURRENCY_GRPC_ADDR` (default
//! `0.0.0.0:50051`). `lobo-frontal` posts frames via `ProcessStimulus`;
//! `safe-core` forces the arousal regime via `SetRegime`.

use arkhe_recurrency::{ArousalRegime, GlobalWorkspace, PerceptionLoop, RecurrencyEngine, StateDaemon};
use arkhe_recurrency_daemon::{proto, RecurrencyGrpcService};
use proto::recurrency_service_server::RecurrencyServiceServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let addr = std::env::var("RECURRENCY_GRPC_ADDR").unwrap_or_else(|_| "0.0.0.0:50051".to_string());

    let engine = RecurrencyEngine::new(
        PerceptionLoop::default(),
        StateDaemon::new(ArousalRegime::Alert),
        GlobalWorkspace::new(0.3),
    );

    let addr_parsed: std::net::SocketAddr = addr.parse()?;
    tracing::info!("arkhe-recurrency-daemon listening on {addr_parsed}");

    tonic::transport::Server::builder()
        .add_service(RecurrencyServiceServer::new(RecurrencyGrpcService::new(engine)))
        .serve(addr_parsed)
        .await?;
    Ok(())
}
