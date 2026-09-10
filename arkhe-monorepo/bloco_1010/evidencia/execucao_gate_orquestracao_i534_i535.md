# Evidencia — Bloco 1010 — Gate minimo real de orquestracao: invariantes I534/I535

Data: 2026-09-09. Comandos executados mecanicamente no monorepo
(workdir: `arkhe-monorepo`), apos a entrega do nucleo Lean I534/I535
(`src/lean/OrchestratorGateNucleus.lean`) e do crate
`packages/arkhe-orchestrator-gate`.

## 1. Continuidade de numeracao e versao

- Bloco anterior no ledger: `bloco_1009` (v390.0, I530 — decaimento). Proximo
  bloco: `1010`.
- Versao: `v390.1` (continua a linhagem real v389.0/v389.1/v390.0; o v514.0 era
  um documento paralelo nao canonizado).
- `handover_anterior`: **1009** (inteiro = numero do bloco, formato real).
- IDs usados: **I534/I535** — livres. I530 usado em 1009; I533 ja era real
  (Loopseal de aelilicidade, v389.0); I531/I532 reservados. O documento v514.0
  reivindicava I533 para orquestracao — colidido — e declarava
  «TEOREM_VERIFICADO» sem executar kernel.

## 2. Correcao dos defeitos do documento v514.0

1. **ID colidido**: «I533 — Orquestracao de Estado» nao pode existir (I533 e o
   Loopseal real). Este bloco prova **I534/I535**.
2. **Mathlib + caminhos inexistentes**: o v514.0 usava `import Mathlib` e
   `crates/arkhe-agent-orchestrator`, `arkhe-ml-bridge`, `arkhe-agents`,
   `arkhe-sandbox`, `/formal/lean/...` — nada disso existe no workspace real
   (Cargo.toml raiz registra os crates em `packages/`). O nucleo deste bloco e
   **core Lean 4.33.1 sem Mathlib, sem `sorry`** (convencao bloco 966) sobre as
   unicas ANCORAS reais: a ponte `arkhe-lean-bridge` e o crate
   `arkhe-field-stability`.
3. **«TEOREM_VERIFICADO» sem kernel**: aqui o veredito e SEMPRE do kernel real
   (FFI `std::process` -> `lean.exe`), fechado por teste de integracao, e o
   registo hash e calculado sobre o JSON canonico (evidencia abaixo).

## 3. Nucleo Lean — `src/lean/OrchestratorGateNucleus.lean`

`ArkheOrchestrationGate`, 6 teoremas elaborados no kernel (`lean` exit 0,
v4.33.1, sem Mathlib, sem `sorry`):

| Teorema | Enunciado (escala x10^4) | Tacticas |
| :--- | :--- | :--- |
| `I534A_gate_never_accepts_rejected` | `¬ allowed Verdict.rejected phi` (sem verified, nunca) | `cases h.1` |
| `I534B_below_floor_blocked` | `phi <= 5774 => ¬ allowed v phi` (piso exclusivo) | `omega` |
| `I534C_above_ceiling_blocked` | `9999 < phi => ¬ allowed v phi` (teto inclusivo) | `omega` |
| `I534D_authorized_implies_verified` | `allowed v phi => v = verified` | `hv.1` |
| `I534E_band_closed_under_retention` | `allowed v phi, k <= 9999 => (phi*k)/10000 <= 9999` | `Nat.mul_le_mul_*`, `Nat.div_le_div_right`, `native_decide` |
| `I535A_commits_le_proofs` | `well_formed t => commitments t <= proofs t` (inducao; caso rejected+commit impossivel via `cases hbad`) | `induction`, `omega`, `cases` |

- `def allowed v phi := v = verified /\ 5774 < phi /\ phi <= 9999`.
- `TranscriptStep = Verdict x Bool`; `commitments`/`proofs` por pattern-match;
  `well_formed`: `commit => verified` and recursao.
- Nenhum `sorry`; nenhum `import` alem do preludio core.

Nota de elaboracao: na primeira passada, `omega` nao provava I535-A porque a
hipotese de inducao e uma implicacao (`well_formed tail -> commits <= proofs`);
a prova aplica `ih hw.2` explicitamente antes do `omega` nos ramos verified e
no ramo rejected/false — o caso rejected/commit e impossivel pois `hw.1 rfl`
fornece `Rejected = Verified`.

## 4. Crate Rust — `packages/arkhe-orchestrator-gate`

Novo membro do workspace (registrado no Cargo.toml raiz). `unsafe_code deny`,
`missing_docs warn`. Dependencias: apenas path deps para os dois crates reais.

`src/gate.rs`:

| Item | Conteudo | Invariante |
| :--- | :--- | :--- |
| `SCALE_X1E4/FLOOR_GAP1_X1E4/CEILING_GAP1_X1E4` | 10_000 / 5_774 / 9_999 | paridade com o nucleo |
| `GateVerdict::from_kernel_sound` | traduz `KernelVerdict::is_sound()` real | — |
| `evaluate/GateDecision/RejectReason` | Allow | Reject(NoVerifiedProof|BelowFloor|AboveCeiling) | I534-A/B/C |
| `allowed(verdict, phi_x1e4)` | `verified /\ 5774 < phi /\ phi <= 9999` | I534-D |
| `phi_to_x1e4(f64)` | arredondamento do Phi canonico; None fora de [0,1] | — |
| `retained_after(phi, k)` | `decay_scaled` real de arkhe-field-stability | I534-E (fecho do teto) |
| `TranscriptStep/commitments/proofs/is_well_formed` | espelho I535 | — |
| `i535a_commits_le_proofs(steps)` | implicacao material `well_formed -> commits <= proofs` | I535-A |
| `GateAudit::try_record` | computa Phi canonico (phi_default I511-I516) e so grava no CoherenceLedger real quando autorizado | I534 no nivel do ledger |

