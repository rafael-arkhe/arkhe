//! Development feed: pushes stimuli into the recurrency daemon so the
//! observability bridge (and therefore the Chladni dashboard) has live
//! tickets to render.
//!
//! ```text
//! cargo run -p arkhe-observability --example feed -- --mode sick --frames 30
//! ```
//!
//! * `healthy` — tight two-pass loops that always close (no clamp, R²→1).
//! * `sick` — feedforward-like loops that broadcast without reality-clamp
//!   (drives the Safe-Core vibe over threshold → clamp → security events).

use std::time::Duration;

use anyhow::{Context, Result};
use arkhe_recurrency_daemon::proto::recurrency_service_client::RecurrencyServiceClient;
use arkhe_recurrency_daemon::proto::{ArousalRegime as ProtoRegime, ProcessRequest};

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == format!("--{name}"))
        .map(|w| w[1].clone())
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == format!("--{name}"))
}

fn embedding(dim: usize, mode: &str, frame: u64) -> Vec<f64> {
    match mode {
        // Zero-embedding: content broadcasts with zero reality closure
        // (err_reduction -> 0), so the Safe-Core vibe hits 1 and clamps.
        "sick" => vec![0.0; dim],
        // Healthy: tight two-pass loop that always closes.
        _ => (0..dim)
            .map(|i| 0.5 + (i as f64) * 0.05 + (frame as f64 % 5.0) * 0.01)
            .collect(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mode = arg("mode").unwrap_or_else(|| "healthy".to_string());
    let frames: u64 = arg("frames").unwrap_or_else(|| "20".to_string()).parse()?;
    let interval_ms: u64 = arg("interval-ms").unwrap_or_else(|| "100".to_string()).parse()?;
    let grpc = arg("grpc").unwrap_or_else(|| "http://127.0.0.1:50051".to_string());
    let _ = flag("help");

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut client = RecurrencyServiceClient::connect(grpc.clone())
        .await
        .with_context(|| format!("connect daemon at {grpc}"))?;

    println!("feeding {frames} frames in {mode} mode...");

    for frame in 0..frames {
        let resp = client
            .process_stimulus(ProcessRequest {
                stimulus_id: format!("{mode}-{frame}"),
                embedding: embedding(8, &mode, frame),
                regime: ProtoRegime::ArousalUnspecified.into(),
            })
            .await
            .context("process_stimulus rpc")?
            .into_inner();
        println!(
            "#{frame:02} tick={} access={} err_reduction={:.3}",
            resp.tick, resp.access_granted, resp.error_reduction
        );
        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }
    Ok(())
}