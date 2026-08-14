//! Buzz (Nostr) bridge — publishes and consumes ARKHE events over a relay.

use anyhow::{anyhow, Context, Result};
use nostr_sdk::prelude::*;
use std::time::Duration;

use crate::evidence::EvidenceBundle;
use crate::fountain_core::{HEADER_LEN, CHECKSUM_LEN};
use crate::firewall::{Zone, validate_event_firewall};

/// Event kinds used by ARKHE.
///
/// The Buzz relay (`crates/buzz-relay/src/handlers/ingest.rs` ->
/// `required_scope_for_kind`) only accepts a closed allowlist of kinds; any
/// other kind is rejected with "restricted: unknown event kind". ARKHE maps
/// onto four allowed, user-write-scoped kinds so evidence transport works on a
/// stock Buzz relay. Frames/sets are distinguished by their tags, not the kind.
pub const KIND_AFT_FRAME: u16 = 30000; // NIP-51 follow set (per-experiment frame sets)
pub const KIND_EVIDENCE_BUNDLE: u16 = 30003; // NIP-51 bookmark set (evidence bookmarks)
pub const KIND_FIREWALL_AUDIT: u16 = 30078; // NIP-51 read state (audit trail)
pub const KIND_ORCH_OR_STATE: u16 = 30315; // NIP-01 user status (OrchOR state)

pub struct BuzzBridge {
    client: Client,
    keys: Keys,
    relay_url: String,
}

impl BuzzBridge {
    pub fn new(secret_key: &str, relay_url: &str) -> Result<Self> {
        let keys = Keys::parse(secret_key)
            .map_err(|e| anyhow!("invalid secret key: {e}"))?;
        let client = Client::new(&keys);
        Ok(Self {
            client,
            keys,
            relay_url: relay_url.to_string(),
        })
    }

    pub fn public_key(&self) -> PublicKey {
        self.keys.public_key()
    }

    pub async fn connect(&self) -> Result<()> {
        self.client
            .add_relay(&self.relay_url)
            .await
            .context("failed to add relay")?;
        // Start the NIP-42 AUTH listener BEFORE connecting so it is subscribed
        // to the notification channel when the relay sends its challenge, and
        // wait (with timeout) for the auth handshake to complete. Relays that
        // don't require auth never challenge, so we treat that as success.
        let auth = self.authenticate().await?;
        self.client.connect().await;
        let _ = tokio::time::timeout(Duration::from_secs(10), auth)
            .await
            .unwrap_or(Ok(Ok(())));
        Ok(())
    }

    /// Handle NIP-42 relay authentication.
    ///
    /// nostr-sdk 0.31 does not automatically answer `AUTH` challenges, so we
    /// listen for the relay's `["AUTH", <challenge>]` message and reply with a
    /// signed kind-22242 event. This is required by relays that demand write
    /// authentication (e.g. the Buzz relay). The returned future resolves when
    /// the handshake completes (or after a timeout).
    pub async fn authenticate(&self) -> Result<tokio::sync::oneshot::Receiver<Result<()>>> {
        let client = self.client.clone();
        let keys = self.keys.clone();
        let relay_url: Url = self
            .relay_url
            .parse()
            .map_err(|e| anyhow!("invalid relay URL: {e}"))?;

        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<Result<()>>();
        let relay_url_for_task = relay_url.clone();
        let done_tx_for_task = std::sync::Arc::new(tokio::sync::Mutex::new(Some(done_tx)));
        let keys_for_task = keys.clone();

        tokio::spawn(async move {
            let client_for_handler = client.clone();
            let _ = client
                .handle_notifications(move |notification| {
                    let client = client_for_handler.clone();
                    let keys = keys_for_task.clone();
                    let relay_url = relay_url_for_task.clone();
                    let done_tx = done_tx_for_task.clone();
                    async move {
                        if let RelayPoolNotification::Message {
                            relay_url: relay,
                            message: RelayMessage::Auth { challenge },
                        } = notification
                        {
                            if relay == relay_url {
                                let builder = EventBuilder::auth(challenge, relay.clone());
                                let result = match builder.to_event(&keys) {
                                    Ok(event) => match client.relay(relay).await {
                                        Ok(relay) => relay
                                            .send_msg(
                                                ClientMessage::auth(event),
                                                RelaySendOptions::default(),
                                            )
                                            .await
                                            .map(|_| ())
                                            .map_err(|e| anyhow!("failed to send AUTH: {e}")),
                                        Err(e) => Err(anyhow!("relay lookup failed: {e}")),
                                    },
                                    Err(e) => Err(anyhow!("failed to sign AUTH: {e}")),
                                };
                                eprintln!("[auth] AUTH result: {:?}", result.is_ok());
                                if let Some(tx) = done_tx.lock().await.take() {
                                    let _ = tx.send(result);
                                }
                            }
                        }
                        Ok(false)
                    }
                })
                .await;
        });

        Ok(done_rx)
    }

