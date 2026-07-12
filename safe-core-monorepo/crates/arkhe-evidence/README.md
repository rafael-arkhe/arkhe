# arkhe-evidence

FI-011/FI-017 — hash-chained, tamper-evident evidence log:
`hash(prev_hash ∥ len(payload) ∥ payload)` per record (BLAKE3, via
`arkhe-core::hash`), matching the catalog's formula exactly.

## Status

9/9 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## How this differs from the existing EvidenceBus

`arkhe-web3-security::agents::evidence_bus::EvidenceBus` already exists and
is real — it records `AuditEvidence { invariant_id, verdict }` for FI-W##
and FI-A## invariant checks. It's a flat `Vec`, with no linking between
entries: nothing detects a record being silently removed, reordered, or
edited after the fact. That's fine for what it's for (a live audit trail
you're actively appending to and reading), but it's not what FI-011/FI-017
ask for.

`EvidenceChain` here is the tamper-evident structure: each
[`chain::EvidenceRecord`] carries `prev_hash` and its own `hash`, so
[`chain::EvidenceChain::verify_chain`] can detect:
- a record's `payload` being edited after the fact (hash mismatch),
- two records being swapped or reordered (a caught real bug during
  development — see below),
- a record being deleted from the middle (the following record's
  `prev_hash` would no longer match).

[`chain::EvidenceChain::verify_at`] verifies only a prefix, satisfying
FI-017's "hash chain é verificável em qualquer ponto" without needing the
whole chain to be present or valid yet.

## A real bug caught by the tests

The first version of `verify_prefix` reported which record was broken using
`record.index` — the record's own self-reported field, set once at
`append` time. After a reorder attack (two records swapped), the record
now sitting at position 0 still carries its *original* `index` field
(`1`), so the error reported the wrong location — exactly backwards, since
a stale self-reported index is precisely the kind of thing this function
exists to catch, not something it can trust for reporting where the break
is. Fixed by using `.enumerate()` over the actual slice position instead.
See `swapping_two_records_breaks_the_link` in `chain.rs`'s tests.
