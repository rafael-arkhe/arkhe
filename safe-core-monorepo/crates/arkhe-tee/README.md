# `arkhe-tee`

TDX / RTMR attestation primitives for the Arkhe OS tree.

This crate is a **standalone workspace**: it declares its own `[workspace]` table
in `Cargo.toml`, following the existing precedent in
`arkhe-monorepo/packages/arkhe-core`. It is deliberately **not** listed in
`safe-core-monorepo/Cargo.toml`'s `members`, because that workspace does not
resolve today — its committed member list names crates that are absent from disk
(`arkhe-geometric-verifier`, `arkhe-security`, `arkhe-buzz-relay`,
`arkhe-web-gateway`, `arkhe-mcp-server`, `arkhe-orcid`, `arkhe-change`), so
`cargo metadata` fails at the root. Nothing outside this directory was modified.

## What is here

| Module | Purpose |
|---|---|
| `report_data` | `report_data = BLAKE3("ARKHE-TEE" \|\| payload)` zero-padded to 64 bytes. Pure, deterministic. |
| `quote_meta` | Extracts `MRTD`, `MR_CONFIG_ID`, `MR_OWNER`, `MR_OWNER_CONFIG`, `RTMR0..3`, `report_data` and `MR_SERVICE_TD` from a quote. |
| `collateral_cache` | Stores `QuoteCollateralV3` on disk with a TTL and explicit provenance (`Fresh` / `LocalCache` / `StaleCache`). |
| `verifier` | Fetches collateral from a PCCS, falls back to the cache, then verifies with `dcap_qvl::verify::verify`. |
| `rtmr` | Extends RTMRs through the Linux TSM sysfs interface or `/dev/tdx-guest`. |
| `binding` | `ProvenanceBinding`: GDID + binary hash + quote version + `MRTD` + RTMRs + `report_data`. |
| `invariant` | Crate-local invariants `TEE-01..TEE-04` with executable checks. |

## Dependencies

* **`dcap-qvl` 0.6.3** (MIT) — quote decoding and the full DCAP verification path.
  The APIs used are its real public surface:
  * `dcap_qvl::collateral::CollateralClient::with_default_http(url)` + `.fetch(quote)`
  * `dcap_qvl::PHALA_PCCS_URL`
  * `dcap_qvl::verify::verify(raw_quote, collateral, now_secs)`
  * `dcap_qvl::quote::Quote::parse` and `Report::as_td10()` / `Report::as_td15()`
* **`tdx-quote` is deliberately absent.** Its real license is
  `AGPL-3.0-or-later`; depending on it would contaminate this crate's
  `MIT OR Apache-2.0` terms. Everything the original design wanted from it —
  quote v4/v5 parsing, `report_data`, the RTMRs, `MRTD` — is already reachable
  through `dcap-qvl`. If it ever becomes genuinely necessary, it must be added
  behind a **default-off** feature (e.g. `agpl-tdx-quote`) with the license
  consequence documented in the manifest; it is not present today, at any
  feature setting.

### Backend swap: `rustcrypto` instead of `ring`

`dcap-qvl`'s default features include `ring`, a crypto backend whose build script
compiles C and assembly for the *target*. That builds natively on MSVC, but it
makes any cross-target build depend on a cross C toolchain. The crate therefore
pins `default-features = false, features = ["std", "rustcrypto", "default-x509"]`.
With `ring` unenabled, `dcap_qvl::verify::verify` resolves to the pure-Rust
`rustcrypto` backend — the predictable choice `dcap-qvl`'s own documentation
recommends. Verification behaviour is unchanged.

### Feature flags

| Feature | Default | Effect |
|---|---|---|
| `pccs` | on | Compiles the live PCCS collateral client. This is the **only** thing in the graph that pulls a TLS stack: `dcap-qvl/report` → `reqwest` → `rustls` → `aws-lc-sys`, a C library. |

Turning `pccs` **off** yields a cache-only offline verifier: quote verification
stays fully available, the network path returns a clear `CollateralFetch` error,
and the build needs no C toolchain at all. That is what makes the Linux
cross-check reproducible:

```console
$ cargo check --target x86_64-unknown-linux-gnu --all-targets --no-default-features   # exit 0
$ cargo check --target x86_64-unknown-linux-gnu --all-targets                         # exit 101 (see below)
```

The second command fails **before `arkhe-tee` is ever compiled**, inside
`aws-lc-sys`'s build script (`cc-rs: failed to find tool "x86_64-linux-gnu-gcc"`),
because this host has no Linux cross C compiler. Cargo features are additive, so
enabling `pccs` cannot be prevented from that side; the `--no-default-features`
run is the honest proof that this crate's own Linux-only code compiles.

## What the tests actually prove

`cargo test` covers, entirely without hardware and without network access:

