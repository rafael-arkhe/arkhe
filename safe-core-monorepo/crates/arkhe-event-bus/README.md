# `arkhe-event-bus`

Typed publish/subscribe for the Arkhe Trust Infrastructure — **Phase 1: in
memory**.

This crate implements the `arkhe-event-bus` row of Table 1 of the canonical
Arkhe paper, *"Arkhe: A Verifiable Trust Infrastructure and Engineering Roadmap
for Artificial General Intelligence"* (**DOI 10.5281/zenodo.21383201**, record
published 2026-07-15, file `AGI.pdf`), and nothing beyond it.

The contract, quoted verbatim from §5.2 ("The shape of the contracts"):

> `EventBus / Publisher / Subscriber` — typed topics (attestation
> completed/failed, storage stored, amendments proposed/applied, kill switch
> activated, credentials verified/rejected, alerts); mandatory `EventMeta` with
> identifiers and schema version on every event; malformed events are
> unpublishable.

Its one-line invariant, quoted from Table 1:

> **Platform · `arkhe-event-bus`** — No crate knows another directly; all domain
> communication is typed pub/sub.

And its phase, quoted from §4.5:

> Phase 1 (2026–2027) delivers storage, identity, event bus, and observability
> with local IPFS and **in-memory pub/sub**.

This crate *is* that "in-memory pub/sub". The paper was downloaded and read while
writing this crate; it is not copied into the repository.

## This is a standalone workspace

`Cargo.toml` declares its own `[workspace]` table, following the existing
precedent in `safe-core-monorepo/crates/arkhe-tee` and
`arkhe-monorepo/packages/arkhe-core`. It is deliberately **not** listed in
`safe-core-monorepo/Cargo.toml`'s `members`, because that workspace does not
resolve today: its committed member list names crates that are absent from disk
(`arkhe-geometric-verifier`, `arkhe-security`, `arkhe-buzz-relay`,
`arkhe-web-gateway`, `arkhe-mcp-server`, `arkhe-orcid`, `arkhe-change`), so
`cargo metadata` fails at the root. The broken host workspace was **not**
repaired and nothing outside this directory was modified — see "Evidence" below.

## What is here

| Module | Purpose |
|---|---|
| `topic` | `Topic`: validated typed topics. The nine §5.2 topics are constants; unknown topics are accepted (`T-E3`). |
| `meta` | `EventMeta` with the four `T-E2` identifiers, `IdGenerator`, `SchemaVersion`. |
| `event` | `Event`: meta + JSON payload + optional signature, plus the canonical bytes a signature covers. |
| `signature` | `SignatureVerifier` (**pluggable; no crypto backend here**), `EventSignature`, `SignaturePolicy`. |
| `log` | `EventLog`: append-only, monotonic `SequenceNumber`s, Merkle root, `Checkpoint` fencing. |
| `bus` | `EventBus`: fail-closed `publish`, typed `subscribe`, delivery. |
| `invariant` | The invariants `T-E1`, `T-E2`, `T-E3` (paper-owned IDs) and `EB-01`, `EB-02` (crate-local), with executable checks where one exists. |
| `error` | `BusError`, with `is_rejection()` separating event refusals from transport failures. |

## The three publish gates

`publish` is the only way into the log, and it fails closed. No mutation happens
until every gate has passed:

1. **Structural** — mandatory `EventMeta`, non-empty identifiers, a legal topic,
   `schema_version.major >= 1`, and derived values (canonical payload, signing
   bytes, digest) that agree with the fields.
2. **Signature** — the `T-12` policy gate (below).
3. **Uniqueness and append** — under the log lock, so that the check and the
   append cannot interleave.

A refusal returns a typed `BusError` for which `is_rejection()` is `true`, and
leaves the log length, the Merkle root and every subscriber exactly as they were.
The tests assert the "leaves no trace" half explicitly, not only that an `Err`
came back.

## Threat-model coverage

From §6.2 of the paper:

