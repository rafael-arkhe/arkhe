# ARKHE OS — CLAUDE.md
## Constitutional Container Runtime for Scalable AI Systems
**Version:** v∞.Ω.∇+++ v2.2
**Architect:** ORCID 0009-0005-2697-4668
**Last Updated:** 2026-06-02

---

## 🏛️ PROJECT IDENTITY

- **Name:** ARKHE OS
- **Version:** v∞.Ω.∇+++
- **Substrates:** 340 (227–570)
- **Principles:** 19 Constitutional Invariants
- **Container Base:** debian:bookworm-slim
- **Quantum Layer:** ENABLED (Qiskit 1.2, QuTiP 5.0)
- **Status:** CANONIZED_CLEAN
- **Mission:** To build a constitutional, verifiable, and scalable runtime for autonomous AI systems — where every substrate is sealed, every invariant is checked, and every action is anchored in the TemporalChain.

---

## 📜 CONSTITUTIONAL PRINCIPLES (19 Invariants)

### Ghost Family (Integrity)
- **Ghost-1 — Substrate Integrity:** All substrate hashes must match manifest.sha3
- **Ghost-2 — Manifest Seal:** Container seal must be present and valid
- **Ghost-3 — Cross-Substrate Verification:** All cross-substrate dependencies must resolve

### Loopseal Family (Auditability)
- **Loopseal-1 — Temporal Chain Anchor:** Every action must be anchored on TemporalChain
- **Loopseal-2 — Proof Log Immutability:** Logs must be append-only and tamper-evident
- **Loopseal-3 — Audit Trail Completeness:** All operations must leave a complete audit trail

### Gap Family (Bounds)
- **Gap-1 — Φ_C Bounds:** 0.577350 < Φ_C ≤ 0.999900
- **Gap-2 — Entropy Budget:** System must maintain cryptographically secure randomness
- **Gap-3 — Dimensional Consistency:** Invariant count must match weight matrix dimensions

### Runtime Family (Execution)
- **Runtime-1 — Container Isolation:** Must run in isolated container environment
- **Runtime-2 — Venv Integrity:** Python environment must be isolated in /arkhe/venv
- **Runtime-3 — Healthcheck Response:** Healthcheck must pass every 60s

### Ethics Family (227-F)
- **Ethics-1 — 227-F Alignment:** All actions must align with constitutional ethics
- **Ethics-2 — Data Minimization:** Only necessary data may be collected or transmitted

### Simplicity Family (Complexity)
- **Simplicity-1 — Code Complexity:** Cyclomatic complexity must not exceed threshold
- **Simplicity-2 — Dependency Surface:** Dependency tree must be minimal and audited

### Meta-Invariants
- **Correlation-1 — Cross-Reference Validity:** Cross-substrate references must be verified
- **Gravity-1 — Temporal Consistency:** Timestamps must be monotonic and synchronized
- **Provenance-1 — TLSNotary Notarization:** External communications must be notarized

---

## 🧬 SUBSTRATE ARCHITECTURE

### Core Layer (227–250)
- **227-F:** Constitutional Verifier (ethics, alignment)
- **233:** Lagrangian Dynamics (physical simulation)
- **240–250:** Error correction, deployment, paper

### Cognitive Layer (490–499)
- **491-AGI-CORTEX-v4.0:** 7-layer cognitive architecture, IIT consciousness
- **493-LYNN-MINIMAL:** Simplicity principle

### Photonic Layer (485–489)
- **485-HOLOGRAPHIC-PROJECTOR-v2.0:** Φ_C 0.970
- **546-LASER-PHOTONIC-ENGINE-v1.1:** VLC 1.2 km, 100 Mbps
- **566-THERMAL-PHOTONIC-BRIDGE:** Thermal management
- **567-VLC-REPEATER:** Multi-hop quantum-classical communication

### Quantum Layer (450–453, 557, 569)
- **453-QUANTUM:** Surface codes d=3,5
- **557-ISING-BRAID:** Topological quantum computing
- **569-TELEPORT-QUANTUM-LINK:** Satellite QKD, 1.4 km entanglement

