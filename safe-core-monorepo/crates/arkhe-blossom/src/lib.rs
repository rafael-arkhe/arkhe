//! Blossom protocol (BUD-01) support: SHA-256-addressed blob storage, and
//! Nostr-event-based authorization (kind `24242`) for upload/delete/list
//! operations — layered on this workspace's real `arkhe-storage::ChunkStore`
//! and `arkhe-nostr-anchor` Nostr event machinery, not a reimplementation
//! of either.
//!
//! # Provenance
//! Nothing resembling a real Blossom implementation existed anywhere in
//! this monorepo before this crate (confirmed by a dedicated research pass:
//! zero matches for "Blossom" in any code, doc, or catalog) — this is new
//! work built against the BUD-01 spec's data model, not a port of anything.
//!
//! # What's actually here
//! - [`BlossomStore`] — plain SHA-256-addressed blob storage. **Distinct
//!   from `arkhe_storage::chk`'s convergent CHK encoding**: Blossom blobs
//!   are addressed by the hash of their own plaintext bytes, with no
//!   encryption layer — that's what makes them fetchable by anyone who
//!   knows the hash, matching the public-blob-hosting model BUD-01
//!   describes. `ChunkStore::put`'s existing address/hash validation
//!   (`sha256(bytes) == address`) already enforces exactly this invariant,
//!   so `BlossomStore` is a thin wrapper, not new storage logic.
//! - [`BlossomAuthorization`] / [`build_authorization`] /
//!   [`verify_authorization`] — real kind-`24242` Nostr events (real
//!   NIP-01 ids, real BIP-340 signatures, via `arkhe-nostr-anchor`),
//!   carrying `t` (verb), `expiration`, and optional `x` (blob-hash scope)
//!   tags per BUD-01. `verify_authorization` checks signature validity,
//!   kind, verb match, expiration, and (if the authorization is scoped to
//!   specific blobs) that the requested blob is actually covered.
//!
//! # What's explicitly not here
//! - **No HTTP server or client.** BUD-01 is fundamentally an HTTP API
//!   (`GET`/`HEAD`/`PUT`/`DELETE` on a Blossom server, with the
//!   authorization event carried in an `Authorization` header). This crate
//!   builds the protocol/data-model layer that would sit under such a
//!   server or client — not the transport itself — matching this
//!   session's established precedent (`arkhe-network` doesn't open sockets
//!   either; see its README).
//! - **No relay-side blob discovery/mirroring** (BUD-03/BUD-04-style
//!   multi-server redundancy) — single-store scope only.

#![forbid(unsafe_code)]

use arkhe_nostr_anchor::{verify_event, NostrAnchorError, NostrEvent, NostrIdentity};
use arkhe_storage::{ChunkStore, Hash, StorageError};
use sha2::{Digest, Sha256};

/// BUD-01's authorization event kind.
pub const BLOSSOM_AUTH_KIND: u16 = 24242;

/// The operation a [`BlossomAuthorization`] grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlossomVerb {
    Get,
    Upload,
    List,
    Delete,
}

impl BlossomVerb {
    fn as_tag_value(self) -> &'static str {
        match self {
            BlossomVerb::Get => "get",
            BlossomVerb::Upload => "upload",
            BlossomVerb::List => "list",
            BlossomVerb::Delete => "delete",
        }
    }

    fn from_tag_value(value: &str) -> Option<Self> {
        match value {
            "get" => Some(BlossomVerb::Get),
            "upload" => Some(BlossomVerb::Upload),
            "list" => Some(BlossomVerb::List),
            "delete" => Some(BlossomVerb::Delete),
            _ => None,
        }
    }
}

/// A BUD-01 authorization: a real, signed Nostr event of kind
/// `BLOSSOM_AUTH_KIND`. Deliberately holds only the underlying
/// [`NostrEvent`] — verb/expiration/blob-scope are read back out of its
/// tags on demand ([`Self::verb`], [`Self::expiration`],
/// [`Self::blob_hashes`]) rather than duplicated into separate fields that
/// could drift from what was actually signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlossomAuthorization {
    pub event: NostrEvent,
}

