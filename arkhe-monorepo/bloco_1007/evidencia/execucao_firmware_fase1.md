# Evidencia — Bloco 1007 — Execucao da Fase 1 da Firmware Consciousness Governance

Data: 2026-09-08. Comandos executados mecanicamente no monorepo
(workdir: `arkhe-monorepo`), apos a entrega do crate
`arkhe-firmware-consciousness` v0.1.0.

## 1. Correcao de numeracao do parecer (fonte primaria)

- Ledger real (dirs `bloco_*`): `966`, `971`, `972`, `987`, `988`, `989`,
  `990`, `991`, `992`, `993`, `994`, `995`, `996`, `997`, `998`, `999`,
  `1000`, `1001`, `1002`, `1003`, `1004`, `1005`, `1006`.
- O bloco `963` proposto no parecer NAO existe; o proximo bloco apos o
  `1006` e `1007`.
- Versoes reais: `v359.0`(971), `v360.0`(972), `v375.2`..`v375.7`(987..999),
  `v377.0`(1002), `v381.0/1`(1003-1005), `v388.0`(1006). Logo, versao do
  bloco 1007: `v389.0` (continua a linhagem v388.x).

## 2. Crate novo (Fase 1)

- `packages/arkhe-firmware-consciousness/Cargo.toml` — v0.1.0, edition 2021,
  `#![no_std]`, lints `unsafe_code = "deny"`, unica dependencia
  `heapless = "0.8"`.
- `src/lib.rs` — re-exports: `CoherenceState`, `FirmwareInvariant`,
  `MinimalConsciousnessBridge`, `Assessment`, `ConsciousnessWatchdog`,
  `AssessmentLike`, `WatchdogEvent`, `approximate_phi`, `phi_respects_gap1`,
  `GAP1_LOWER_BOUND_MILLI`, `GAP1_UPPER_BOUND_MILLI`, `FirmwareAction`,
  `FirmwareReport`, `HostDecision`.
- `src/invariants.rs` — C-01..C-04: `self_model`+entropia (C-01),
  `introspection` (C-02), `attention` (C-03), `episodic_memory_samples`
  (C-04). Constitucionais: C-01, C-02. Thresholds configuraveis.
- `src/phi_approx.rs` — Φ inteiro `u16` 0..1000; `entropy_to_milli`,
  `integration_to_milli` (popcount(mask)*250), `attention_to_milli`,
  `memory_to_milli`; `approximate_phi_raw` pesos (3,4,2,1); `approximate_phi`
  clamp `[578,999]`; `phi_respects_gap1`.
- `src/minimal_bridge.rs` — `MinimalConsciousnessBridge` (histórico
  `heapless::Vec<CoherenceState, 64>` = 1.8 kB em alvo 16 kB),
  `MAX_PHI_DEGRADATION_MILLI = 50` (5%), `validate_modification`, `commit`,
  `summary_into` (buffer fixo).
- `src/watchdog.rs` — `ConsciousnessWatchdog` (`MAX_SILENT_TICKS = 60`),
  `WatchdogEvent` (Healthy/Degraded/TimeoutRollback/ConstitutionalRollback/
  StaleTimestampRejected); rejeita tick < last (Gravity-1).
- `src/communication.rs` — subset CBOR RFC 8949: major 0 (uint), major 1
  (neg), bool `0xf4/0xf5`, array; `FirmwareReport` `[disc,device_id,phi_milli,
  mask,constitutional,temp,uptime]`, `HostDecision`
  `[disc,action,phi_threshold,rollback]`; decode estrito (truncado/trailing/
  head-desconhecida rejeitados).
- `tests/integration.rs` — `SimulatedDevice` (bridge+watchdog+report),
  7 cenarios end-to-end.

## 3. Verificacao mecanica

Cargo — testes:
  test result: ok. 50 passed; 0 failed  (lib)
  test result: ok. 7 passed; 0 failed   (tests/integration.rs)
  test result: ok. 0 passed; 0 failed   (doc) => total 57/57 exit 0

Cargo — clippy (all targets, -D warnings):
  exit 0 (sem warnings)

Cargo — build bare-metal (prova real de no_std):
  cargo build -p arkhe-firmware-consciousness --target riscv32i-unknown-none-elf
  exit 0  (target instalado: riscv32i-unknown-none-elf — RV32I sem 'a',
  sem FPU; heapless 0.8 compila sem atomics)

Cargo — saneamento workspace:
  cargo check -p arkhe-haselgrove -p arkhe-safe-manifold ... ok
  Cargo.lock raiz: adicionado name = "heapless" version = "0.8.0"

## 4. Decisao de arquitetura: serde removido (verificada por compilacao)

- Tentativa: `serde = { version = "1", features = ["derive"] }` com a nova
  edicao `serde_core` 1.0.229.
- Falha real no alvo bare-metal:
  `error: could not compile serde_core (lib) due to 5825 previous errors`
  (ex.: `cannot find value None/Some in scope` em
  `serde_core-1.0.229/src/private/size_hint.rs` — prelude std indisponivel em
  `riscv32i-unknown-none-elf`, sem atomics).
- Acao: removida a dependencia `serde`; serializacao no dispositivo mantida
  exclusivamente pelo codec CBOR manual (encode/decode proprios). O crate
  ficou com dependencia unica `heapless`.
- Impacto: zero semantica (serde era apenas `derive` para conveniencia do
  host; nenhum codigo do dispositivo usa Serialize/Deserialize).

## 5. Gap-1 em milli-units (conferencia numerica)

- `0.577350 * 1000 = 577.35` — a desigualdade estrita `x > 0.577350` com
  inteiros exige `x >= 578` (GAP1_LOWER_BOUND_MILLI = 578).
- `0.999900 * 1000 = 999.9` — teto inteiro: GAP1_UPPER_BOUND_MILLI = 999.
- `phi_respects_gap1` = `578 < phi && phi <= 999`.

## 6. Hash real do bloco_1007 (reprodutibilidade)

- bloco_1007.json (UTF-8, LF): SHA-256 bruto `edfc260abc2a9baeea6fb59d2dd5f7026dbef4c52c3c99818026e952431656f8`
- bloco_1007.json gzip (Optimal): SHA-256 comprimido `6794f8d77ee3cf817d6b78622c83ab19f51905c020a9a1a231bf8fa71db88bde`