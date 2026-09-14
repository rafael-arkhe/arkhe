//! Read-only HTTP exposure of Safe-Core metrics — the ingestion leg of the
//! Power BI pipeline (Runtime-3: always-on, cheap healthcheck included).
//!
//! The server is *strictly read-only*: it serves snapshots from
//! [`MetricsRegistry`] and can never influence the policy or the engine, the
//! same constitutional posture as `arkhe-observability`.
//!
//! # Endpoints
//!
//! | Route | Purpose |
//! |-------|---------|
//! | `GET /api/v1/policy/metrics` | aggregate summary (counters, vibe, config echo) |
//! | `GET /api/v1/policy/tickets?limit=N` | most recent N observations (default 100, max 1024) |
//! | `GET /healthz` | liveness probe |

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use serde::Serialize;

use crate::metrics::{MetricsRegistry, MetricsSummary, RECENT_CAPACITY};
use crate::telemetry::PolicyObservation;

/// Default `limit` for the recent-tickets endpoint.
const DEFAULT_TICKET_LIMIT: usize = 100;

/// Bound, running metrics server. Dropping the handle does **not** stop the
/// server task; keep it alive for the process lifetime.
pub struct MetricsServer {
    addr: SocketAddr,
    _registry: MetricsRegistry,
}

impl MetricsServer {
    /// Bind and spawn the server on the given address.
    pub async fn bind(addr: SocketAddr, registry: MetricsRegistry) -> std::io::Result<Self> {
        let state = Arc::new(registry.clone());
        let app = Router::new()
            .route("/api/v1/policy/metrics", axum::routing::get(metrics))
            .route("/api/v1/policy/tickets", axum::routing::get(tickets))
            .route("/healthz", axum::routing::get(health))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!(error = %e, "metrics server terminated");
            }
        });
        Ok(Self { addr, _registry: registry })
    }

    /// Address the server is bound to.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

async fn metrics(State(state): State<Arc<MetricsRegistry>>) -> Json<MetricsSummary> {
    Json(state.snapshot())
}

#[derive(Debug, Serialize)]
struct TicketsResponse {
    count: usize,
    tickets: Vec<PolicyObservation>,
}

async fn tickets(
    State(state): State<Arc<MetricsRegistry>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<TicketsResponse> {
    let limit = params
        .get("limit")
        .and_then(|raw| raw.parse::<usize>().ok())
        .map(|n| n.min(RECENT_CAPACITY))
        .unwrap_or(DEFAULT_TICKET_LIMIT);
    let tickets = state.recent(limit);
    Json(TicketsResponse { count: tickets.len(), tickets })
}

async fn health() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RecurrencyPolicy;
    use crate::telemetry::PolicyAction;

    fn observation(vibe: f64, action: PolicyAction, clamping: bool) -> PolicyObservation {
        PolicyObservation {
            ts: 7,
            tick: 3,
            error_reduction: 0.1,
            access_granted: true,
            suggested_gain: 2.0,
            vibe,
            action,
            clamping,
        }
    }

    async fn get(addr: SocketAddr, path: &str) -> (u16, String) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8_lossy(&buf).to_string();
        let status: u16 = raw
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let body = raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (status, body)
    }

    #[tokio::test]
    async fn serves_metrics_tickets_and_health() {
        let policy = RecurrencyPolicy::new(0.8, 8, 12);
        let registry = MetricsRegistry::new(&policy);
        registry.record(&observation(0.9, PolicyAction::Clamp, true));
        registry.record(&observation(0.2, PolicyAction::None, true));

        let server = MetricsServer::bind("127.0.0.1:0".parse().unwrap(), registry)
            .await
            .expect("bind");
        let addr = server.addr();

        let (status, body) = get(addr, "/healthz").await;
        assert_eq!(status, 200);
        assert_eq!(body, "ok");

        let (_status, body) = get(addr, "/api/v1/policy/metrics").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["tickets_total"], 2);
        assert_eq!(json["clamps_total"], 1);
        assert_eq!(json["clamping"], true);
        assert_eq!(json["threshold"], 0.8);

        let (_status, body) = get(addr, "/api/v1/policy/tickets?limit=1").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["count"], 1);
        assert_eq!(json["tickets"][0]["vibe"], 0.2);

        // Out-of-range and malformed limits fall back safely.
        let (_status, body) = get(addr, "/api/v1/policy/tickets?limit=99999").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["count"], 2);
        let (_status, body) = get(addr, "/api/v1/policy/tickets?limit=nope").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["count"], 2);
    }
}