    pub async fn publish_event(&self, kind: u16, content: &str, tags: Vec<Tag>) -> Result<EventId> {
        let event = EventBuilder::new(Kind::Custom(kind), content, tags)
            .to_event(&self.keys)
            .map_err(|e| anyhow!("failed to build event: {e}"))?;
        self.client
            .send_event(event)
            .await
            .map_err(|e| anyhow!("failed to send event: {e}"))
    }

    pub async fn publish_aft_frame(&self, frame: &[u8], experiment_id: &str) -> Result<EventId> {
        let content = hex::encode(frame);
        let session = extract_session_id(frame);
        let seq = extract_seq(frame);
        let tags = vec![
            // NIP-33 parameterized-replaceable kinds (30000 etc.) are keyed by
            // (pubkey, kind, d). Without a unique `d` tag the relay collapses
            // every frame into a single addressable slot; `d = <session>:<seq>`
            // keeps each fountain frame independently stored and queryable.
            Tag::custom(TagKind::from("d"), [format!("{session}:{seq}").as_str()]),
            Tag::custom(TagKind::from("experiment"), [experiment_id]),
            Tag::custom(TagKind::from("type"), ["aft_frame"]),
            Tag::custom(TagKind::from("session_id"), [session.as_str()]),
        ];
        self.publish_event(KIND_AFT_FRAME, &content, tags).await
    }

    pub async fn fetch_aft_frames(&self, experiment_id: &str) -> Result<Vec<Vec<u8>>> {
        let filter = Filter::new()
            .kind(Kind::Custom(KIND_AFT_FRAME))
            .author(self.keys.public_key())
            .limit(500);
        let events = self
            .client
            .get_events_of(vec![filter], Some(Duration::from_secs(10)))
            .await
            .map_err(|e| anyhow!("failed to fetch events: {e}"))?;
        let mut frames = Vec::new();
        for event in events {
            let is_exp = event
                .tags()
                .iter()
                .any(|t| t.as_vec().first().map(|s| s.as_str()) == Some("experiment")
                    && t.as_vec().get(1).map(|s| s.as_str()) == Some(experiment_id));
            if !is_exp {
                continue;
            }
            if let Ok(bytes) = hex::decode(event.content()) {
                frames.push(bytes);
            }
        }
        Ok(frames)
    }

    /// Convert an `EvidenceBundle` into a signed Nostr event.
    pub fn evidence_bundle_to_event(bundle: &EvidenceBundle) -> EventBuilder {
        let content = serde_json::to_string(bundle).unwrap_or_default();
        let mut tags = vec![
            // Unique NIP-33 `d` address for the bundle (parameterized-replaceable kind).
            Tag::custom(TagKind::from("d"), [bundle.id.as_str()]),
            Tag::custom(TagKind::from("hypothesis"), [bundle.hypothesis.as_str()]),
            Tag::custom(TagKind::from("baseline"), [bundle.baseline_hash.as_str()]),
            Tag::custom(
                TagKind::from("cert"),
                [format!("{:?}", bundle.certification).as_str()],
            ),
            Tag::custom(TagKind::from("translation_digest"), [bundle.digest_hex().as_str()]),
        ];
        for pump in &bundle.pump_sequence {
            tags.push(Tag::custom(TagKind::from("pump"), [pump.as_str()]));
        }
        EventBuilder::new(Kind::Custom(KIND_EVIDENCE_BUNDLE), content, tags)
    }