| Threat | Mitigation named by the paper | Status here |
|---|---|---|
| **T-12** — man-in-the-middle between crates | "mTLS with 24-hour certificates plus per-event Ed25519 signatures verified by the bus" | The **signature half is implemented** (`bus` + `signature`). The mTLS half is **not** — it is transport, out of scope. |
| **T-13** — replay against the event log | "monotonic sequence numbers, Merkle roots, signed events, checkpoint fencing" | **All four present**: `EventLog` assigns strictly increasing sequence numbers; `current_root()` is a Merkle root over event digests; signed events are the `signature` module; `Checkpoint` + `EventLog::admits_as_new` is the fence. |

### No cryptographic backend is embedded, on purpose

The bus owns *where* verification happens and *what* bytes are covered. It holds
no key material and ships no Ed25519 implementation. `SignatureVerifier` is a
trait the embedding application implements with the crypto stack it already
audits, and the message it receives is `Event::signing_bytes()`:

```text
"ARKHE-EVENT-BUS/v1/event" ‖ u64-le(meta length) ‖ canonical meta ‖ canonical payload
```

Canonical JSON means `serde_json::to_value` → `serde_json::to_vec`, i.e. compact
JSON with object keys sorted (serde_json's default `Map` is a `BTreeMap`). Two
consequences, both asserted by tests:

* struct field order does not matter, so a future reordering of `EventMeta`
  cannot silently invalidate existing signatures;
* `to_json` → `from_json` preserves `signing_bytes()` and the digest
  **byte-for-byte**, so an event received as JSON verifies identically to the one
  the publisher signed.

Adding `ed25519-dalek` (or any backend) would put an implementation whose failure
mode is silent forgery into this crate's dependency graph. Phase 1 keeps that
decision with the caller, whose keys it is.

## Why `tokio::sync::broadcast`

Recorded as a decision, with the alternatives it beat:

* **Fan-out is the requirement.** §5.2's `Subscriber` is plural — attestation,
  storage, governance, security and observability all consume the same stream.
  `mpsc` gives one consumer per channel and would need one channel per subscriber
  per topic, hand-rolled.
* **A bounded buffer with an explicit lag report.** `broadcast` never blocks
  `publish` and never drops silently: a slow subscriber receives
  `BusError::SubscriberLagged { skipped }`, and the log still holds the events for
  replay. `watch` keeps only the newest value, losing exactly the history §4.2
  promises to replay.
* **`send` is synchronous**, so `publish` keeps §5.2's
  `publish(event) -> Result<SequenceNumber, _>` shape and needs no runtime; only
  the consuming side is `async`.
* **Every subscriber sees every event in log order** for events published by a
  single thread, which is what makes an `A → B → C` causal chain arrive as a
  chain.

Delivery semantics, stated rather than implied:

* **No history for late subscribers.** `broadcast` delivers from subscription
  time forward. Catch-up is `replay(seq)`; dedupe on `event_id`. There is no
  at-least-once or exactly-once guarantee in Phase 1 — there is "delivered while
  subscribed, and always in the log".
* **Ordering is the log's, not the channel's, when publishers race.** Concurrent
  publishers may be appended in one order and delivered in another. Every
  delivery carries its `SequenceNumber`, so a consumer sorts by it; the log never
  disagrees with itself. This is asserted by a test with eight threads.

## Dependencies

| Direct dependency | Why |
|---|---|
| `serde` (derive) | serialization of meta, identifiers, topics, schema versions |
| `serde_json` | the payload, the wire envelope, and canonicalisation |
| `tokio` (`sync` only) | `broadcast` for fan-out delivery |
| `thiserror` | `BusError` |
| `blake3` | event digests and the Merkle tree |
| `hex` | hex rendering of digests, roots and signatures |

### `blake3` is pinned to the pure-Rust implementation

`blake3`'s default features make its **build script compile C and assembly** for
the SIMD kernels through the `cc` crate. That was observed here, not assumed: a
fresh `target/` grew `blake3_avx2_x86-64_windows_msvc.o`,
`blake3_avx512_x86-64_windows_msvc.o`, `blake3_sse2_x86-64_windows_msvc.o`,
`blake3_sse41_x86-64_windows_msvc.o`, `blake3_avx512_assembly.lib` and
`blake3_sse2_sse41_avx2_assembly.lib`. A library should not require a C toolchain
of every consumer and every CI runner for that.

This crate therefore pins `default-features = false, features = ["std", "pure"]`,
which selects blake3's portable Rust implementation. The algorithm and the digests
are unchanged — only the implementation is — and the digest and Merkle-root tests
would fail if that were not true. Verified after the swap on a wiped `target/`:
no assembly objects and no `.lib` are produced, and the build script takes the
Rust-intrinsics path (annotated *"No C code to compile here"* in its source).

Two honest footnotes:

* blake3's build script still **probes** for a C compiler even in `pure` mode — it
  writes and tries to build `flag_check.c`, which is what the remaining
  `flag_check.obj` in `target/debug/build/blake3-*/out` is. Its source handles a
  missing compiler (`NoCompiler` → the Rust path), **but that path was not
  exercised here**: this host has a working compiler, so the graceful-degradation
  branch is read, not tested.
* This mirrors `arkhe-tee`'s pinned `rustcrypto` backend instead of `ring`, for the
  same reason. To recover the assembly, drop `default-features = false, features =
  ["pure"]` and accept the `cc` dependency.

The Cargo.lock inside this directory pins the exact set; `cargo tree --depth 1`
shows nothing beyond the six direct dependencies above.

## Invariants

| ID | Owner | Statement |
|---|---|---|
| `T-E1` | paper §4.2 | Domain crates never import each other; all domain-to-domain communication flows through `arkhe-event-bus` as typed pub/sub. A direct import fails compilation and blocks CI. |
| `T-E2` | paper §4.2 | Every event carries `event_id`, `trace_id`, `causation_id` and `correlation_id`, and they survive publishing, delivery, logging and a JSON round trip. |
| `T-E3` | paper §4.2 | Unknown topics, unknown payload members and future schema versions never panic a consumer; each yields a value or a typed error. |
| `EB-01` | this crate | §5.2's "malformed events are unpublishable": a refused publish leaves the log, its root and every subscriber untouched. |
| `EB-02` | this crate | Sequence numbers are monotonic from 1; replay reproduces the logged order exactly; the root is stable for the same log and moves when an event's bytes move; checkpoint fencing refuses already-fenced history. |

The repository's governed canonical ID spaces (`I6xx`, `I-xx`) are deliberately
**not** reused. `T-E1`/`T-E2`/`T-E3` keep the paper's own identifiers, and the
two crate-local invariants use this crate's own `EB-` prefix.

### `T-E1` cannot be checked from inside this crate

`T-E1` is a property **between crates**, enforced by the compiler and CI by the
absence of an import edge. Nothing running inside `arkhe-event-bus` can observe
whether `arkhe-attestation` imports `arkhe-storage`. It is therefore declared
with `check: None` and a `verification` string that names the real mechanism;
`tests/invariants.rs::t_e1_is_declared_with_the_papers_identifier_and_not_faked`
asserts that no runtime "check" is claimed for it. A fabricated self-test would
be evidence of nothing. **This crate does not claim `T-E1` as verified.**

## What the tests actually prove

```console
$ cargo test                          # exit 0
    unittests src/lib.rs               88 passed
    tests/audit.rs                     11 passed
    tests/contract.rs                  10 passed
    tests/invariants.rs                 7 passed
    tests/signature.rs                 10 passed
    Doc-tests arkhe_event_bus           3 passed
```

* **`tests/contract.rs`** — `T-E2` end to end (a three-event `A → B → C` chain:
  `causation_id` points at the previous event, `trace_id` and `correlation_id`
  stay constant, all four identifiers survive delivery, the log, and a JSON round
  trip); `T-E3` with `catch_unwind` around unknown topics, an unknown future
  *major* schema (which must produce a typed `UnsupportedSchemaVersion` naming
  the sequence, not a panic) and unknown payload members; the "unpublishable"
  rule over twelve distinct malformed envelopes, asserting after each one that
  the log length and the Merkle root did not move and that the subscriber
  received nothing; duplicate `event_id` refusal; all nine canonical topics.
* **`tests/audit.rs`** — monotonic sequence numbers, including under eight
  concurrent publishing threads (every sequence `1..=200` handed out exactly
  once); exact replay from an arbitrary sequence; Merkle-root stability for the
  same log, and movement when a payload changes, when two events are swapped, and
  when the log is truncated; every prefix root distinct; reconstruction from the
  event stream alone (the §4.2 "audited from the event stream alone" claim);
  checkpoint verification; and fencing — history at or below a checkpoint is
  refused as "new", a rewritten prefix is detected with a
  `CheckpointMismatch` naming both roots, and a stream that does not reach the
  fence is refused rather than trusted.
* **`tests/signature.rs`** — `SignaturePolicy::Required` without a verifier
  refuses both signed and unsigned events; a missing signature under `Required`
  is refused; a signature over different bytes is refused; the verifier is handed
  exactly `Event::signing_bytes()`, and again after a JSON round trip; a verifier
  that returns `false` is believed.
* **`tests/invariants.rs`** — the declared invariants pass; `T-E1` is declared
  with the paper's identifier and explicitly not faked; crate-local IDs avoid the
  governed namespaces; the documented delivery semantics (late subscribers get no
  history, the log is how they catch up, a dropped bus reports
  `SubscriptionClosed` instead of going quiet).

### What the signature tests do *not* prove

The stand-in verifier in `tests/signature.rs` accepts a signature equal to
`BLAKE3(message)`. That is **not a signature scheme** and is forgeable by anyone
who can read the message. It is the right instrument for testing the bus's
plumbing (refusal when unverifiable, exact bytes handed over, canonicalisation
stability), and it proves nothing about Ed25519.

## Not verified / out of scope

Explicitly **not** implemented here, and **not** claimed to work:

* **Persistence of any kind.** No disk, no `arkhe-storage`, no IPFS, no pinning.
  A process exit loses the log. (§4.5 pairs this bus with "local IPFS" in Phase 1;
  that half is not this crate.)
* **A cryptographic signature backend.** No Ed25519, no key management, no
  rotation. Only the pluggable trait, and only the plumbing it needs is tested
  (see above). `T-12` is therefore *half* mitigated here.
* **mTLS, 24-hour certificates, transport security** — the other half of `T-12`.
* **Global / cross-process uniqueness of `event_id`.** Uniqueness is enforced per
  `EventBus` instance, in memory. Two processes can mint the same id; nothing in
  this crate can detect that. Making it global needs the identity/storage crates.
* **`IdGenerator` is not an identity provider.** Its salt is process id plus
  first-use time; it is a convenience for keeping the per-bus uniqueness check
  meaningful, not an unforgeable identity, and not a CSPRNG.
* **Timestamp plausibility.** `EventMeta::timestamp_unix_us` is carried, never
  validated (monotonicity, skew, ordering). Phase 1 has no clock policy.
* **Merkle proofs of inclusion.** The root is computed and exposed; Merkle *paths*
  are not, so a third party cannot be handed a compact proof that a given event is
  in a given log — only the root comparison.
* **Merkle semantics for byte-identical events.** The root binds content. A swap
  of two events whose bytes are identical is not a distinguishable change,
  because there is nothing to distinguish. Reordering *distinct* events is
  detected.
* **At-least-once or exactly-once delivery.** See "Delivery semantics".
* **Back-pressure beyond the bounded channel.** A lagging subscriber is reported,
  not slowed down; the publisher is never blocked.
* **Consumer-side schema handling.** §4.2's `T-E3` gate is implemented
  (`SchemaVersion::is_readable_by`, `Subscription::recv_understanding`), and
  unknown *members* are preserved rather than dropped. Nothing here can guarantee
  that a consumer's own payload handling is forward compatible.
* **Load, latency and multi-node behaviour.** No benchmark, no soak test, no
  network, no consensus. `T-13`'s checkpoint fencing is a local mechanism here,
  not a distributed one.
* **C-free builds, only partly.** `blake3` with the `pure` feature compiles no C
  or assembly for its kernels, but its build script still probes for a C compiler
  and the no-compiler fallback was not exercised on this host (see
  "Dependencies").
* **Production hardening of the tokio runtime.** The async API needs a runtime;
  no runtime is created or owned by this crate.

### Declared divergences from the paper, and from the plan

* **Topic string syntax is invented here.** §5.2 names the topics in prose and
  fixes no string form. This crate encodes them as `lower_snake` segments joined
  by `.`, and pins the mapping in `topic::CANONICAL_TOPICS`. The prose word is the
  authority; the spelling is this crate's choice, documented so it can be amended
  rather than guessed.
* **`causation_id` is `Option<EventId>`.** §4.2 says every event *carries* all
  four identifiers. The field is always present in the struct and on the wire, and
  `None` explicitly means "this is a chain root" — a modelling decision, stated
  here instead of hidden.
* **`blake3` is pinned to `pure`**, where the plan named only "blake3". Reason and
  evidence are in "Dependencies": the default features compile C/assembly and
  impose a C toolchain on every consumer.
* **`src/event.rs` exists** in addition to the modules named in the plan, so that
  identity/versioning (`meta.rs`) and the message itself (`event.rs`) stay
  separate.
* **Four integration test binaries** instead of one `tests/integration.rs` (the
  `arkhe-tee` precedent), so that each test maps to one property.
* **`EventBus::log()` returns a clone**, not a lock guard: a snapshot keeps the
  mutex private and cannot be held across an `await`. The cost is copying the
  entry list (the events themselves are shared `Arc`s).
* **Lock poisoning is recovered from, deliberately.** Every critical section is a
  `Vec`/`HashMap` operation with no user code inside — signature verification,
  canonicalisation and validation all happen outside the locks — so a poisoned
  guard cannot describe a torn structure. The reasoning is recorded at the
  `lock` helper rather than left as an unexplained `unwrap_or_else`.
* **`publish`'s structural re-check is defence in depth.** The crate's
  constructors make an invalid `Event` unrepresentable, so gate 1 cannot fire
  through the public API today. The *reachable* refusals are the wire path
  (`Event::from_json`) and gates 2 and 3. Said plainly rather than presented as
  three equally reachable gates.

## Usage

```rust,no_run
use arkhe_event_bus::bus::{BusConfig, EventBus};
use arkhe_event_bus::event::Event;
use arkhe_event_bus::meta::{CorrelationId, EventId, EventMeta, SchemaVersion, TraceId};
use arkhe_event_bus::topic::{ATTESTATION_COMPLETED, KILL_SWITCH_ACTIVATED};

# fn main() -> Result<(), arkhe_event_bus::error::BusError> {
let bus = EventBus::new(BusConfig::default());
let mut attestations = bus.subscribe(ATTESTATION_COMPLETED);

// A chain root: one trace, one correlation.
let root = EventMeta::root(
    ATTESTATION_COMPLETED,
    SchemaVersion::V1,
    bus.new_event_id()?,
    TraceId::new("trc_settlement_7")?,
    CorrelationId::new("cor_settlement_7")?,
    arkhe_event_bus::now_unix_us(),
);
let root_sequence = bus.publish(Event::new(root.clone(), serde_json::json!({
    "stage": "requested",
    "graph": "dg_01H",
}))?)?;

// A caused event: same trace, same correlation, causation_id = the event above.
let child = root.caused_by(bus.new_event_id()?, arkhe_event_bus::now_unix_us());
bus.publish(Event::new(child, serde_json::json!({
    "stage": "verified",
    "proof": "0x…",
}))?)?;

if let Some(delivery) = attestations.try_recv()? {
    println!("seq {} {:?}", delivery.sequence().get(), delivery.event_id());
}

// The log is the authority; the channels are a convenience.
println!("events = {}", bus.log_len());
println!("root   = {}", bus.current_root().to_hex());
println!("replay = {}", bus.replay(root_sequence)?.len());

// A checkpoint fences what has been recorded so far.
let checkpoint = bus.checkpoint();
assert!(bus.verify_against(&checkpoint).is_ok());

// A different topic, same bus.
bus.publish(Event::new(
    EventMeta::root(
        KILL_SWITCH_ACTIVATED,
        SchemaVersion::V1,
        bus.new_event_id()?,
        TraceId::new("trc_kill")?,
        CorrelationId::new("cor_kill")?,
        arkhe_event_bus::now_unix_us(),
    ),
    serde_json::json!({ "shards": 4, "threshold": 4 }),
)?)?;
# Ok(())
# }
```

Using a real signature scheme means implementing one trait and opting in:

```rust,no_run
use std::sync::Arc;
use arkhe_event_bus::bus::{BusConfig, EventBus};
use arkhe_event_bus::signature::{SignaturePolicy, SignatureVerifier};

/// Your audited Ed25519 verifier lives in your crate, with your keys.
struct Ed25519Verifier { /* your key material */ }

impl SignatureVerifier for Ed25519Verifier {
    fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        // `message` is `Event::signing_bytes()` — always. `false` fails closed.
        # let _ = (message, signature); false
    }
}

let bus = EventBus::with_verifier(
    BusConfig::default().with_signature_policy(SignaturePolicy::Required),
    Arc::new(Ed25519Verifier { /* … */ }),
);
```

## Evidence

Produced from inside this directory on the host that wrote the crate (Windows,
x86_64, `cargo 1.94.0` / `rustc 1.94.0`):

```console
$ cargo check --all-targets                          # exit 0, no warnings
$ cargo clippy --all-targets -- -D warnings          # exit 0, no warnings
$ cargo test                                         # exit 0, 129 tests (see above)
$ cargo doc --no-deps                                # exit 0, no warnings

# The library has no panic paths: same strict lint set on production code only.
$ cargo clippy --lib -- -D warnings -D clippy::unwrap_used -D clippy::expect_used \
      -D clippy::panic -D clippy::todo -D clippy::unreachable -D clippy::indexing_slicing
                                                     # exit 0
# Control: the same lints over test code fail (exit 101, 24 errors), proving the
# instrument above is actually measuring something.
$ cargo clippy --all-targets -- -D clippy::panic -D clippy::unwrap_used
                                                     # exit 101, 24 errors, all in #[cfg(test)] / tests/
```

An independent audit of the library lines (test modules excluded) found no
`panic!`, no `unwrap()`, no `expect(`, no `unreachable!`, no `todo!` and no
`unimplemented!`; the only indexing is a full-range slice (`&self.signing_bytes[..]`),
which cannot panic. The one `unwrap_or_else` on a lock is the documented
poison-recovery path, not a panic path.

`#![forbid(unsafe_code)]` is declared in `src/lib.rs` and `unsafe_code =
"forbid"` in `Cargo.toml`.

### Nothing outside this directory was touched

* Every file written during the session sits under
  `safe-core-monorepo/crates/arkhe-event-bus/`: 2111 files under
  `safe-core-monorepo/` are newer than this crate's `Cargo.toml` (created at the
  start of the work), and **0** of them are outside the crate (the 2111 are the
  crate's sources, `Cargo.lock` and its own `target/`).
* `git status --short --untracked-files=no` reports the 51 tracked modifications
  that were already in the working tree before this work began — none of them
  under this crate. This crate is the only new path (`?? safe-core-monorepo/crates/arkhe-event-bus/`).
* `safe-core-monorepo/Cargo.toml` was **not** edited and this crate was **not**
  added to its `members`; the host workspace is left exactly as broken as it was
  found. No cargo command was run from it.

## License

`MIT OR Apache-2.0`, matching the rest of the tree.
