# Evidencia — Bloco 1011 — Camada GPU da Catedral: invariantes I536/I537/I538

Data: 2026-09-09. Comandos executados mecanicamente no monorepo
(workdir: `arkhe-monorepo`), apos a entrega do nucleo Lean
(`src/lean/GpuInvariantsNucleus.lean`) e do novo membro do workspace
`packages/arkhe-gpu`.

## 1. Continuidade de numeracao e versao

- Bloco anterior no ledger: `bloco_1010` (v390.1, I534/I535 — gate de
  orquestracao). Proximo bloco: `1011`.
- Versao: `v390.2` (continua a linhagem real v389.0/v389.1/v390.0/v390.1; o
  v520.0/v521.0/v605.0 sao documentos paralelos nao canonizados).
- `handover_anterior`: **1010** (inteiro = numero do bloco, formato real).
- IDs usados: **I536/I537/I538** — livres. I500–I530 usados (I530 =
  decaimento, bloco 1009); I531/I532 reservados; I533 = Loopseal real (v389.0);
  I534/I535 = gate (bloco 1010). Os documentos v520.0/v521.0 reivindicavam
  I530–I537 como GPU e I647–I650 — **IDs inexistentes/colididos** e substrato
  ficticio (sem bloco_1045, sem crate `arkhe-gpu`).

## 2. Correcao dos defeitos dos documentos v520.0/v521.0/v605.0

1. **Substrato inexistente**: v520.0/v521.0 citavam «implementacao v520.0,
   bloco 1045» e invariantes I647–I650 com provas `by rfl` — nada disso existe
   no monorepo (sem bloco_1045, sem crate, sem kernel). Este bloco constroi o
   substrato real e prova **I536/I537/I538** no kernel.
2. **Mathlib + `by rfl`**: os documentos usavam `import Mathlib`, `ℝ` e provas
   tautologicas. O nucleo deste bloco e **core Lean 4.33.1 sem Mathlib, sem
   `sorry`** (convencao bloco 966), aritmtica Nat, com trabalho real por
   inducao/cases/omega — nao tautologia.
3. **Nomes/numerao errados**: «bloco 7» nao existe (proximo e 1011); o crate
   do crates.io e `cutile` (NVIDIA/cutile-rs) — **nao** `cutile-rs` como nome
   de pacote; o mock proposto usava `tensors: HashMap` + `self.tensors.lock()`
   (mutabilidade interior sem `Mutex`) — aqui o mock usa `Mutex<HashMap<…>>`.
4. **«PRONTO PARA DEPLOY» sem kernel**: o veredito e SEMPRE do kernel real
   (FFI `std::process` -> `lean.exe`), fechado por teste de integracao, e o
   registo hash e calculado sobre o JSON canonico (evidencia abaixo).

## 3. Nucleo Lean — `src/lean/GpuInvariantsNucleus.lean`

`ArkheGpu`, 7 teoremas elaborados no kernel (`lean` exit 0, v4.33.1, sem
Mathlib, sem `sorry`):

| Teorema | Enunciado (escala Nat/ms) | Tacticas |
| :--- | :--- | :--- |
| `I536A_wellformed_entries_satisfied` | `well_formed t -> t.e -> e.1 = satisfied` (isolamento: a trilha nao esquece a violacao) | `induction`, `cases` |
| `I536B_threads_within_band` | `allowed_geometry t g -> t <= 1024` | `rcases` |
| `I536C_grid_within_band` | `allowed_geometry t g -> g <= 2147483647` | `rcases` |
| `I536D_geometry_positive` | `allowed_geometry t g -> 0 < t / 0 < g` | `rcases` |
| `I537A_violations_le_launches` | `violations t <= launches t` (inducao estrutural) | `induction`, `omega` |
| `I537B_wellformed_has_no_violations` | `well_formed t -> violations t = 0` | `induction`, `cases` |
| `I538A_temporal_band_closed_under_retention` | `d <= 5000 -> k <= 5000 -> (d+k) <= 10000` (fecho Tile->SIMT) | `simp`, `omega` |

- Constantes: `max_threads_per_block = 1024`, `max_grid = 2147483647`,
  `tile_cap_ms = 5000`, `simt_cap_ms = 10000` — mesmas do crate Rust
  (`governance.rs`), fechadas por teste FFI contra o TEXTO aceito pelo kernel.
- Modelo: `LaunchStatus` (satisfied/violated), `LaunchOutcome` (ok/failed),
  `AuditEntry = LaunchStatus x LaunchOutcome x Nat`, `launches`,
  `contract_violations`, `failed_launches`, `well_formed` — espelho 1:1 do
  `audit.rs`.
