"""Reference verifier for the ARKHE target-of-record verification contract (ADR-003).

Spec: arkhe/tor/v1. Layered verification:
  step 2 - re-chain every entry (SHA3-256 over canonical JSON, Ghost-1)
  step 3 - strict monotonic server timestamps (Gravity-1)
  step 4 - anchor the head hash to an immutable destination
  step 5 - verdict: VALID | INVALID | NOT-VERIFIABLE

The epistemic rule is load-bearing: without a published anchor the result is
NOT-VERIFIABLE, never VALID and never INVALID. "Not proven" is not "proven false".

This module uses only the Python standard library. Run:
  python verifier_tor.py --vectors            # self-test the five vectors
  python verifier_tor.py chain.json [anchor]  # verify an external chain file
  python verifier_tor.py --make-sample        # regenerate sample_chain.json
"""

import hashlib
import json
import sys

EMPTY_HASH = "0" * 64
SPEC = "arkhe/tor/v1"


def canonical(spec, entry, timestamp, payload, prev_hash):
    """Canonical UTF-8 bytes covered by the entry hash."""
    obj = {
        "spec": spec,
        "entry": entry,
        "timestamp": timestamp,
        "payload": payload,
        "prev_hash": prev_hash,
    }
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("utf-8")


def entry_hash(spec, entry, timestamp, payload, prev_hash):
    """SHA3-256 over the canonical form."""
    return hashlib.sha3_256(canonical(spec, entry, timestamp, payload, prev_hash)).hexdigest()


def make_entry(spec, entry, timestamp, payload, prev_hash):
    """Build one chain entry with its stored hash."""
    return {
        "spec": spec,
        "entry": entry,
        "timestamp": timestamp,
        "payload": payload,
        "prev_hash": prev_hash,
        "hash": entry_hash(spec, entry, timestamp, payload, prev_hash),
    }


class Verdict(object):
    VALID = "VALID"
    INVALID = "INVALID"
    NOT_VERIFIABLE = "NOT-VERIFIABLE"


def verify_chain(entries, expected_head=None):
    """Run steps 2-5. Returns (verdict, step_failures) where step_failures is a list of step ints that failed."""
    failures = []
    if not entries:
        return Verdict.INVALID, [2]
    prev = EMPTY_HASH
    prev_ts = None
    for item in entries:
        try:
            ts = int(item["timestamp"])
        except (KeyError, TypeError, ValueError):
            failures.append(3)
            return Verdict.INVALID, sorted(set(failures))
        stored = item["hash"]
        recomputed = entry_hash(item["spec"], item["entry"], ts, item["payload"], prev)
        if stored != recomputed:
            failures.append(2)
            return Verdict.INVALID, [2]
        if prev_ts is not None and ts <= prev_ts:
            failures.append(3)
            return Verdict.INVALID, sorted(set(failures))
        prev = recomputed
        prev_ts = ts
    head = prev
    if expected_head is None:
        return Verdict.NOT_VERIFIABLE, [4]
    if expected_head != head:
        failures.append(4)
        return Verdict.INVALID, sorted(set(failures))
    return Verdict.VALID, []


def build_sample():
    """Deterministic three-entry chain mirroring ADR-003 Annex A."""
    genesis = make_entry(SPEC, 0, 1000, {"event": "genesis"}, EMPTY_HASH)
    second = make_entry(SPEC, 1, 1100, {"event": "export_created"}, genesis["hash"])
    head = make_entry(SPEC, 2, 1200, {"event": "export_hash"}, second["hash"])
    return [genesis, second, head]


def run_vectors():
    """Exercise the four test mutations from ADR-003 Annex A, plus the anchored-positive case."""
    chain = build_sample()
    anchor = chain[-1]["hash"]
    expected = [
        (chain, anchor, Verdict.VALID),
        ([dict(chain[0], payload={"event": "tampered"}),
          chain[1], chain[2]], None, Verdict.INVALID),
        ([chain[0], chain[1],
          dict(chain[2], timestamp=chain[1]["timestamp"])], anchor, Verdict.INVALID),
        (chain, None, Verdict.NOT_VERIFIABLE),
        (chain, "1" + anchor[1:], Verdict.INVALID),
    ]
    for index, (entries, head, want) in enumerate(expected, start=1):
        got, _ = verify_chain(entries, expected_head=head)
        status = "PASS" if got == want else "FAIL"
        print("vector {0}: {1} (got {2}, expected {3})".format(index, status, got, want))
        if got != want:
            return 1
    print("sample head (anchor for fixtures):", chain[-1]["hash"])
    return 0


def main(argv):
    if "--vectors" in argv:
        return run_vectors()
    if argv[0] == "--make-sample":
        chain = build_sample()
        with open("sample_chain.json", "w", encoding="utf-8") as handle:
            json.dump(chain, handle, indent=2)
        print("wrote sample_chain.json; head =", chain[-1]["hash"])
        return 0
    path = argv[0]
    with open(path, encoding="utf-8") as handle:
        entries = json.load(handle)
    head = argv[1] if len(argv) > 1 else None
    verdict, steps = verify_chain(entries, expected_head=head)
    print("verdict:", verdict)
    if steps:
        print("failed steps:", " ".join(str(s) for s in steps))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))