* **Pure logic** — `report_data` derivation (determinism, 32-byte digest plus
  32 zero bytes, payload and domain sensitivity); the RTMR extension law
  `SHA-384(previous || new)` including a published SHA-384 known-answer vector;
  the agent-artifact digest; index validation; and the ioctl request-number
  arithmetic.
* **Real parsing paths** — quotes are synthesised locally with `scale` (the same
  codec `dcap-qvl` decodes with) and pushed through the real
  `dcap_qvl::quote::Quote::parse`, for TD10 (quote v4), TD15 (quote v5) and SGX
  (which must be rejected with an explicit error). `report_data` mismatches are
  detected against these parsed quotes.
* **Cache behaviour** — write/read JSON round-trips, TTL boundaries, stale vs
  fresh classification, provenance rewriting on load, absent cache, and corrupt
  cache (reported, never trusted).
* **Verification failure modes** — `dcap_qvl::verify::verify` returns `Err` for
  garbage input and for a well-formed but unsigned quote; it never panics.
* **Fail-closed behaviour** — with no reachable PCCS and no cache, verification
  returns a typed error rather than a result.

## What is **not** verified here

**No line of this crate has been run on TDX hardware.** Specifically, none of
the following has been observed to work:

* generating a real TDX quote, or verifying one against Intel's live trust chain;
* fetching real collateral from a real PCCS (`pccs.phala.network` or any other) —
  every test either fails before reaching the network or uses synthetic
  collateral with no valid signature;
* extending an RTMR on a real TD, or reading one back;
* the kernel ABI of the RTMR extension path. `TDX_EXTEND_RTMR_IOCTL` is
  *computed* from `size_of::<TdxExtendRtmrReq>()` rather than transcribed, and
  the struct layout (`data[64]` + `u64` index, 72 bytes) is an assumption. The
  original design note's hard-coded `0x40085403` decomposes as `_IOW('T', 3, 8)`
  — an 8-byte size field, inconsistent with any 56- or 72-byte payload. A test
  pins that decomposition so the discrepancy cannot be forgotten, but **the
  correct ABI must be confirmed against the target kernel's headers**;
* whether writing to `/sys/kernel/tsm/mr/rtmrN` extends the register or replaces
  it. The code assumes *extend* and submits the raw digest;
* whether `dcap_qvl::verify`'s TCB policy accepts a given production platform.

Consequently, `RtmrExtension` separates three distinct claims — `submitted`,
`expected` (the locally computed SHA-384 law) and `observed` (read back from the
register) — so that a caller can tell them apart rather than trusting a
successful write. Off Linux, every hardware operation returns
`TeeError::RtmrUnsupportedPlatform`; the crate compiles and its pure logic is
tested on Windows and Linux alike.

## Invariants

Identifiers are crate-local (`TEE-*`) and intentionally do **not** reuse the
repository's governed canonical ID spaces (`I6xx`, `I-xx`).

| ID | Statement |
|---|---|
| `TEE-01` | `report_data` is exactly 64 bytes: domain-tagged BLAKE3 in the first 32, zeros in the last 32, deterministically derived. |
| `TEE-02` | An RTMR extension is `SHA-384(previous \|\| new)` over full 48-byte operands — never truncated, never reordered. |
| `TEE-03` | Collateral that did not come from a live PCCS fetch is reported as `degraded` and is never classified `Fresh`. |
| `TEE-04` | A quote whose `MRTD`, RTMRs or `report_data` disagree with a recorded `ProvenanceBinding` is rejected, naming the field that moved. |

## Usage

```rust,no_run
use std::time::Duration;
use arkhe_tee::{parse_tdx_quote, TdxVerifier};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let quote = std::fs::read("quote.bin")?;

// Offline inspection needs no collateral and no hardware.
let metadata = parse_tdx_quote(&quote)?;
println!("MRTD   = {}", metadata.mr_td_hex());
println!("RTMR1  = {}", metadata.rtmr(1).unwrap());

// Verification does need collateral; the cache is optional.
let verifier = TdxVerifier::production()
    .with_cache_dir("/var/cache/arkhe-tee")
    .with_collateral_ttl(Duration::from_secs(6 * 3600));

match verifier.verify(&quote) {
    Ok(v) => println!(
        "valid={} tcb={} via {} (degraded={}, age={:?})",
        v.valid, v.tcb_status, v.collateral_source.describe(), v.degraded, v.collateral_age
    ),
    Err(e) => eprintln!("verification failed: {e}"),
}
# Ok(())
# }
```

`TdxVerifier` also exposes `verify_async` for use inside an existing async
runtime; the synchronous `verify` returns `TeeError::NestedRuntime` rather than
panicking when called from within one. A `network_timeout` of zero disables the
network path entirely, which is how strictly offline operation is expressed.

## License

`MIT OR Apache-2.0`, matching the rest of the tree.