- Nenhum `sorry`; nenhum `import` alem do preludio core.

Nota de elaboracao: nao houve `by rfl` tautologico — I537-A e por inducao
estrutural com `omega` nos dois ramos de `LaunchStatus`; I536-A e por inducao
sobre a lista com `cases` do `∈`; I538-A prova o fecho aditivo da banda
(`simp` das defs + `omega`) — trabalho real sobre a semantica do crate.

## 4. Crate Rust — `packages/arkhe-gpu`

Novo membro do workspace (registrado no Cargo.toml raiz). `unsafe_code deny`,
`missing_docs warn`. Zero dependencias externas novas no core (Simplicity-2):
`cutile` (0.2, crates.io) e opcional atras da feature `tile` — nunca ativada
neste ambiente; `arkhe-lean-bridge` apenas como dev-dependency do teste FFI.

| Módulo | Conteudo | Invariante |
| :--- | :--- | :--- |
| `governance.rs` | `MAX_THREADS_PER_BLOCK=1024`, `MAX_GRID=2147483647`, `TILE_CAP_MS=5000`, `SIMT_CAP_MS=10000`, `allowed_geometry`, `tile_duration_ok`, `simt_duration_ok`, `evaluate`/`LaunchDecision`/`RejectReason`, `i536a..d`, `i537a/b`, `i538a` | I536 (banda G-07), I537, I538 |
| `audit.rs` | `LaunchAudit` append-only; `record` so grava `Satisfied` (debug_assert I536-A); `launches`/`contract_violations`/`failed_launches`/`is_well_formed` | I536-A, I537-A/B |
| `backend.rs` | `GpuBackend` (allocate/launch/execute/sync), `LaunchPlan`, `BackendError`, `BackendKind` (Mock/Tile/Simt) | — |
| `mock.rs` | `MockGpuBackend` com `Mutex<HashMap<u64, Vec<f64>>>` + `Metrics` atomicos; governanca consultada antes da execucao (out-of-band -> Err sem executar) | I536/I537 no nivel do mock |
| `metrics.rs` | contadores atomicos (launches/failures/violations/busy) | — |
| `tensor.rs` | `TensorId` opaco, `Shape`/`numel` | — |
| `lib.rs` | docs honestos + `GPU_NUCLEUS_INVARIANTS` (7 nomes) + `GPU_NUCLEUS_LEAN_RELATIVE` | — |

## 5. Paridade FFI — `packages/arkhe-gpu/tests/gpu_kernel.rs`

- `kernel_elaborates_gpu_nucleus_without_sorry`: chama o kernel real
  (`LeanKernel::from_env()`) sobre o nucleo e exige `verdict.is_sound()`
  (digest SHA3-256 + exit 0 + 7/7 acknowledges) e `stderr` vazio.
- `gpu_constants_parity_with_kernel_accepted_text`: fecha a paridade contra o
  TEXTO que o kernel aceitou (1024/2147483647/5000/10000) — alterar uma
  constante no nucleo ou no Rust faz o teste falhar (convencao da ponte).
- `kernel_sound_maps_to_allow_and_in_band_geometry`: veredito sintetizado
  saudavel + geometria dentro da banda autoriza; fora (0, 1025, 2^31) nunca.

## 6. Verificacao mecanica

Lean — nucleo I536/I537/I538:
  lean.exe src/lean/GpuInvariantsNucleus.lean -> exit 0 (7 elaboracoes)
  (kernel v4.33.1 local: C:\Users\Lemes\.elan\bin\lean.exe)

Cargo — testes arkhe-gpu:
  test result: ok. 14 passed (lib) + 3 passed (tests/gpu_kernel.rs) = 17
  Unidades: audit::tests.{i537a_b_satisfied_on_wellformed,
  ad_hoc_violated_entry_is_counted_but_never_pushed_by_api,
  append_only_no_removal_ops_present},
  governance::tests.{geometry_band_exact_edges, i536b_c_threads_and_grid_never_above_band,
  track_caps_and_i538_retention_closure, evaluate_decision_typed},
  mock::tests.{mock_launch_ok_fills_tensor_and_audits_satisfied,
  mock_failed_outcome_still_satisfies_contract,
  mock_governance_rejects_out_of_band_without_execution,
  mock_unknown_tensor_rejected, mock_i537b_holds_over_multi_launch_trail},
  metrics::tests.metrics_counters, tensor::tests.shape_numel_and_ids.

