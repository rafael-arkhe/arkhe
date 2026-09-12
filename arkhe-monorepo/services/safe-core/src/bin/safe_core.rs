//! Safe-Core watchdog — the executable that holds the collar of the
//! StateDaemon over the wire.
//!
//! Subscribes to the daemon's `WatchTickets` stream, feeds each ticket to the
//! [`RecurrencyPolicy`], and turns its demands into `SetRegime` calls (see
//! [`arkhe_safe_core::run_watchdog`]).
//!
//! Every decision is also fanned out to two read-only observability legs:
//!
//! * a JSONL log ([`arkhe_safe_core::TelemetrySink`]) for batch ingestion;
//! * an HTTP metrics server ([`arkhe_safe_core::MetricsServer`]) exposing
//!   `/api/v1/policy/metrics`, `/api/v1/policy/tickets` and `/healthz`
//!   for Power BI / live dashboards.
//!
//! # Configuration (environment)
//!
//! | Variable | Default | Purpose |
//! |----------|---------|---------|
//! | `RECURRENCY_GRPC_ADDR` | `http://127.0.0.1:50051` | recurrency daemon endpoint |
//! | `SAFE_CORE_TELEMETRY_PATH` | `safe-core-telemetry.jsonl` | JSONL append-only log |
//! | `SAFE_CORE_METRICS_ADDR` | `127.0.0.1:8080` | metrics HTTP listener |

use std::net::SocketAddr;

use arkhe_recurrency_daemon::proto::recurrency_service_client::RecurrencyServiceClient;
use arkhe_safe_core::{
    run_watchdog_with_recorder, Fanout, MetricsRegistry, MetricsServer, RecurrencyPolicy,
    TelemetrySink,
};

const VIBE_THRESHOLD: f64 = 0.8;
const VIBE_WINDOW: usize = 8;
const CLAMP_COOLDOWN: u64 = 12;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let addr = std::env::var("RECURRENCY_GRPC_ADDR").unwrap_or_else(|_| "http://127.0.0.1:50051".to_string());
    let telemetry_path =
        std::env::var("SAFE_CORE_TELEMETRY_PATH").unwrap_or_else(|_| "safe-core-telemetry.jsonl".to_string());
    let metrics_addr: SocketAddr = std::env::var("SAFE_CORE_METRICS_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
        .parse()?;

    let mut client = RecurrencyServiceClient::connect(addr.clone()).await?;
    tracing::info!("safe-core watching {addr}");

    let policy = RecurrencyPolicy::new(VIBE_THRESHOLD, VIBE_WINDOW, CLAMP_COOLDOWN);
    let registry = MetricsRegistry::new(&policy);

    let metrics_server = MetricsServer::bind(metrics_addr, registry.clone()).await?;
    tracing::info!("safe-core metrics on http://{}/api/v1/policy/metrics", metrics_server.addr());

    let telemetry_path_display = telemetry_path.clone();
    let mut recorder = Fanout(TelemetrySink::open(telemetry_path), registry);
    tracing::info!(path = %telemetry_path_display, "safe-core telemetry JSONL active");

    let mut policy = policy;
    run_watchdog_with_recorder(&mut client, &mut policy, &mut recorder).await
}
