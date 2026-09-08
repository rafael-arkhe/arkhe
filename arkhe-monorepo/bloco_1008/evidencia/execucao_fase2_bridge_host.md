# Evidencia — Bloco 1008 — Execucao da Fase 2: ponte dispositivo-hospedeiro (C-05..C-08)

Data: 2026-09-08. Comandos executados mecanicamente no monorepo
(workdir: `arkhe-monorepo`), apos a entrega do modulo
`packages/arkhe-safe-manifold/src/device_bridge.rs` (Fase 2).

## 1. Continuidade de numeracao e versao

- Bloco anterior no ledger: `bloco_1007` (v389.0). Proximo bloco: `1008`.
- Versao: `v389.1` (continua a linhagem v389.x iniciada no bloco 1007;
  o ledger nao salta versoes).

## 2. Modulo novo (Fase 2, host-side)

- `packages/arkhe-safe-manifold/src/device_bridge.rs`:
  - `DeviceConsciousnessBridge` — compoe o `ConsciousnessGovernanceBridge`
    existente (projecao: estado SafeManifold + ops de auditoria + complexidade)
    e um historico append-only de `DeviceObservation`.
  - `evaluate<B: PrologBackend>`: (a) projeta o estado host pelo motor Prolog
    real (`check_invariants` + `check_constitutional_safeguards`);
    (b) `host_phi_milli` = `phi_approximation * 1000` clampado em `[578,999]`;
    (c) C-01..C-04 do bitmask do report (C-01=1<<0 .. C-04=1<<3);
    (d) C-05 = Phi do dispositivo nao-regressa entre reports (None=cold start);
    (e) C-06 = `|phi_dev - phi_host| <= 40` quando o host tem >=2 observacoes;
    (f) C-07 = `1 - sigma(Phi)/50` clamp `[0,1]` (0.5 neutro ate 3 obs);
    (g) C-08 = fracao de C-01..C-06;
    (h) `combined_phi_milli` = `(3*dev + 2*host)/5` clampado.
  - `decide(&DeviceEvaluation)` — politica em ordem de prioridade:
    1. `!phi_in_window || !constitutional` (janela efetiva `(577,999]`; 578 =
       clamp floor tratado como degenerado) -> Rollback.
    2. `!host_state_ok` (invariantes ou salvaguardas Prolog inativas) -> Rollback.
    3. `metacognition == Some(false)` -> AlertHost.
    4. `dev_phi < 600 && adaptability >= 0.5` -> ReconfigureThreshold
       (mediana do historico, clampada na janela).
    5. Continue.
  - `DeviceDecision::into_host_decision()` -> `HostDecision` (CBOR-subset).
- Dependencia: `arkhe-firmware-consciousness` (path dep do mesmo workspace) —
  **FirmwareReport/HostDecision/FirmwareAction** reutilizados como fonte unica
  do protocolo; nenhuma dependencia externa nova (lockfile inalterado apos a
  adicao, alem da heapless ja presente).
- `lib.rs`: `pub mod device_bridge` + re-exports (`DeviceBridgeError`,
  `DeviceConsciousnessBridge`, `DeviceDecision`, `DeviceEvaluation`,
  `DeviceObservation`, constantes `GAP1_LOWER/UPPER_BOUND_MILLI`,
  `DEFAULT_OPERATIONAL_THRESHOLD_MILLI`, `CALIBRATION_TOLERANCE_MILLI`,
  `MASK_C01..C04`, e re-export de `FirmwareAction/FirmwareReport/HostDecision`).
- `audit.rs`: novo evento `AuditEventType::ConsciousnessDeviceDecision`
  (label `consciousness_device_decision`) + metodo
  `AuditLog::log_consciousness_device_decision`.

## 3. Verificacao mecanica

Cargo — testes arkhe-safe-manifold:
  test result: ok. 99 passed   (lib — inclui 13 novos em device_bridge::tests)
  test result: ok. 18 passed   (tests/consciousness_tests.rs)
  test result: ok. 4 passed    (tests/device_bridge_tests.rs — round-trip)
  test result: ok. 44 passed   (tests/integration.rs)
  test result: ok. 2 passed    (doc)
  => total 167/167 exit 0

Cargo — testes arkhe-firmware-consciousness (fase 1 intacta):
  test result: ok. 50 passed   (lib)
  test result: ok. 7 passed    (tests/integration.rs)
  => total 57/57 exit 0 (sem regressao)

Cargo — clippy (all targets, -D warnings):
  cargo clippy -p arkhe-firmware-consciousness -p arkhe-safe-manifold
    --all-targets -- -D warnings -> exit 0 (sem warnings)

Cargo — saneamento vizinho:
  cargo check -p arkhe-haselgrove -> ok (sem regressao)
  Cargo.lock raiz: ja continha heapless 0.8.0; nenhum novo pacote externo.

Nota honesta — check workspace-wide:
  cargo check --workspace -> falha PREEXISTENTE e ambiental (nao relacionada a
  esta fase): hardy-bpv7@0.6.0 / hardy-cbor@2.0.0 exigem rustc 1.95; toolchain
  instalado e 1.94.0. Nenhum crate de consciencia depende de 'hardy'; os crates
  alterados (arkhe-safe-manifold, arkhe-firmware-consciousness) e o vizinho
  arkhe-haselgrove compilam e testam exit 0 individualmente.

## 4. Correcao de liquor preexistente (clippy)

- O `-D warnings` expoe um lint PREEXISTENTE em
  `explore_critical_regions.rs:189`: `clone_on_copy` — `dim.clone()` onde
  `ExploreDimension` e `Copy`. Corrigido para `*dim`; zero mudanca de
  comportamento (semantica identica). Sem isso, o crate nao passaria clippy
  estrito nem antes desta fase.

## 5. Honestidade de medicao (C-05/C-06 Option<bool>)

- `None` (nao mensuravel) != `Some(false)` (falha medida).
- politica so age sobre falha medida:
  `cold_start_c06_is_unmeasured_not_failed` — primeira avaliacao (cold start,
  sem historia do host) tem `metacognition == None` e decide **Continue**
  (nao alerta, nao rollback).
- cobertos por testes: `uncalibrated_device_alerts_host` (erro 180 > 40 ->
  AlertHost), `non_constitutional_report_rolls_back`,
  `phi_below_gap1_window_rolls_back` (577 e 578 -> Rollback),
  `degraded_host_state_rolls_back`, `inactive_safeguards_roll_back`
  (MockProlog::empty), `steady_low_device_suggests_threshold_reconfigure`
  (590 estavel, erro 12, -> Reconfigure 590),
  round-trip integrado em `tests/device_bridge_tests.rs` (encode/decode
  CBOR-subset preserva acao, rollback e phi_threshold).

## 6. Hash real do bloco_1008 (reprodutibilidade)

- bloco_1008.json (UTF-8, LF): SHA-256 bruto
  `fe620b26ef6d1a07afdab48c558e658fb084ca6141797d1ce7bf0835075f2749`
- bloco_1008.json gzip (Optimal): SHA-256 comprimido
  `d932d50c077f536889b951d8d1d4ff613647b5c919b0bb3c182103e67d9cdfe0`