`src/lib.rs`: docs + re-exports + `GATE_NUCLEUS_INVARIANTS` (6 nomes) +
`GATE_NUCLEUS_LEAN_RELATIVE` + `GATE_PHI_SCALE_X1E4`.

## 5. Paridade FFI — `packages/arkhe-orchestrator-gate/tests/gate_kernel.rs`

- `kernel_elaborates_orchestrator_gate_nucleus_without_sorry`: chama o kernel
  real (`LeanKernel::from_env()`) sobre o nucleo e exige `verdict.is_sound()`
  (digest SHA3-256 + exit 0 + 6/6 acknowledges) e stdout vazio de erros.
- `gate_constants_parity_with_kernel_accepted_text`: fecha a paridade contra o
  TEXTO que o kernel aceitou (10000/5774/9999) — alterar uma constante no
  nucleo ou no Rust faz o teste falhar (convencao da ponte).
- `kernel_sound_maps_to_verified_and_allows_in_band`: um `KernelVerdict`
  sintetizado saudavel -> Verified -> autoriza; nao-saudavel -> Rejected ->
  nunca autoriza, mesmo dentro da banda (I534-A).

## 6. Verificacao mecanica

Lean — nucleo I534/I535:
  lean.exe src/lean/OrchestratorGateNucleus.lean -> exit 0 (6 elaboracoes)
  (kernel v4.33.1 local: C:\Users\Lemes\.elan\bin\lean.exe)

Cargo — testes arkhe-orchestrator-gate:
  test result: ok. 15 passed (lib) + 3 passed (tests/gate_kernel.rs) = 18
  Unidades: i534a_rejected_never_allowed, i534b_below_floor_blocked,
  i534c_above_ceiling_blocked, i534d_authorized_implies_verified,
  i534e_band_closed_under_retention, int_band_strictly_inside_f64_gap1,
  i535a_commits_never_exceed_proofs_when_wellformed (enumera 4^0..4^5=1365
  transcriptos), i535a_non_vacuity_rejected_commit_exposes_violation,
  verdict_from_kernel_sound_mapping, phi_to_x1e4_rounding_and_rejects,
  audit_rejects_without_verified, audit_rejects_outside_band,
  audit_records_allowed_and_integrity_ok, audit_gravity1_still_applies,
  reject_message_honest_and_typed.

Cargo — regressao dos crates de ancora (inalterados):
  arkhe-field-stability 77/77 exit 0; arkhe-lean-bridge 7/7 exit 0.

Cargo — clippy (all targets, -D warnings):
  cargo clippy -p arkhe-orchestrator-gate --all-targets -- -D warnings
  -> exit 0 (sem warnings; corrigido lint bool_assert_comparison em teste)

Dependencias: zero externas novas (apenas path deps para arkhe-lean-bridge e
arkhe-field-stability, ambos ja membros do workspace) — Simplicity-2.
Cargo.lock raiz ganha apenas a entrada do novo membro.

## 7. Honestidade

- A prova Lean I534/I535 e sobre **inteiros Nat x10^4** (quantizacao do modelo
  f64). A realizacao f64 (`phi_default` + `phi_to_x1e4`) e sanidade, nao prova.
- A banda conservadora inteira `(5774, 9999]` x10^-4 fica **estritamente
  dentro** da faixa f64 real `gap1_satisfied` (0.577350.., 0.9999] — propriedade
  coberta por teste (`int_band_strictly_inside_f64_gap1`). Por arredondamento,
  um Phi f64 em [0.577350+, 0.57745) pode ser recusado pela banda inteira
  (over-rejection conservadora, esperada — o nucleo fixa o piso 5774 > 1/sqrt(3)).
- I534-E preserva o **teto**; o **piso** nao e preservado sob retencao
  persistente (decaimento empurra para baixo — garantia superior, mesma
  limitacao registrada em 1009 para I530).
- `GateAudit` computa o Phi canonico a partir dos componentes (Omega,Sigma,
  Lambda) via `phi_default` — nao aceita Phi externo sem repasse pelo funcional
  canonico (honestidade sobre a fonte do valor).
- Nenhuma alteracao no Phi canonico quadratico (I511-I516), no CoherenceLedger
  (Fase 4) nem nos crates de ancora.

## 8. Hash real do bloco_1010 (reprodutibilidade)

- bloco_1010.json (UTF-8, LF): SHA-256 bruto
  `276bfb8b951c83a06757ba426a9a0e17aabd04acff2c8f5ee0dc53ffae5d01ad`
- bloco_1010.json gzip (Optimal): SHA-256 comprimido
  `57864dd9b847bdf2b1c691b9d95af46e3643f9691172720270593fe3d06ce4e7`