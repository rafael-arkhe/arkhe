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

Scope limits: this profile does not replace existing legacy governance policies
automatically, consume operation nonces, provide journal atomicity, certify SLSA
levels, or prove universal policy compatibility. The loaded binary, its build,
host trust configuration and dispatcher remain part of the trust boundary.

Artifacts contain the actual WASM, bundle, captured trust, source/run/commit
metadata and observed native/WASM test results. Check the workflow conclusion;
the presence of this source alone is not evidence that signing succeeded.
