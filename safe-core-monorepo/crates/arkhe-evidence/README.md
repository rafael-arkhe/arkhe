# arkhe-evidence

FI-011/FI-017 — hash-chained, tamper-evident evidence log.

Each `EvidenceRecord` carries **two** BLAKE3 hashes (via `arkhe-core::hash`):

| field | formula | what it is |
|:---|:---|:---|
| `hash` | `hash(prev_hash ∥ len(payload) ∥ payload)` | the catalog's exact formula (FI-011/FI-017) |
| `record_hash` | `BLAKE3(domain ∥ index ∥ timestamp ∥ prev_hash ∥ len(payload) ∥ payload)` | additive binding over the whole record, domain tag `arkhe-evidence/record-hash/v1` |

## Status

17/17 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## Why two hashes, and why they are not one

The catalog formula is *specified and registered*: it is what FI-011/FI-017
asks for, and it cannot be rewritten without ceasing to implement those items.
But it covers only `(prev_hash, payload)` — `index` and `timestamp` are outside
its scope, so a record whose timestamp (or index) was edited after the fact
still has a matching catalog `hash`.

That gap is closed **additively**, the same practice the catalog documents
elsewhere ("built additively instead"): a second hash, `record_hash`, over the
record's own four fields. The first hash is left byte-for-byte as specified;
the second is added alongside it. The two live under distinct domain tags, so
no `record_hash` can ever equal a catalog `hash` and the two checks cannot be
confused.

**A chain is intact only when both match.** `EvidenceChain::verify_chain`
runs both checks and reports which one failed:

- catalog `hash` mismatch → `ChainError::HashMismatch` (payload edited);
- `prev_hash` disagrees with the actual predecessor → `ChainError::LinkBroken`
  (records swapped, or one removed from the middle);
- `record_hash` mismatch → `ChainError::RecordHashMismatch` (`index` or
  `timestamp` edited, or the stored `record_hash` itself).

`EvidenceChain::verify_catalog_chain` runs the catalog check **alone** — for
validating a chain against the FI-011/FI-017 contract exactly as registered,
without the metadata binding. Exposing it separately is the point: a caller
says which of the two verifications it wants.

## How this differs from the existing EvidenceBus

`arkhe-web3-security::agents::evidence_bus::EvidenceBus` already exists and
is real — it records `AuditEvidence { invariant_id, verdict }` for FI-W##
and FI-A## invariant checks. It's a flat `Vec`, with no linking between
entries: nothing detects a record being silently removed, reordered, or
edited after the fact. That's fine for what it's for (a live audit trail
you're actively appending to and reading), but it's not what FI-011/FI-017
ask for.

`EvidenceChain` here is the tamper-evident structure: each
`chain::EvidenceRecord` carries `prev_hash` plus its own `hash` and
`record_hash`, so `chain::EvidenceChain::verify_chain` can detect:
- a record's `payload` being edited after the fact (catalog hash mismatch),
- two records being swapped or reordered (a caught real bug during
  development — see below),
- a record being deleted from the middle (the following record's
  `prev_hash` would no longer match),
- a record's `timestamp` or `index` being edited after the fact
  (`record_hash` mismatch).

`chain::EvidenceChain::verify_at` verifies only a prefix (both checks within
it), satisfying FI-017's "hash chain é verificável em qualquer ponto" without
needing the whole chain to be present or valid yet.

## A real bug caught by the tests

The first version of the prefix check reported which record was broken using
`record.index` — the record's own self-reported field, set once at
`append` time. After a reorder attack (two records swapped), the record
now sitting at position 0 still carries its *original* `index` field
(`1`), so the error reported the wrong location — exactly backwards, since
a stale self-reported index is precisely the kind of thing this function
exists to catch, not something it can trust for reporting where the break
is. Fixed by using `.enumerate()` over the actual slice position instead.
See `swapping_two_records_breaks_the_link` in `chain.rs`'s tests.