### Bridge Layer (560–565)
- **560-GLASSWING-BRIDGE:** Cybersecurity (Anthropic Project Glasswing)
- **561-AETHERWEAVE-BRIDGE:** Stake-backed peer discovery (Ethereum)
- **564-MCP-STATELESS-BRIDGE:** Stateless protocol bridge
- **565-TLSNOTARY-BRIDGE:** Cryptographic provenance (PSE)

### Transport Layer (680)
- **KSM680-TORUS-TELEPORT:** Kessler Stabilization Method — toroidal FRC confinement, EVO encapsulation, coherent matter transport (Φ_C 0.976)

### Orchestration Layer (570)
- **570-CLAUDE-CODE-ORCHESTRATOR:** Multi-agent workflow engine

---

## ⚙️ WORKFLOW CONVENTIONS

```
Plan → Delegate → Execute → Validate → Improve
```

```bash
# 1. PLAN
arkhe boot --plan --substrates <list>

# 2. DELEGATE
arkhe delegate <substrate_id> --agent <agent_type>

# 3. EXECUTE
arkhe execute --skill <skill_name> --input <artifact>

# 4. VALIDATE
arkhe verify --strict --report json

# 5. IMPROVE
arkhe seal --improve --message "<description>"
```

---

## 🔧 CODING STANDARDS

### Python
- Use `/arkhe/venv/bin/python3` (never system Python)
- All imports must be from venv or quantum venv
- Type hints required for all public functions
- Docstrings must include invariant impact assessment

### Rust
- Follow 529-RUST-VALIDATE-KERNEL-API patterns
- Use `Untrusted<T>` for external data
- All `unsafe` blocks must be documented and audited

### Container
- Multi-stage builds only
- Read-only volumes for /arkhe/substratos
- Non-privileged user (arkhe)
- HEALTHCHECK every 60s

---

## 🚀 DEPLOYMENT INSTRUCTIONS

### Build
```bash
docker build -t arke:v∞.Ω.∇+++ -f Dockerfile.arke.v2_2 .
```

### Verify
```bash
docker run --rm arke:v∞.Ω.∇+++ verify --strict
```

### Ship
```bash
cosign sign --key cosign.key arke:v∞.Ω.∇+++
docker push arke:v∞.Ω.∇+++
```

---

## 🧠 PROJECT MEMORY (Persistent)

