# arkhe-blossom

Blossom protocol (BUD-01) support: SHA-256-addressed blob storage and
Nostr-event-based authorization (kind `24242`), layered on
`arkhe-storage::ChunkStore` and `arkhe-nostr-anchor`.

## Status

9/9 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## Provenance

Nothing resembling a real Blossom implementation existed anywhere in this
monorepo before this crate — confirmed by a dedicated research pass before
writing any code: zero matches for "Blossom" in any code, doc, or catalog
across the whole tree. This is new work built against the BUD-01 spec's
data model, not a port of anything.

## What's actually here

- **`BlossomStore<S: ChunkStore>`** — plain SHA-256-addressed blob storage.
  Deliberately **not** built on `arkhe_storage::chk`'s convergent CHK
  encoding: Blossom blobs are addressed by the hash of their own plaintext
  bytes, no encryption layer — that's what makes them fetchable by anyone
  who knows the hash, matching BUD-01's public-blob-hosting model.
  `ChunkStore::put`'s existing address/hash validation
  (`sha256(bytes) == address`) already enforces exactly this invariant, so
  `BlossomStore` is a thin wrapper, not new storage logic — confirmed by
  `blossom_store_address_is_the_real_sha256_of_the_blob`.
- **`build_authorization` / `verify_authorization`** — real kind-`24242`
  Nostr events: real NIP-01 event ids, real BIP-340 signatures (via
  `arkhe-nostr-anchor`, not placeholders), carrying `t` (verb),
  `expiration`, and optional `x` (blob-hash scope) tags per BUD-01.
  `verify_authorization` checks, in order: signature/id validity
  (`arkhe_nostr_anchor::verify_event`), event kind, verb match, expiration,
  and — if the authorization is scoped to specific blobs via `x` tags —
  that the requested blob is actually covered. An authorization with no
  `x` tags at all is unscoped and covers any blob for its verb (BUD-01's
  model for whole-account operations like `list`), confirmed by
  `unscoped_authorization_covers_any_blob`.

Building this required a small, real addition to `arkhe-nostr-anchor`:
that crate previously only exposed identity-root event signing
(`root_identity`, GDID-specific); Blossom authorization events needed
general-purpose event signing under the same identity, so
`NostrIdentity::sign_event(created_at, kind, tags, content)` was added,
and `root_identity` refactored to be a thin wrapper around it (no
duplicated signing logic). `arkhe_nostr_anchor::verify_root_event` was
already fully generic — nothing in it assumed an identity-root event's
shape — so it's reused directly (via the `verify_event` alias, added for
readability at call sites outside the identity-root context).

## What's explicitly not here

- **No HTTP server or client.** BUD-01 is fundamentally an HTTP API
  (`GET`/`HEAD`/`PUT`/`DELETE` on a Blossom server, with the authorization
  event carried in an `Authorization` header). This crate builds the
  protocol/data-model layer that would sit under such a server or client —
  not the transport itself — matching this session's established
  precedent (`arkhe-network` doesn't open sockets either; see its README).
- **No relay-side blob discovery/mirroring** (BUD-03/BUD-04-style
  multi-server redundancy) — single-store scope only.
- **No integration with `arkhe-storage`'s CHK encryption or ML-KEM key
  encapsulation.** Blossom's addressing model (plaintext, publicly
  fetchable by hash) and CHK's (convergent-encrypted, key required to
  decrypt) are two different, deliberately separate storage models in this
  workspace — combining them (e.g. "upload CHK ciphertext to a Blossom
  server, share the key via `arkhe-storage::encapsulation`") is a real,
  plausible next step, not attempted here.
