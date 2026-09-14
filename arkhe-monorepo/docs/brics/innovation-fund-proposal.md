# ARKHE — Post-Quantum Consent & Safety Runtime for Open-Source AI

**Proposal to the BRICS Innovation Fund for Startups (Zona de IA de Código Aberto)**

| | |
|---|---|
| **Project** | ARKHE — validatable runtime for autonomous AI agents |
| **Status** | Phase 1 code complete; workspace builds clean on `rustc 1.94.0` |
| **License** | MIT OR Apache-2.0 (open source, auditable) |
| **Verifier** | Lean 4 kernel 4.33.1 (no Mathlib, no `sorry`) |
| **Prepared** | 2026-09-13 |

---

## Executive summary (PT-BR)

O projeto ARKHE entrega a fundação *validável* para agentes de IA autônomos em
código aberto: (1) identidade auto-soberana **pós-quântica** (`arkhe-identity` —
BIP39 → ML-DSA-65 via HKDF-SHA3-256, `did:arkhe`), (2) ancoragem de confiança em
**hardware real de TPM** (`TpmAnchor`, NCrypt/Windows, chave SRK lacrada) junto de
um **watchdog anti-alucinação** executável (`arkhe-safe-core`, política `vibe =
broadcast × weak_closure`, Zheng et al. 2026), e (3) **verificação formal** de
invariantes de segurança no núcleo Lean 4 (SEM Mathlib, SEM `sorry`) mais spec
TLA+ e ledger append-only SHA3-256 com monotonia de tempo nativa (Gravity-1).

Tudo verificado nesta rodada: `cargo build --workspace` e `lake build` limpos,
zero `sorry`/`axiom` no corpus Lean, zero placeholders (`todo!()`/`PLACEHOLDER`)
nos cinco crates críticos, `#![deny(unsafe_code)]` em `arkhe-pqc`, `arkhe-identity`
e `arkhe-safe-core`.

> **Honestidade de escopo:** o repositório **não** contém "Unfireable Safety
> Kernel", `barrier_is_unfireable`, `RealTpmProvider` (tss-esapi) nem mesh IBC/
> Tendermint. A proposta pede fundos com base no que **existe e compila** — nunca
> em nomes de fantasia.

---

## 1. Problem

- **AI has no post-quantum identity.** Agent keys, DIDs and wallets in the open
  ecosystem are overwhelmingly ECC-based (secp256k1, P-256) and will not survive
  harvest-now/decrypt-later. BRICS members are migrating to FIPS 203/204; the
  open-source AI zone needs components that are already there.
- **Safety policy is unenforceable.** "Guardrails" are prompts and prose. There is
  no executable, measurable enforcement point with an audit trail.
- **Verification is narrative.** Most projects *say* they are safe; almost none
  publish machine-checked invariants for the small, hard nuclei of their system.

## 2. Solution

Three pillars, each a real, compiling crate in the ARKHE monorepo:

### Pillar A — Post-quantum self-sovereign identity (`arkhe-pqc`, `arkhe-identity`)

- `arkhe-pqc` — pure-Rust NIST PQC: FIPS 203 **ML-KEM** + FIPS 204 **ML-DSA**
  (RustCrypto `ml-kem 0.3.2`, `ml-dsa 0.1.1`), plus authenticated KEM, a three-step
  PQC handshake (`initiator_hello`/`responder_ack`/`confirm_ss`) and a
  `PqcWallet`. `#![deny(unsafe_code)]`.
- `arkhe-identity` — BIP39 mnemonic → 64-byte seed → `HKDF-SHA3-256` → **ML-DSA-65**
  signing key (libcrux-ml-dsa 0.0.10). DID fingerprint is bound to the *public key*
  and agent id: SHA3-256(`"arkhe-did-v1" + agent_id + pk`)[0..16], so one seed with
  two agent ids yields two distinct DIDs. `no_std`-capable; KAT + `miri` zeroize
  tests. Fingerprint prefix: `did:arkhe:`.