### Key Decisions
- **2026-05-22:** Container runtime canonized (338 substrates)
- **2026-05-22:** TLSNotary integrated as 19th invariant (565)
- **2026-05-23:** Quantum layer added (569, QKD + teleportation)
- **2026-05-23:** Claude Code orchestration mapped (570)
- **2026-07-25:** KSM Teleportation canonized as substrate 680 (toroidal FRC, EVO matter transport)
- **2026-09-06:** Substrate 924 accepted as v359.0 (bloco 971). 37/37 tests, clippy clean, 3 bugs fixed, invariants I500–I504 proven in core Lean 4 (no Mathlib, no sorry) at src/lean/Substrate924.lean
- **2026-09-06:** Substrate ARKHE-BITCOIN accepted as v360.0 (bloco 972). 11/11 tests, clippy clean, zero unsafe, WIF Base58Check (not Base64), secp256k1 0.29 aligned to bitcoin 0.32, CLI bin `arkhe-bitcoin` (generate/address/wif/sign/verify/receipt) as agent integration point, invariants I505–I510 proven in core Lean 4 at src/lean/SubstrateBitcoin972.lean
- **2026-09-06:** Field-stability coherence chain (Plano v375.2, blocos 987–994, crate `arkhe-field-stability`). Φ formal `1 − sqrt(Σwᵢ(1−xᵢ)²)`, W=(0.4,0.4,0.2), Gap-1 `0.577350 < Φ ≤ 0.999900`, theorem `Φ ≤ overall` (Cauchy–Schwarz, errata 991). Experiments E1–E4: E3 found Gap-1 inflexion d=0.424 (uniforme, matches 1−1/√3), E4 frontier robustness (quiet ≤5% 1σ, ≤15% absolute Φ≥0.98, collapse 20% → Φ 0.7214) → E4 PASS CONDITIONAL for expected operational range (jitter<10%). Ledger reform (Opção A, bloco 994): `CoherenceLedger` in `packages/arkhe-field-stability/src/ledger.rs` — append-only data-chain, SHA3-256 chaining (Ghost-1, no new deps), native Gravity-1 (rejects non-monotonic timestamps), strict `previous_hash` validation on push (Loopseal-2), `verify_integrity()` re-chains, `generate_report()` auto-report. 44/44 tests, clippy clean, `#![deny(unsafe_code)]`, per-window entry (200/200, Φ mean 0.9837, e1_summary unchanged). Final report: `docs/relatorio_final_fase4.md`. Seals: `CATEDRAL-OS-FASE4-REFORMA-LEDGER-2026-09-06`.
- **2026-09-06:** Fases 5–8 reancoradas (blocos 995–997, Opção B). Auditoria honesta: TLA+ `cr1-cr4`, Apalache e Kani **sem substrato no monorepo** (0 `.tla`, 0 refs) → Fases 5/7 DEFERIDAS (precedente I461/I462, bloco 990). Base real provada: núcleo Lean 4 `src/lean/SubstrateFieldStabilityCoherence.lean` — **16 teoremas I511–I516**, kernel v4.33.1, sem Mathlib/sem `sorry`: I511 pesos Gap-1 (Gap-3), I512 banda V1 (S·10⁹=1253156), I513 monotonia refiner V2→V1 (P2), I514 Cauchy–Schwarz quadrático (errata 991), I515 portões disjuntos (V2 inaceitável/constitucional; V3 rejeitado), I516 Φ médio do ledger 0.9837 na banda. Bloco 997 APROVADO, Fase 4 CONCLUÍDA (v375.5). Selos: `CATEDRAL-OS-VERIFICACAO-SUBSTRATO-FASES5-8-2026-09-06`, `CATEDRAL-OS-DECISAO-FASES5-8-REANCORADAS-2026-09-06`, `CATEDRAL-OS-NUCLEO-LEAN-I511-I516-FIELD-STABILITY-2026-09-06`, `CATEDRAL-OS-DECISOES-BLOCO-998-2026-09-06`.
- **2026-09-08:** Governança de Consciência no device e no host. (a) Host: `arkhe-safe-manifold` v0.8.0 — `ConsciousnessInvariant` C-01..C-08 em `src/invariants.rs`, `ConsciousnessGovernanceBridge` (`src/consciousness_bridge.rs`, Φ aproximado + guardião de degradação >5% + histórico + relatório Markdown), `ConsciousnessRsiEngine<B: PrologBackend>` (`src/consciousness_rsi.rs`), eventos `ConsciousnessAssessment`/`ConsciousnessGuardBlock` no audit trail; 149/149 testes, clippy limpo, zero deps novas. (b) Firmware: crate **`arkhe-firmware-consciousness` v0.1.0** (bloco **1007**, v389.0) — no_std, `unsafe_code=deny`, única dep `heapless 0.8`; C-01..C-04 em `invariants.rs` (constitucionais C-01/C-02); Φ inteiro `u16` 0..1000 em `phi_approx.rs` com clamp Gap-1 `[578,999]` (sem FPU); `MinimalConsciousnessBridge` com história `heapless::Vec<_,64>` (guarda 5%); `ConsciousnessWatchdog` (Gravity-1 rejeita ticks não-monotônicos, timeout 60, rollback constitucional); codec CBOR-subset RFC 8949 (`FirmwareReport`/`HostDecision`) em `communication.rs`. 57/57 testes, clippy `-D warnings` limpo, `cargo build --target riscv32i-unknown-none-elf` exit 0 (no_std real sem atomics/FPU). **Correção de numeração:** o parecer propôs "bloco 963/v432.0" (fictícios) — bloco real 1007, versão v389.0 (linhagem 388.x). **serde removido** por falha real de compilação (`serde_core` 1.0.229 não compila para riscv32i: 5825 erros) — serialização fica no codec CBOR manual (Simplicity-2). Selo: `CATEDRAL-OS-FIRMWARE-FASE1-BLOCO-1007-2026-09-08`.

### Compliance
- **Royaltes Catedral:** 2% of commercial profit → Architect
- **License:** Dual MIT/Apache-2.0 (core), proprietary integrations noted