/// Errors verifying a [`BlossomAuthorization`].
#[derive(Debug, thiserror::Error)]
pub enum BlossomError {
    #[error("authorization event is not valid: {0}")]
    InvalidEvent(#[from] NostrAnchorError),
    #[error("event kind {actual} is not a Blossom authorization (expected {BLOSSOM_AUTH_KIND})")]
    WrongKind { actual: u16 },
    #[error("authorization has no 't' (verb) tag")]
    MissingVerb,
    #[error("authorization is for verb {authorized:?}, not the requested {requested:?}")]
    WrongVerb { authorized: BlossomVerb, requested: BlossomVerb },
    #[error("authorization has no 'expiration' tag, or it isn't a valid timestamp")]
    MissingOrInvalidExpiration,
    #[error("authorization expired at {expiration}, now is {now}")]
    Expired { expiration: u64, now: u64 },
    #[error("authorization is scoped to specific blobs and does not cover the requested one")]
    BlobNotAuthorized,
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
}

impl BlossomAuthorization {
    fn tag_values<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> {
        self.event.tags.iter().filter(move |tag| tag.first().map(String::as_str) == Some(name)).filter_map(|tag| tag.get(1)).map(String::as_str)
    }

    pub fn verb(&self) -> Option<BlossomVerb> {
        self.tag_values("t").next().and_then(BlossomVerb::from_tag_value)
    }

    pub fn expiration(&self) -> Option<u64> {
        self.tag_values("expiration").next().and_then(|v| v.parse().ok())
    }

    /// The blob hashes this authorization is scoped to. Empty means
    /// unscoped — authorizes the operation for *any* blob (used for
    /// operations like `list` that aren't about one specific blob).
    pub fn blob_hashes(&self) -> Vec<Hash> {
        self.tag_values("x")
            .filter_map(|hex_hash| {
                let bytes = hex_decode(hex_hash)?;
                let hash: Hash = bytes.try_into().ok()?;
                Some(hash)
            })
            .collect()
    }
}

/// Builds and signs a real BUD-01 authorization event.
pub fn build_authorization(
    identity: &NostrIdentity,
    verb: BlossomVerb,
    blob_hashes: &[Hash],
    expiration: u64,
    created_at: u64,
    content: impl Into<String>,
) -> BlossomAuthorization {
    let mut tags = vec![vec!["t".to_string(), verb.as_tag_value().to_string()], vec![
        "expiration".to_string(),
        expiration.to_string(),
    ]];
    for hash in blob_hashes {
        tags.push(vec!["x".to_string(), hex_encode(hash)]);
    }
    let event = identity.sign_event(created_at, BLOSSOM_AUTH_KIND, tags, content.into());
    BlossomAuthorization { event }
}

/// Verifies `auth` authorizes `requested_verb` against `blob_hash` (pass
/// `None` for operations not scoped to one specific blob, e.g. `list`) at
/// time `now`. Checks, in order: event signature/id validity, kind, verb
/// match, expiration, and (if `auth` is scoped to specific blobs) that
/// `blob_hash` is one of them.
pub fn verify_authorization(
    auth: &BlossomAuthorization,
    requested_verb: BlossomVerb,
    blob_hash: Option<&Hash>,
    now: u64,
) -> Result<(), BlossomError> {
    verify_event(&auth.event)?;

    if auth.event.kind != BLOSSOM_AUTH_KIND {
        return Err(BlossomError::WrongKind { actual: auth.event.kind });
    }

    let authorized_verb = auth.verb().ok_or(BlossomError::MissingVerb)?;
    if authorized_verb != requested_verb {
        return Err(BlossomError::WrongVerb { authorized: authorized_verb, requested: requested_verb });
    }

    let expiration = auth.expiration().ok_or(BlossomError::MissingOrInvalidExpiration)?;
    if now >= expiration {
        return Err(BlossomError::Expired { expiration, now });
    }

    let scoped_hashes = auth.blob_hashes();
    if let (false, Some(requested)) = (scoped_hashes.is_empty(), blob_hash) {
        if !scoped_hashes.contains(requested) {
            return Err(BlossomError::BlobNotAuthorized);
        }
    }

    Ok(())
}

/// Plain SHA-256-addressed blob storage over any `arkhe_storage::ChunkStore`
/// — see the module docs for why this needs no encoding beyond what
/// `ChunkStore` already validates.
pub struct BlossomStore<S: ChunkStore> {
    store: S,
}

impl<S: ChunkStore> BlossomStore<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Stores `data`, returning its SHA-256 address.
    pub fn put_blob(&mut self, data: Vec<u8>) -> Result<Hash, StorageError> {
        let hash = sha256(&data);
        self.store.put(hash, data)?;
        Ok(hash)
    }

