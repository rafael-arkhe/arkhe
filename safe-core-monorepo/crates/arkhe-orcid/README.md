# arkhe-orcid

ORCID iD parsing, checksum validation, and verification against the ORCID
public API — used by `arkhe-agi`'s `AgiCoordinator` to attest each turn's
provenance record to a verified researcher identity.

## Two halves

`OrcidId` is pure and offline: `OrcidId::parse` validates the shape
(`XXXX-XXXX-XXXX-XXXX`, hyphens optional on input) *and* the ISO 7064 mod
11-2 check character, so a value of that type is a value that could be a real
ORCID iD. The canonical spelling is what `Display` emits, and what serde
writes — parsing `0000000218250097` yields `"0000-0002-1825-0097"` on the wire
in both directions.

`OrcidClient` is the network half. `OrcidClient::new()` targets
`https://pub.orcid.org/v3.0`; `OrcidClient::with_base_url(..)` points it
somewhere else (tests, mirrors, proxies). `verify` issues

```
GET {base_url}/{orcid}/person
Accept: application/json
```

and turns the person record into an `OrcidVerification`
(`display_name`, `id`, plus the raw `given_names`/`family_name`) — the shape
ORCID's `/person` endpoint actually returns:

```json
{"name":{"given-names":{"value":"Josiah"},"family-name":{"value":"Carberry"}}}
```

A `2xx` whose body has no name at all is an `OrcidError::UnexpectedResponse`
rather than a verification with a blank name: attesting turns to nobody while
reporting success would be worse than an error.

## Status mapping

| Response | Error |
|---|---|
| `2xx` | parsed; bad JSON or no name → `UnexpectedResponse` |
| `404` | `NotFound { orcid }` |
| `429` | `RateLimited` |
| `5xx` | `Provider { status }` |
| other non-`2xx` | `UnexpectedResponse { reason }` |
| no response (DNS/TLS/timeout) | `Transport(String)` |

The provider's error *body* is deliberately not carried in `Provider` — ORCID
error pages are large and unparsed, and `Display` would become unreadable.

## Tests

```
cargo test -p arkhe-orcid
```

16 unit tests (checksum algorithm, canonicalisation, serde, request URL,
display-name joining), 10 integration tests against a real local HTTP server
via `mockito` (path and `Accept` header asserted, every status branch, JSON
without a name, an unreachable port), and 4 doctests. One further test is
`#[ignore]`d because it hits the live ORCID API:

```
cargo test -p arkhe-orcid -- --ignored
```