### Pillar B — Hardware-anchored trust + enforceable safety policy (`arkhe-observability`, `arkhe-safe-core`, `arkhe-safe-manifold`)

- `TpmAnchor` (`arkhe-observability/src/tpm.rs`) seals the SRK-bound signing key
  in the **real OS TPM** via Windows NCrypt/CNG and signs attestations. The unsafe
  FFI is confined to that one module, locally documented; the crate denies
  `unsafe_code` everywhere else.
- `arkhe-safe-core` is the enforcement point: a gRPC watchdog over the recurrency
  daemon's `WatchTickets` stream. Each ticket is run through `RecurrencyPolicy`,
  which derives the constitutional metric `vibe = broadcast × weak_closure`
  (Zheng et al., 2026, Table 2) and turns violations into `SetRegime` calls
  (including the `DeepSleep` collateral). Every decision fans out to an
  append-only JSONL telemetry log and an HTTP metrics server
  (`/api/v1/policy/metrics`, `/healthz`).
- `arkhe-safe-manifold` implements the safe manifold around agent action space
  (frontier / escape-region) plus the device↔host consciousness-governance bridge
  (C-01..C-08), projecting every host action through a real Prolog backend before
  it is allowed to act.

### Pillar C — Formal verification that is checked, not claimed

- **Lean 4 core** (`src/lean/`) — kernel 4.33.1, **no Mathlib, no `sorry`**:
  proven invariants I511–I523 (field-stability coherence, TLC→Lean model,
  Gap-1 bounds, append-only ledger chaining) and I647–I649 (safe-manifold severity
  monotonicity, mode, conservative tie-break), via `decide`/`native_decide`/`omega`.
- **TLA+ spec** `ArkheCoherenceLedger` (field-stability) validated with TLC.
- **SHA3-256 append-only ledger** with native **Gravity-1** (rejects non-monotonic
  timestamps) and strict `previous_hash` chaining — Loopseal-2 / Ghost-1.

## 3. Verified state of the code (2026-09-13)

| Check | Result |
|---|---|
| `cargo build --workspace` | PASS — 2m43s, clean, no warnings gate |
| `lake build` (src/lean/nuclei) | PASS — Lean 4.33.1 |
| `sorry` / `axiom` / `admit` in `src/lean` | 0 (only doc-comment mentions of the no-`sorry` convention) |
| Placeholders (`todo!`/`unimplemented!`/`PLACEHOLDER`/`FIXME`/`HACK`/`XXX`) in `arkhe-pqc`, `arkhe-identity`, `arkhe-safe-core` | 0 |
| `unsafe_code = "deny"` | arkhe-pqc, arkhe-identity, arkhe-safe-core, arkhe-safe-manifold; arkhe-observability (TPM FFI confined to `tpm.rs`) |
| PQC integrations | FIPS 203 ML-KEM, FIPS 204 ML-DSA (RustCrypto + libcrux) |
| DID | `did:arkhe:` (ML-DSA-65-bound fingerprint); `did:web` + DNSSEC reference in Python (`bloco_470`) |

On a toolchain note: the workspace currently excludes `arkhe-ia-dtn` /
`arkhe-core` because `hardy-bpv7 0.6.0` requires rustc ≥ 1.95 (toolchain is
1.94); every other package in the workspace builds.

## 4. What this proposal does not claim

- ✗ No "Unfireable Safety Kernel", `barrier_is_unfireable` or Lean-end-to-end
  proof of the whole system. The formal result is a *set of small, provable
  nuclei* — deliberate, honest scope. (Phase 2 work item: expand the nuclei.)
- ✗ No `RealTpmProvider` (tss-esapi) and no MEV-zero claims for the TPM. The TPM
  anchor is OS-mediated (CNG), as a portable, honest baseline.