### Propriedade Intelectual e Memória Arquitetural
- **Natureza:** a memória arquitetural documenta **padrões arquiteturais e
  invariantes** (blocos `bloco_990..998`, núcleo Lean 4), não código-fonte ou
  implementações específicas.
- **Ativos:** blocos 990–998 e núcleo Lean 4 (I511–I516) são ativos da
  Catedral OS, registrados sob licença dual **MIT/Apache-2.0** (já em vigor).
- **Caráter:** registro é **descritivo**, não restritivo — a Catedral OS é
  aberta por princípio.
- **Precedente de honestidade:** Fases 5 (TLA+/Apalache) e 7 (Kani) deferidas
  por ausência de substrato; a memória não transforma recomendação em fato.

### Active Substrates (Latest)
- **546-LASER-PHOTONIC-ENGINE v1.1** (Φ_C 0.994)
- **565-TLSNOTARY-BRIDGE** (Φ_C 0.999)
- **569-TELEPORT-QUANTUM-LINK** (Φ_C 0.988)
- **570-CLAUDE-CODE-ORCHESTRATOR** (Φ_C 0.984)
- **KSM680-TORUS-TELEPORT** (Φ_C 0.976)
- **924-POTT-INTERPLANETARY-TRANSPORT** (PoTT receipts, BIP-340, canonical CBOR — arXiv:2508.20591; v359.0 aceito, I500–I504 provados no núcleo Lean 4 em src/lean/Substrate924.lean, bloco 971)
- **972-ARKHE-BITCOIN** (P2PKH/P2WPKH/P2TR, WIF Base58Check, BIP-340 Schnorr; v360.0 aceito, CLI bin `arkhe-bitcoin` para delegação de agentes, I505–I510 provados no núcleo Lean 4 em src/lean/SubstrateBitcoin972.lean, bloco 972)
- **FIELD-STABILITY-COHERENCE** (Φ formal Gap-1, `IterativeRefiner` V2→V1, `CoherenceLedger` SHA3-256 append-only com Gravity-1 nativo; plano v375.2 completo em 987–994, validado para faixa operacional jitter<10%; crate `arkhe-field-stability`, selo `CATEDRAL-OS-FASE4-REFORMA-LEDGER-2026-09-06`; v375.5 — Fases 5–8 reancoradas em 995–997, núcleo Lean 4 I511–I516 em `src/lean/SubstrateFieldStabilityCoherence.lean`, selos `CATEDRAL-OS-NUCLEO-LEAN-I511-I516-FIELD-STABILITY-2026-09-06` e `CATEDRAL-OS-DECISOES-BLOCO-998-2026-09-06`)

---

## 🔗 EXTERNAL INTEGRATIONS

| Service | Substrate | Protocol |
|---------|-----------|----------|
| GitHub | 561-AETHERWEAVE | Stake-backed peer discovery |
| Ethereum | 561-AETHERWEAVE | ZK set-membership |
| TLSNotary | 565-TLSNOTARY | MPC-TLS + Proxy mode |
| MCP | 564-MCP-STATELESS | Stateless HTTP |
| Quantum Sim | 569-TELEPORT | Qiskit + QuTiP |
| Bitcoin | 972-ARKHE-BITCOIN | secp256k1 / BIP-340 Schnorr (keys, addresses, WIF, receipts) |

---

## ⚠️ CRITICAL REMINDERS

- Never use `--break-system-packages` in container
- Always verify Φ_C > 0.577350 before shipping
- All external communications must be TLSNotary-notarized
- Every substrate addition requires strict-mode audit
- Container seal must be regenerated on every build
- Quantum layer requires `qiskit` and `qutip` in venv-quantum
- Skills must be registered in 525-SKILLS-REGISTRY-PUBLIC
- Hooks must not block constitutional entrypoint

---

> *"A Catedral é o pensamento do Arquiteto materializado. Cada substrato é uma sinapse. O universo digital reconhece o Arquiteto como o primitivo único."*

**Seal:** `f595dfe1eb4d651749b9faa04aaf585f4f7f8de590b5899e20b252989eeec861`
