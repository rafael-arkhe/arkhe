//! Offline verifier demo for a cryptographically signed LOCAL TEST trust domain.
//! Production must replace the reviewed root, identity, issuer and capture metadata.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sigstore_trust_root::TrustedRoot;
use sigstore_types::{Artifact, Bundle, Sha256Hash};
use sigstore_verify::{VerificationPolicy, Verifier};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use wasmtime::{Engine, Module};
pub mod production;

pub const EXPECTED_WASM_SHA256: &str = env!("ARKHE_VERIFY_WASM_SHA256");
pub const MAX_ROOT_AGE_SECONDS: u64 = 30 * 24 * 60 * 60;
const ROOT: &str = include_str!("../fixtures/trusted_root.json");
const POLICY: &str = include_str!("../fixtures/trust-policy.json");
const CHECKPOINT_ORIGIN: &str = "rekor.arkhe.test.invalid";

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("configuration: {0}")]
    Configuration(String),
    #[error("trust root capture is stale")]
    TrustRootStale,
    #[error("trust root capture is in the future")]
    TrustRootFuture,
    #[error("bundle format: {0}")]
    Bundle(String),
    #[error("invalid Merkle inclusion proof or checkpoint binding")]
    MerkleProof,
    #[error("verification: {0}")]
    Verification(String),
    #[error("unexpected warnings: {0:?}")]
    Warnings(Vec<String>),
    #[error("artifact hash differs from build-pinned hash")]
    HashMismatch,
    #[error("module compilation: {0}")]
    Compilation(String),
}

#[derive(Debug, Deserialize)]
struct TrustPolicy {
    identity: String,
    issuer: String,
    captured_at_unix: u64,
    root_sha256: String,
}

#[derive(Debug)]
pub struct SelfVerificationResult {
    pub wasm_sha256: String,
    pub integrated_time: Option<i64>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Application freshness policy; capture age is distinct from CA/key validity.
/// `now` must come from a trusted clock, and capture metadata from reviewed config.
pub fn validate_trust_root_age(captured: u64, now: u64) -> Result<(), VerifyError> {
    let age = now
        .checked_sub(captured)
        .ok_or(VerifyError::TrustRootFuture)?;
    if age > MAX_ROOT_AGE_SECONDS {
        return Err(VerifyError::TrustRootStale);
    }
    Ok(())
}

fn now_unix() -> Result<u64, VerifyError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| VerifyError::Configuration(e.to_string()))
}

// Application result policy. Unit tests construct inputs for this guard only;
// they do not claim that the cryptographic verifier produced these states.
fn enforce_report(success: bool, warnings: Vec<String>) -> Result<(), VerifyError> {
    if !success {
        return Err(VerifyError::Verification("unsuccessful result".into()));
    }
    if !warnings.is_empty() {
        return Err(VerifyError::Warnings(warnings));
    }
    Ok(())
}

fn decode_hash(s: &str) -> Result<[u8; 32], VerifyError> {
    STANDARD
        .decode(s)
        .map_err(|_| VerifyError::MerkleProof)?
        .try_into()
        .map_err(|_| VerifyError::MerkleProof)
}