    /// Convert a Nostr event into an `EvidenceBundle`.
    pub fn event_to_evidence_bundle(event: &Event) -> Option<EvidenceBundle> {
        if let Ok(bundle) = serde_json::from_str::<EvidenceBundle>(event.content()) {
            return Some(bundle);
        }
        None
    }

    /// Publish an evidence bundle signed by this bridge's key.
    pub async fn publish_evidence_bundle(&self, bundle: &EvidenceBundle) -> Result<EventId> {
        let builder = Self::evidence_bundle_to_event(bundle);
        let event = builder
            .to_event(&self.keys)
            .map_err(|e| anyhow!("failed to build event: {e}"))?;
        validate_event_firewall(&event, Zone::Z1_Tools, Kind::Custom(KIND_EVIDENCE_BUNDLE))?;
        self.client
            .send_event(event)
            .await
            .map_err(|e| anyhow!("failed to send event: {e}"))
    }

    /// Fetch evidence bundles authored by this key.
    pub async fn fetch_evidence_bundles(&self) -> Result<Vec<EvidenceBundle>> {
        let filter = Filter::new()
            .kind(Kind::Custom(KIND_EVIDENCE_BUNDLE))
            .author(self.keys.public_key())
            .limit(200);
        let events = self
            .client
            .get_events_of(vec![filter], Some(Duration::from_secs(10)))
            .await
            .map_err(|e| anyhow!("failed to fetch events: {e}"))?;
        Ok(events.iter().filter_map(Self::event_to_evidence_bundle).collect())
    }
}

/// Extract the `session_id` u32 from an AFT frame (bytes 4..8).
fn extract_session_id(frame: &[u8]) -> String {
    if frame.len() >= HEADER_LEN {
        u32::from_le_bytes(frame[4..8].try_into().unwrap_or_default()).to_string()
    } else {
        "0".to_string()
    }
}

/// Extract the `seq` u32 from an AFT frame (bytes 8..12).
fn extract_seq(frame: &[u8]) -> u32 {
    if frame.len() >= HEADER_LEN {
        u32::from_le_bytes(frame[8..12].try_into().unwrap_or_default())
    } else {
        0
    }
}

/// Serialize an AFT frame so it fits in the relay payload budget.
#[allow(dead_code)]
pub fn frame_payload_len(block_size: usize) -> usize {
    HEADER_LEN + block_size + CHECKSUM_LEN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_extraction() {
        let mut frame = vec![0u8; HEADER_LEN];
        frame[4..8].copy_from_slice(&0xCAFE_BABEu32.to_le_bytes());
        assert_eq!(extract_session_id(&frame), "3405691582");
    }

    #[test]
    fn event_to_bundle_roundtrip() {
        let bundle = crate::evidence::EvidenceBundle {
            id: "eb-1".into(),
            hypothesis: "test hypothesis".into(),
            baseline_hash: "b".into(),
            pump_sequence: vec!["pump-a".into()],
            probe: "probe".into(),
            counterfactual: "cf".into(),
            observations_forward: vec!["obs".into()],
            observations_reverse: vec![],
            divergences: crate::evidence::DivergenceReport::none(0.5),
            witness: None,
            certification: crate::evidence::CertificationStatus::Pending,
            timestamp: "2026-08-02T00:00:00Z".into(),
        };
        let builder = BuzzBridge::evidence_bundle_to_event(&bundle);
        let keys = Keys::generate();
        let event = builder.to_event(&keys).unwrap();
        let back = BuzzBridge::event_to_evidence_bundle(&event).unwrap();
        assert_eq!(back, bundle);
    }
}
