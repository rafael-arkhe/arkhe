# Arkhe verifier and authenticated policy release

Isolated, reviewable snapshot of the actual D:/arkhe production verifier source,
not the loader demo's answer-42 fixture. This does not replace the repository's
existing root Cargo workspace.

The new `policy` module is shared by native tests and the production WASM. It
checks principal signatures, exact request-to-intent equality, worker-to-key
authorization and signatures, record contents/hash, audience, policy digest,
time bounds, spending cap and module/version pairs. Canonical signatures cover
domain-separated, length-prefixed bytes including the full payload. The caller
must supply trusted registries, policy and time; agents cannot supply authority.

Compatibility tests generate only v1-permitted requests and execute 10,000 cases
for each of compatible expansion and explicitly justified version removal.
Migration reasons are separate from runtime permissions and cannot relax spending.
Adversarial vectors include a positive control and 15 rejected attacks, including
the original changed-digest scenario 10 and changing the amount while re-signing
a consistent worker record. Test signing seeds are synthetic test credentials,
not production keys.

The workflow builds the production `arkhe-verify-wasm`, signs it using GitHub
OIDC/Cosign and public Rekor, captures public trust using Cosign's authenticated
TUF client, verifies offline independently with Cosign and the pinned Rust 0.6.6
loader, then executes those policy vectors inside the admitted WASM with Wasmtime.
Browser-only imports trap if invoked; none grant synthetic verification success.

The workflow exact signer identity is tied to its repository/path/ref. The loader
also pins the WASM digest before compiling exactly the verified bytes. A saved
root directory is trusted host configuration; never trust a root supplied solely
by an unverified bundle. The Rust 0.6.6 TUF acquisition path failed locally; the
workflow deliberately uses Cosign's TUF client instead, without bypassing checks.

Legacy integration: the production runtime factory wraps the existing resolver
with an operator-configured authenticated context. Existing admission and broker
calls then require the signed request, bind its principal, worker, operation,
canonical parameters and policy to the legacy intent, and preserve the original
governance requirements. `AuthenticatedEvidence` transports untrusted proof bytes;
it grants no authority. Executors must implement `execute_authenticated` and use
the verified typed action; the default refuses execution. The `host-dispatch`
package compiles the exact factory source from the runtime; it is not the entire
runtime HTTP service.

The SQLite journal atomically consumes the caller nonce with the operation
reservation. Completed operations replay their saved receipt. In the local SQL
path, effect, receipt, reservation and nonce commit in one transaction, or all
roll back, including on panic. The callback is trusted to perform local SQL only.
For external effects, the reservation is durable before dispatch. Error, panic,
abrupt process exit or receipt-write failure leaves `UNCERTAIN`; restart refuses
redispatch and requires reconciliation. This is not an atomic transaction with
an external provider and does not promise exactly-once provider execution.

Enable `sqlite-dispatch` in the full runtime and construct its authenticated
factory using trusted operator registries/policy, a durable database path and an
adapted executor. Existing callers without that configured resolver retain their
legacy profile; no live deployment is changed by this snapshot. The authenticated
AP2 payment path refuses dispatch until closed-mandate and nonce consumption can
be combined. SLSA levels and universal compatibility remain outside this evidence.
The loader binary, build, host configuration and dispatcher remain trusted.

The journal suite includes a real subprocess exiting with code 86 after a synced
external-effect marker, concurrent independent connections, restart/replay,
nonce reuse, parameter mismatch and SQL rollback. Its ignored child test is
invoked explicitly by the parent crash test; it is not missing coverage.

Artifacts contain the actual WASM, bundle, captured trust, source/run/commit
metadata and observed native/WASM test results. Check the workflow conclusion;
the presence of this source alone is not evidence that signing succeeded.
