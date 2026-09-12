//! Minimal WebSocket client for the Telegraph ξM-field bus.
//!
//! Speaks the wire contract of `telegraph.js`:
//!
//! ```json
//! {"action":"subscribe","topic":"/cathedral/control"}
//! {"action":"publish","signal":{...}}
//! ```
//!
//! The bus answers with `{"type":"signal","data":{...}}` frames for every
//! published signal on a subscribed topic, and `{"type":"status",...}` /
//! `{"type":"subscribed",...}` acknowledgements.

use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::signal::{topic, Signal, SOURCE};

/// A frame received from the bus.
#[derive(Debug, Deserialize)]
pub struct BusFrame {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub data: Option<Value>,
    #[serde(default)]
    pub topic: Option<String>,
}

/// A control command the dashboard issued on `/cathedral/control`.
#[derive(Debug, Clone)]
pub struct ControlCommand {
    pub metric: String,
    pub value: Value,
}

/// Streaming connection to the Telegraph bus.
pub struct TelegraphClient {
    stream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    /// Topics currently subscribed (for resubscribe after a reconnect).
    subscriptions: Vec<String>,
}

impl TelegraphClient {
    /// Connect to the bus and subscribe to the dashboard topics.
    pub async fn connect(url: &str) -> Result<Self> {
        let (stream, _) = connect_async(url)
            .await
            .with_context(|| format!("telegraph connect {url}"))?;
        let mut client = Self {
            stream,
            subscriptions: vec![
                topic::CONTROL.to_string(),
                topic::STATS.to_string(),
                topic::EVENTS.to_string(),
                topic::SIGNAL_PHI.to_string(),
            ],
        };
        client.resubscribe().await?;
        Ok(client)
    }

    /// Re-issue all known subscriptions (used after a reconnect).
    async fn resubscribe(&mut self) -> Result<()> {
        let pending: Vec<String> = self.subscriptions.clone();
        for topic in pending {
            self.subscribe(&topic).await?;
        }
        Ok(())
    }

    /// Subscribe to a topic.
    pub async fn subscribe(&mut self, topic: &str) -> Result<()> {
        self.send_text(json!({ "action": "subscribe", "topic": topic }).to_string())
            .await
    }

    /// Publish a signal onto the bus.
    pub async fn publish(&mut self, signal: &Signal) -> Result<()> {
        let frame = json!({ "action": "publish", "signal": signal }).to_string();
        self.send_text(frame).await
    }

    /// Read the next frame from the bus.
    pub async fn next_frame(&mut self) -> Result<Option<BusFrame>> {
        loop {
            match self.stream.next().await {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(frame) = serde_json::from_str::<BusFrame>(&text) {
                        return Ok(Some(frame));
                    }
                    // Non-signal control frames are ignored.
                }
                Some(Ok(_)) => continue,
                Some(Err(e)) => return Err(anyhow::anyhow!("telegraph read: {e}")),
                None => return Ok(None),
            }
        }
    }

    /// Wait for the next control command issued by the dashboard.
    ///
    /// The dashboard publishes `{metric, value}` objects on
    /// `/cathedral/control`; anything else (e.g. `status` acknowledgements or
    /// the bridge's own `*_response` echoes) is filtered out so the bridge
    /// never responds to itself.
    pub async fn next_control(&mut self) -> Result<ControlCommand> {
        loop {
            match self.next_frame().await? {
                Some(frame) if frame.kind == "signal" => {
                    if let Some(data) = frame.data {
                        let source = data.get("source").and_then(Value::as_str);
                        let is_self = source == Some(SOURCE);
                        if !is_self && data.get("topic").and_then(Value::as_str) == Some(topic::CONTROL) {
                            let metric = data
                                .get("metric")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown")
                                .to_string();
                            let value = data.get("value").cloned().unwrap_or(Value::Null);
                            return Ok(ControlCommand { metric, value });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    async fn send_text(&mut self, text: String) -> Result<()> {
        self.stream
            .send(Message::Text(text.into()))
            .await
            .context("telegraph send")
    }

    /// Keep the connection alive against short bus outages by reconnecting.
    ///
    /// Returns only once a reconnection has succeeded, or propagates a fatal
    /// error.
    pub async fn reconnect_loop(&mut self, url: &str) -> Result<()> {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            tracing::warn!("telegraph reconnect attempt");
            match Self::connect(url).await {
                Ok(replacement) => {
                    *self = replacement;
                    return Ok(());
                }
                Err(e) => tracing::warn!("telegraph reconnect failed: {e}"),
            }
        }
    }
}
