//! Claim a Buzz relay invite with the throwaway test key via NIP-98.
//!
//! Flow (matches buzz-relay `crates/buzz-relay/src/api/invites.rs`):
//!   1. GET  /api/join-policy                -> policy version + age requirement
//!   2. POST /api/invites/accept-policy      -> { code, policy_version, age_confirmed } -> receipt
//!   3. POST /api/invites/claim              -> { code, policy_receipt } -> joined
//!
//! Every POST is NIP-98 (kind 27235) signed: `Authorization: Nostr <base64(event_json)>`
//! with tags u=<expected_url>, method=POST, payload=<sha256_hex(body)>.
//! The NIP-98 event shape matches buzz-relay's own test helper `nip98_auth_header`.

use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use nostr_sdk::prelude::*;
use reqwest::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const HOST: &str = "arkhe.communities.buzz.xyz";
const SCHEME: &str = "https";

fn expected_url(path: &str) -> String {
    format!("{SCHEME}://{HOST}{path}")
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Build the `Authorization: Nostr <base64>` header for a NIP-98 request.
fn nip98_auth(keys: &Keys, url: &str, method: &str, body: &[u8]) -> Result<String> {
    let payload = sha256_hex(body);
    let tags = vec![
        Tag::parse(&["u", url]).context("u tag")?,
        Tag::parse(&["method", method]).context("method tag")?,
        Tag::parse(&["payload", payload.as_str()]).context("payload tag")?,
    ];
    let event = EventBuilder::new(Kind::HttpAuth, "", tags)
        .to_event(keys)
        .context("sign NIP-98 event")?;
    let event_json = serde_json::to_string(&event).context("serialize NIP-98 event")?;
    let encoded = STANDARD.encode(event_json.as_bytes());
    Ok(format!("Nostr {encoded}"))
}

async fn get_join_policy(client: &Client) -> Result<Value> {
    let url = expected_url("/api/join-policy");
    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    let status = resp.status();
    let body = resp.text().await.context("read join-policy response")?;
    anyhow::ensure!(
        status.is_success(),
        "GET /api/join-policy failed: {status}: {body}"
    );
    let value: Value = serde_json::from_str(&body).context("parse join-policy")?;
    Ok(value["policy"].clone())
}

async fn post_nip98(client: &Client, keys: &Keys, path: &str, body: &Value) -> Result<Value> {
    let url = expected_url(path);
    let body_bytes = serde_json::to_vec(body).context("serialize body")?;
    let auth = nip98_auth(keys, &url, "POST", &body_bytes)?;
    let resp = client
        .post(&url)
        .header("Authorization", &auth)
        .header("Content-Type", "application/json")
        .body(body_bytes)
        .send()
        .await
        .with_context(|| format!("POST {path}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .with_context(|| format!("read {path} response"))?;
    let value: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({ "raw": text }));
    anyhow::ensure!(status.is_success(), "POST {path} failed: {status}: {value}");
    Ok(value)
}

#[tokio::main]
async fn main() -> Result<()> {
    let sk = std::env::var("BUZZ_SECRET_KEY")
        .context("BUZZ_SECRET_KEY env var (hex or nsec) is required")?;
    let code =
        std::env::var("BUZZ_INVITE_CODE").context("BUZZ_INVITE_CODE env var is required")?;
    let keys = Keys::parse(sk.as_str()).context("parse BUZZ_SECRET_KEY")?;

    let client = Client::new();

    let policy = get_join_policy(&client).await?;
    println!("join-policy: {}", serde_json::to_string_pretty(&policy)?);
    let version = policy["version"]
        .as_str()
        .context("policy has no version")?
        .to_string();
    let age_required = policy["age_attestation_required"].as_bool().unwrap_or(false);
    println!("policy version: {version}, age_required: {age_required}");

    let mut accept_body = json!({
        "code": code,
        "policy_version": version,
    });
    if age_required {
        accept_body["age_confirmed"] = json!(true);
    }

    let accepted =
        post_nip98(&client, &keys, "/api/invites/accept-policy", &accept_body).await?;
    println!("accept-policy: {}", serde_json::to_string_pretty(&accepted)?);
    let receipt = accepted["receipt"]
        .as_str()
        .context("no receipt in accept-policy response")?
        .to_string();

    let claim_body = json!({
        "code": code,
        "policy_receipt": receipt,
    });
    let claimed = post_nip98(&client, &keys, "/api/invites/claim", &claim_body).await?;
    println!("claim: {}", serde_json::to_string_pretty(&claimed)?);

    anyhow::ensure!(
        claimed["status"] == "joined" || claimed["status"] == "already_member",
        "unexpected claim status: {}",
        claimed["status"]
    );
    println!("SUCCESS: pubkey {} is a relay member", keys.public_key());
    Ok(())
}