    pub fn get_blob(&self, hash: &Hash) -> Result<Vec<u8>, StorageError> {
        self.store.get(hash)
    }

    pub fn has_blob(&self, hash: &Hash) -> bool {
        self.store.get(hash).is_ok()
    }
}

fn sha256(data: &[u8]) -> Hash {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

fn hex_encode(hash: &Hash) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_storage::InMemoryStore;

    #[test]
    fn authorization_roundtrip_verifies() {
        let identity = NostrIdentity::generate();
        let hash = [7u8; 32];
        let auth = build_authorization(&identity, BlossomVerb::Upload, &[hash], 2_000_000_000, 1, "upload one blob");
        assert!(verify_authorization(&auth, BlossomVerb::Upload, Some(&hash), 1_000_000_000).is_ok());
    }

    #[test]
    fn wrong_verb_is_rejected() {
        let identity = NostrIdentity::generate();
        let hash = [1u8; 32];
        let auth = build_authorization(&identity, BlossomVerb::Get, &[hash], 2_000_000_000, 1, "");
        let result = verify_authorization(&auth, BlossomVerb::Delete, Some(&hash), 1_000_000_000);
        assert!(matches!(result, Err(BlossomError::WrongVerb { .. })));
    }

    #[test]
    fn expired_authorization_is_rejected() {
        let identity = NostrIdentity::generate();
        let hash = [1u8; 32];
        let auth = build_authorization(&identity, BlossomVerb::Get, &[hash], 100, 1, "");
        let result = verify_authorization(&auth, BlossomVerb::Get, Some(&hash), 200);
        assert!(matches!(result, Err(BlossomError::Expired { .. })));
    }

    #[test]
    fn authorization_scoped_to_another_blob_is_rejected() {
        let identity = NostrIdentity::generate();
        let authorized_hash = [1u8; 32];
        let requested_hash = [2u8; 32];
        let auth =
            build_authorization(&identity, BlossomVerb::Delete, &[authorized_hash], 2_000_000_000, 1, "");
        let result = verify_authorization(&auth, BlossomVerb::Delete, Some(&requested_hash), 1_000_000_000);
        assert!(matches!(result, Err(BlossomError::BlobNotAuthorized)));
    }

    #[test]
    fn unscoped_authorization_covers_any_blob() {
        let identity = NostrIdentity::generate();
        let auth = build_authorization(&identity, BlossomVerb::List, &[], 2_000_000_000, 1, "list everything");
        // No 'x' tags at all — a specific blob check still passes, since
        // BUD-01 treats an authorization with no blob scope as covering
        // any blob for that verb (used for whole-account operations).
        let hash = [9u8; 32];
        assert!(verify_authorization(&auth, BlossomVerb::List, Some(&hash), 1_000_000_000).is_ok());
        assert!(verify_authorization(&auth, BlossomVerb::List, None, 1_000_000_000).is_ok());
    }

    #[test]
    fn tampered_authorization_event_fails_signature_check() {
        let identity = NostrIdentity::generate();
        let hash = [1u8; 32];
        let mut auth = build_authorization(&identity, BlossomVerb::Upload, &[hash], 2_000_000_000, 1, "");
        auth.event.sig[0] ^= 0xFF;
        let result = verify_authorization(&auth, BlossomVerb::Upload, Some(&hash), 1_000_000_000);
        assert!(matches!(result, Err(BlossomError::InvalidEvent(_))));
    }

    #[test]
    fn blossom_store_put_get_roundtrip() {
        let mut store = BlossomStore::new(InMemoryStore::new());
        let hash = store.put_blob(b"a real blob".to_vec()).unwrap();
        assert_eq!(store.get_blob(&hash).unwrap(), b"a real blob");
        assert!(store.has_blob(&hash));
    }

    #[test]
    fn blossom_store_address_is_the_real_sha256_of_the_blob() {
        let mut store = BlossomStore::new(InMemoryStore::new());
        let data = b"content-addressed for real".to_vec();
        let hash = store.put_blob(data.clone()).unwrap();
        assert_eq!(hash, sha256(&data));
    }

    #[test]
    fn blossom_store_get_of_missing_blob_fails() {
        let store = BlossomStore::new(InMemoryStore::new());
        assert_eq!(store.get_blob(&[3u8; 32]), Err(StorageError::NotFound));
        assert!(!store.has_blob(&[3u8; 32]));
    }
}
