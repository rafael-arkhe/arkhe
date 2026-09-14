# arkhe-geometric-verifier

FI-120–FI-125 — canonicalises an agent turn into deterministic bytes, hashes
those bytes into a coordinate, runs every registered rule against the turn,
and builds the typed provenance (FI-122) and memory (FI-124) graphs from the
result. Used by `arkhe-agi`'s `AgiCoordinator`.

## Verifying a record

`GeometricVerifier::verify` is **synchronous**: `AgiCoordinator::process`
calls it on the turn's critical path, between the inference call and the
evidence append, where there is no I/O to hide behind an async state machine.

The canonical form is a fixed-order, length-prefixed encoding of `user_input`,
`response`, and `attested_by` (a `1` tag plus the length-prefixed value when
present, `0` when absent). Length prefixes make it unambiguous — `("ab","c")`
and `("a","bc")` cannot collide — and because attestation is part of the form,
an attested turn and an unattested one with identical text hash differently.

Rules are types, not closures:

```rust
use arkhe_geometric_verifier::{VerifiableRecord, VerificationRule};

impl VerificationRule for NonEmptyResponse {
    fn check(&self, record: &dyn VerifiableRecord) -> Result<(), String> {
        if record.response().trim().is_empty() {
            Err("the response is empty".to_string())
        } else {
            Ok(())
        }
    }
}
```

`Err` carries the reason only; the verifier attaches the rule's own type name
when it reports `GeometricError::RuleRejected`, so a rule cannot misname
itself and no two rules need a shared naming scheme. Rules run in registration
order and the first failure short-circuits.

## Graphs

`TypedGraph<N, E>` is a container with no policy: nodes and edges in insertion
order, addressed by position (`node_count`, `edges`, `node(i)`). The rules
that make a graph *valid* live in the modules that know what the nodes mean.

- `memory_graph::build_from_memory(memory, links)` — one node per stored
  memory entry, one edge per `(working_key, episodic_key, weight)` link.
  Nodes are identified by `key_node_id`, the BLAKE3 of the key, so the same
  key reached by two links is one node. A link naming a key that is not in
  memory fails the build (`UnknownKey`); the graph describes what is stored
  rather than what a caller claimed.
- `provenance_graph::build_from_chain(chain)` — one node per evidence record
  (node count equals record count; no node stands in for the genesis
  sentinel), one edge per adjacent pair. The chain is verified with
  `EvidenceChain::verify_chain` first, and the built graph is then run
  through `validate`, so `Ok` means what `AgiCoordinator::provenance_graph`
  documents: acyclic, and every node's `prev_hash` agrees with its
  predecessor's `hash`. `validate` is public because a graph can also arrive
  deserialised or hand-assembled.

## Tests

```
cargo test -p arkhe-geometric-verifier
```

40 unit tests, 6 integration tests over the public API, 7 doctests, and 7
tests in `tests/arkhe_agi_contract.rs`, which compiles the exact import block
`arkhe-agi/src/coordinator.rs:18-23` uses and mirrors that coordinator's call
sites — the check that the API really is the one the consumer needs, which
`cargo check -p arkhe-agi` cannot make (that crate's five missing internal
modules abort rustc before name resolution).

## Dependencies

`arkhe-core` (hashing, `AgentMemory`), `arkhe-evidence` (`EvidenceChain`),
`serde`, `thiserror`, `tracing`. Hashing goes through
`arkhe_core::hash::blake3_hash` rather than a direct `blake3` dependency, so
there is one version of the hash in play. `tokio` is a dev-dependency only —
nothing in the library uses a tokio type.