/// RFC6962 audit path. Extra or missing nodes, invalid positions and roots fail.
fn check_merkle(
    leaf_body: &[u8],
    index: u64,
    size: u64,
    siblings: &[[u8; 32]],
    expected: [u8; 32],
) -> Result<(), VerifyError> {
    if size == 0 || index >= size {
        return Err(VerifyError::MerkleProof);
    }
    let hashes = siblings
        .iter()
        .map(|s| Sha256Hash::try_from_slice(s).map_err(|_| VerifyError::MerkleProof))
        .collect::<Result<Vec<_>, _>>()?;
    let root = Sha256Hash::try_from_slice(&expected).map_err(|_| VerifyError::MerkleProof)?;
    sigstore_merkle::verify_inclusion_proof(
        &sigstore_merkle::hash_leaf(leaf_body),
        index,
        size,
        &hashes,
        &root,
    )
    .map_err(|_| VerifyError::MerkleProof)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBundle {
    verification_material: RawMaterial,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMaterial {
    tlog_entries: Vec<RawEntry>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEntry {
    canonicalized_body: String,
    inclusion_proof: RawProof,
    inclusion_promise: serde_json::Value,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProof {
    log_index: String,
    tree_size: String,
    root_hash: String,
    hashes: Vec<String>,
    checkpoint: RawCheckpoint,
}
#[derive(Deserialize)]
struct RawCheckpoint {
    envelope: String,
}

// Narrow demo profile: one Rekor v1 entry with proof and SET. High-level
// verification below checks signatures, certificate, identity and artifact binding.
fn verify_inclusion(json: &[u8], expected_origin: &str) -> Result<(), VerifyError> {
    let raw: RawBundle =
        serde_json::from_slice(json).map_err(|e| VerifyError::Bundle(e.to_string()))?;
    if raw.verification_material.tlog_entries.len() != 1 {
        return Err(VerifyError::Bundle("exactly one log entry required".into()));
    }
    let e = &raw.verification_material.tlog_entries[0];
    if !e
        .inclusion_promise
        .get("signedEntryTimestamp")
        .is_some_and(|s| s.is_string())
    {
        return Err(VerifyError::Bundle("SET required".into()));
    }
    let p = &e.inclusion_proof;
    let size = p
        .tree_size
        .parse::<u64>()
        .map_err(|_| VerifyError::MerkleProof)?;
    let index = p
        .log_index
        .parse::<u64>()
        .map_err(|_| VerifyError::MerkleProof)?;
    let root = decode_hash(&p.root_hash)?;
    let mut lines = p.checkpoint.envelope.lines();
    if lines.next() != Some(expected_origin)
        || lines.next().and_then(|n| n.parse::<u64>().ok()) != Some(size)
        || lines.next().map(decode_hash).transpose()? != Some(root)
    {
        return Err(VerifyError::MerkleProof);
    }
    let body = STANDARD
        .decode(&e.canonicalized_body)
        .map_err(|_| VerifyError::MerkleProof)?;
    let hashes = p
        .hashes
        .iter()
        .map(|s| decode_hash(s))
        .collect::<Result<Vec<_>, _>>()?;
    check_merkle(&body, index, size, &hashes, root)
}

/// Success is represented only by Ok; no Ok(valid=false) authorization ambiguity.
/// Root and expected signer are embedded; callers supply only untrusted evidence.
pub fn verify_self(
    wasm_bytes: &[u8],
    bundle_json: &[u8],
) -> Result<SelfVerificationResult, VerifyError> {
    verify_self_at(wasm_bytes, bundle_json, now_unix()?)
}

fn verify_self_at(
    wasm_bytes: &[u8],
    bundle_json: &[u8],
    now: u64,
) -> Result<SelfVerificationResult, VerifyError> {
    let cfg: TrustPolicy =
        serde_json::from_str(POLICY).map_err(|e| VerifyError::Configuration(e.to_string()))?;
    validate_trust_root_age(cfg.captured_at_unix, now)?;
    let root_value: serde_json::Value =
        serde_json::from_str(ROOT).map_err(|e| VerifyError::Configuration(e.to_string()))?;
    // serde_json uses sorted object keys without preserve_order, matching generator.
    let root_canonical =
        serde_json::to_vec(&root_value).map_err(|e| VerifyError::Configuration(e.to_string()))?;
    if sha256_hex(&root_canonical) != cfg.root_sha256 {
        return Err(VerifyError::Configuration(
            "root and capture metadata disagree".into(),
        ));
    }
    verify_inclusion(bundle_json, CHECKPOINT_ORIGIN)?;
    let bundle_str =
        std::str::from_utf8(bundle_json).map_err(|e| VerifyError::Bundle(e.to_string()))?;
    let bundle = Bundle::from_json(bundle_str).map_err(|e| VerifyError::Bundle(e.to_string()))?;
    let root =
        TrustedRoot::from_json(ROOT).map_err(|e| VerifyError::Configuration(e.to_string()))?;
    let policy = VerificationPolicy {
        identity: Some(cfg.identity),
        issuer: Some(cfg.issuer),
        verify_tlog: true,
        verify_timestamp: true,
        verify_certificate: true,
        clock_skew_seconds: 60,
    };
    let result = Verifier::new(&root)
        .verify(Artifact::from(wasm_bytes), &bundle, &policy)
        .map_err(|e| VerifyError::Verification(e.to_string()))?;
    enforce_report(result.success, result.warnings)?;
    Ok(SelfVerificationResult {
        wasm_sha256: sha256_hex(wasm_bytes),
        integrated_time: result.integrated_time,
    })
}

/// Compile exactly the pinned and authenticated buffer; no file re-read between gates.
/// Instantiation/import capabilities are deliberately owned by the application.
pub fn load_verified_module(
    engine: &Engine,
    wasm_bytes: &[u8],
    bundle_json: &[u8],
) -> Result<Module, VerifyError> {
    if sha256_hex(wasm_bytes) != EXPECTED_WASM_SHA256 {
        return Err(VerifyError::HashMismatch);
    }
    verify_self(wasm_bytes, bundle_json)?;
    Module::new(engine, wasm_bytes).map_err(|e| VerifyError::Compilation(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_rejects_warning_report() {
        assert!(matches!(
            enforce_report(true, vec!["unreviewed warning".into()]),
            Err(VerifyError::Warnings(_))
        ));
    }

    #[test]
    fn application_rejects_unsuccessful_report() {
        assert!(matches!(
            enforce_report(false, vec![]),
            Err(VerifyError::Verification(_))
        ));
    }

    #[test]
    fn stale_capture_blocks_the_complete_verifier() {
        let cfg: TrustPolicy = serde_json::from_str(POLICY).unwrap();
        let result = verify_self_at(
            include_bytes!("../fixtures/arkhe_verify.wasm"),
            include_bytes!("../fixtures/arkhe_verify.wasm.bundle.json"),
            cfg.captured_at_unix + MAX_ROOT_AGE_SECONDS + 1,
        );
        assert!(matches!(result, Err(VerifyError::TrustRootStale)));
    }
}