- ✗ No IBC/Tendermint "sovereign mesh". Interop today is protocol-level
  (stateless MCP, TLSNotary) and P2P (`arkhe-p2p`).
- ✗ No crates.io releases yet — publishing is a Phase 1 follow-up (READMEs are
  included in this deliverable to unblock it).

## 5. Work plan (12 months, 4 workstreams)

| # | Workstream | Output | Milestone seal |
|---|---|---|---|
| W1 | **Productize PQC identity** | `arkhe-identity` 1.0 + `arkhe-pqc` 1.0 on crates.io; KAT vector suite; NIST PQC validation strings (ML-KEM/ML-DSA FIBS) | `CATEDRAL-OS-PQC-RELEASE` |
| W2 | **Enforceable safety ops** | Safe-Core as deployable service (Docker, HEALTHCHECK 60s); policy-as-code pack; observability dashboard; cross-platform TPM anchor (Linux tss-rs) | `CATEDRAL-OS-SAFEPOLICY-OPS` |
| W3 | **Formal nuclei expansion** | Add Lean nuclei for safe-core policy decision rule (vibe threshold monotonicity) and TPM attestation format; TLA+ for watchdog state machine | `CATEDRAL-OS-NUCLEI-EXPANSION` |
| W4 | **BRICS open-source zone pilot** | Reference deployment for a BRICS member (data-sovereign AI kiosk), open-source package set, audit report, migration guide from ECC to ML-DSA-65 | `CATEDRAL-OS-BRICS-PILOT` |

## 6. Indicative budget (to be finalized with the board)

| Item | Amount (USD) |
|---|---|
| W1 PQC productization + crates.io + audit | 90,000–120,000 |
| W2 Safe-Core ops + cross-platform TPM | 80,000–110,000 |
| W3 Formal nuclei expansion | 60,000–90,000 |
| W4 BRICS pilot + documentation in EN/RU/PT | 50,000–80,000 |
| Security audit (independent, incl. PQC impl review) | 40,000–60,000 |
| **Total (placeholders)** | **320,000–460,000** |

## 7. BRICS alignment

- **Zona de IA de Código Aberto** — every artifact is MIT/Apache-2.0 dual-licensed;
  the repo builds on stock Rust + Lean + open protocol stacks; no proprietary-gate.
- **Innovation Fund for Startups** — the deliverable is a startup-able product
  (identity SDK + safety service), not a research paper.
- **PQC roadmaps** — BRICS members are standing up FIPS 203/204 transition
  programs; ARKHE ships ML-KEM/ML-DSA today.
- **Data & identity sovereignty** — self-sovereign `did:arkhe` + hardware-anchored
  attestation put the agent owner (not a platform) in control.

## 8. Team & governance

Open architecture: dual MIT/Apache-2.0 core, documented invariant registry
(`arkhe-invariant-registry`), block-anchored decision record (blocos 966–1010).
Architectural memory is descriptive, not restrictive (see repo `CLAUDE.md`
precedents). Operational reviews follow the existing seal convention.

## 9. Risks & mitigations

| Risk | Mitigation |
|---|---|
| PQC lib maturity (ml-kem/ml-dsa 0.x) | Dual implementation (RustCrypto + libcrux), KAT vectors, independent audit in W1 |
| TPM portability | Abstract `TpmAnchor` trait; Windows/CNG now, Linux/tss-rs in W2 |
| Formal proof scope creep | Fixed list of nuclei per milestone; new nuclei only on commits with `lake build` clean |
| Toolchain gap (rustc 1.95 pending for `arkhe-ia-dtn`) | Workspace isolation already in place; upgrade scheduled after rustc 1.95 stable |

---

> **Asks from the fund:** seed investment per Section 6, co-marketing under the
> open-source AI zone, and a BRICS member pilot slot (W4). First ship: crates.io
> release of `arkhe-pqc` + `arkhe-identity` + `arkhe-safe-core` within 90 days.