Cargo — clippy (all targets, -D warnings):
  cargo clippy -p arkhe-gpu --tests --all-targets -- -D warnings -> exit 0
  (corrigiu doc_lazy_continuation e bool_comparison em teste)

Cargo — workspace:
  cargo check --workspace -> BLOQUEADO por condicao PRE-EXISTENTE do arvore
  de trabalho, independente deste crate: (a) o Cargo.lock committado estava
  DESATUALIZADO (nao registra arkhe-ia-dtn nem hardy-bpv7 alias: o membro
  committado usa hardy-bpv7 = "0.6" cujo MSRV e 1.95, acima do rustc 1.94.0
  local); (b) com --ignore-rust-version, hardy-bpv7 0.6.0 NAO COMPILA no
  rustc 1.94.0 (E0283 — ambiguidade AsRef em aes-kw/hybrid-array). O crate
  deste bloco foi verificado isoladamente (`cargo check -p arkhe-gpu` exit 0,
  `cargo test -p arkhe-gpu` 17/17, `cargo clippy -p arkhe-gpu -D warnings`
  exit 0) e nao participa da falha (a compilacao parou em hardy-bpv7, antes
  de chegar a arkhe-gpu). NENHUMA reivindicacao de `cargo check --workspace`
  verde neste bloco (honestidade / precedente bloco 990).

Regressao dos crates de ancora (inalterados):
  arkhe-lean-bridge foi verificado de novo (compilado como dev-dep do FFI).
  arkhe-orchestrator-gate e arkhe-field-stability nao foram alterados.

Dependencias: zero externas novas no core. O Cargo.lock raiz ganha o membro
arkhe-gpu e a resolucao opcional da track Tile (cutile 0.2.0 + cuda-core/
cuda-async/cuda-bindings v0.2.0 e o restante da arvore do cutile — 25
pacotes, LOCKED, nao compilados: feature `tile` off por default). A re-
resolucao do lock NOVAMENTE expoe a staleness pre-existente do workspace
(arkhe-ia-dtn/hardy) — documentada, nao introduzida por este bloco.

## 7. Honestidade

- A prova Lean I536/I537/I538 e sobre **inteiros Nat** (contagens +
  geometria + duracao em ms). Nao ha realizacao f64 envolvida nas invariantes
  (diferente do gate I534/I535) — as provas sao EXATAS sobre o modelo do
  crate.
- I538-A prova o fecho aditivo Tile(5s)+retencao(k<=5s) dentro da capa SIMT
  (10s) — propriedade de retencao/merge; nao modela jitter de rede nem
  sobreposicao de kernels (fora do escopo, honesto).
- As tracks reais Tile (`cutile`) e SIMT (cuda-oxide) NAO compilam neste
  ambiente (GTX 1660 Ti sm_75 < sm_80; CUDA 12.1 < 13.3; Windows — nenhuma
  das duas e suportada nessas condicoes). O crate entrega o NUCLEO de
  governanca/auditoria testavel e a ponte honesta para a flywheel real:
  feature `tile` ativa `cutile` (early-stage, "neither is production-ready"
  — NVIDIA); `simt` fica VAZIA por honestidade (cuda-oxide nao e consumivel
  como dependencia Cargo: precisa cargo-oxide + nightly pinado + LLVM).
- A API lazy inventada no v521.0 (`backend.tensor([1024]).partition(...)
  .sync()`) NAO existe no cutile real: a API documentada e
  `#[cutile::module]` + `kernel::add(…).first().unpartition().to_host_vec()
  .sync_on(&stream)` — registada, nunca invocada neste ambiente.
- `cargo check --workspace` nao esta verde por causa de hardy-bpv7 0.6.0
  (MSRV 1.95 / E0283 no rustc 1.94) — condicao PRE-EXISTENTE da arvore de
  trabalho, sem relacao com arkhe-gpu.
- Nenhuma alteracao em crates de ancora nem no CoherenceLedger (Fase 4) nem
  nos nucleos I500–I535. Extensao ortogonal.

## 8. Hash real do bloco_1011 (reprodutibilidade)

- bloco_1011.json (UTF-8, LF): SHA-256 bruto
  `12ebb9b12d8d6c7938d2fec0b05aa84a321b52e62c7c2e3519d658a2e31b56c4`
- bloco_1011.json gzip (Optimal): SHA-256 comprimido
  `2e559c08f1bfe9bfbfc0ef5b278aaf48f9b7b0cbbf8b5eed977236676ef4b29e`