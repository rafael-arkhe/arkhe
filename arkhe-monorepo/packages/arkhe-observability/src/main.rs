//! ARKHE Observatory daemon — bridges Safe-Core metrics to the Chladni Cloud
//! / Cathedral v26.5 dashboard over the Telegraph ξM-field bus.
//!
//! Reads the recurrency daemon's `WatchTickets` stream, runs a local
//! anti-hallucination policy (read-only — never issues `SetRegime`), and
//! publishes the resulting constitutional metrics onto `ws://<host>:7474`.
//! Concurrently, it answers `/cathedral/control` commands issued by the
//! dashboard (audit, attest_identity, multiplicity_check, evolve, ...).

use std::time::Duration;

use anyhow::{Context, Result};
use arkhe_observability::{Observatory, TelegraphClient};
use arkhe_recurrency_daemon::proto::recurrency_service_client::RecurrencyServiceClient;
use arkhe_recurrency_daemon::proto::WatchRequest;
use tokio::sync::mpsc;

const VIBE_THRESHOLD: f64 = 0.8;
const VIBE_WINDOW: usize = 8;
const CLAMP_COOLDOWN: u64 = 12;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let grpc_addr = env_or("RECURRENCY_GRPC_ADDR", "http://127.0.0.1:50051");
    let telegraph_url = env_or("TELEGRAPH_WS_URL", "ws://127.0.0.1:7474");

    let mut client = RecurrencyServiceClient::connect(grpc_addr.clone())
        .await
        .with_context(|| format!("connect recurrency daemon at {grpc_addr}"))?;

    tracing::info!("arkhe-observability watching {grpc_addr} → publishing to {telegraph_url}");
    let stream = client
        .watch_tickets(WatchRequest { since_tick: 0 })
        .await
        .context("watch_tickets rpc")?
        .into_inner();

    let (control_tx, mut control_rx) = mpsc::channel::<arkhe_observability::ControlCommand>(128);

    // One connection reads dashboard commands off the bus and forwards them
    // to the main loop over a channel. The main loop owns a second connection
    // purely for publishing, so the two never contend on the same stream.
    let reader_url = telegraph_url.clone();
    let reader_task = tokio::spawn(async move {
        match TelegraphClient::connect(&reader_url).await {
            Ok(mut reader) => loop {
                match reader.next_control().await {
                    Ok(cmd) => {
                        if control_tx.send(cmd).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("telegraph control read: {e}");
                        break;
                    }
                }
            },
            Err(e) => tracing::warn!("telegraph reader connect failed: {e}"),
        }
        tracing::warn!("telegraph control reader ended");
    });

    let mut publisher = TelegraphClient::connect(&telegraph_url)
        .await
        .context("telegraph publish connect")?;

    let mut observatory = Observatory::new(VIBE_THRESHOLD, VIBE_WINDOW, CLAMP_COOLDOWN);
    observatory.bind_tpm();
    tracing::info!(
        "identity tpm anchor: {}",
        if observatory.tpm_anchored() {
            "SRK sealed"
        } else {
            "unavailable (running unanchored)"
        }
    );
    let mut tickets = stream;
    let mut beacon = tokio::time::interval(Duration::from_secs(15));

    loop {
        tokio::select! {
            ticket = tickets.message() => {
                match ticket? {
                    Some(t) => {
                        let signals = observatory.observe(&t);
                        publish_all(&mut publisher, &telegraph_url, &signals).await?;
                    }
                    None => {
                        tracing::warn!("ticket stream closed; reconnecting");
                        break;
                    }
                }
            }
            _ = beacon.tick() => {
                // Keep the dashboard live even between tickets.
                let signals = observatory.beacon();
                publish_all(&mut publisher, &telegraph_url, &signals).await?;
            }
            cmd = control_rx.recv() => {
                match cmd {
                    Some(cmd) => {
                        tracing::info!("dashboard command: {}", cmd.metric);
                        let signals = observatory.control(&cmd.metric, &cmd.value);
                        publish_all(&mut publisher, &telegraph_url, &signals).await?;
                    }
                    None => {
                        tracing::warn!("control channel closed");
                        break;
                    }
                }
            }
        }
    }

    reader_task.abort();
    Ok(())
}

/// Publish signals onto the bus, transparently reconnecting on transport
/// failures (loopseal: the log must remain append-only, so we retry).
async fn publish_all(
    client: &mut TelegraphClient,
    url: &str,
    signals: &[arkhe_observability::Signal],
) -> Result<()> {
    for signal in signals {
        if let Err(e) = client.publish(signal).await {
            tracing::warn!("telegraph publish failed: {e}; reconnecting");
            let replacement = TelegraphClient::connect(url).await;
            match replacement {
                Ok(new_client) => {
                    *client = new_client;
                    client.publish(signal).await?;
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
    Ok(())
}
