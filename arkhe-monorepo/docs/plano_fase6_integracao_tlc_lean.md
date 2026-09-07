# Plano da Fase 6 — Integração TLC ↔ Núcleo Lean ↔ `verify_integrity()`

> Catedral OS — v376.1 → **Fase 6** (blocos 1001+)
> Autor: Arquiteto-Ω (revisão solicitada) — Data: **2026-09-06**
> Base sólida: spec TLA+ `ArkheCoherenceLedger` (bloco 1000, PASS TLC);
> núcleo Lean I511–I516 (bloco 997); `CoherenceLedger` Rust (bloco 994).

---

## 0. Objetivo e princípio

Integrar as **três camadas de evidência** — model-check TLC (finita),
núcleo Lean 4 (teoremas fechados) e `verify_integrity()` runtime (Rust) — de
modo que cada uma reforce as outras **sem inflar a reivindicação**:

> O TLC prova **alcançabilidade e liveness até `MaxWindows`** sobre um modelo
> abstrato. O Lean prova **símbolos e desigualdades** (I511–I516) no domínio
> da banda Gap-1. O Rust **verifica hashes SHA3-256 e encadeamento** em
> runtime. Nenhuma das três substitui as outras duas.

Toda a Fase 6 é **planejada antes de qualquer implementação** (este
documento). Nenhuma alteração de código ocorre sem este plano aprovado.

---

## 1. Achados de auditoria que a Fase 6 deve endereçar

| # | Achado | Local | Impacto |
| :-: | :--- | :--- | :--- |
| A1 | `entry.compute_hash() != entry.compute_hash()` é **tautologia** (nunca detecta) | `ledger.rs:173` | A checagem de "hash interno vs forma canônica" é vacua; só o elo `previous_hash` de sucessores pega adulteração de conteúdo (e a última entrada nunca é verificada por conteúdo). |
| A2 | `verify_integrity()` retorna `Vec<u64>` (ids quebrados) — sem status semântico | `ledger.rs:170` | Sem distinção `OK`/`UNKNOWN`; a reivindicação "TLC vs banda real" não é refletida na API. |
| A3 | Modelo TLA+ não tem contraparte executável no crate (`spec/` órfão) | `spec/ArkheCoherenceLedger.tla` | Nenhum caminho automático entre a spec, o Lean e o Rust. |

Decisão solicitada no §5: corrigir A1 no escopo da Fase 6 (recomendado,
defeito real e de baixo risco), ou apenas registrá-lo.

---

## 2. Trabalho 6.1 — Mapear invariantes TLC → Lean

Para cada invariante verificado no TLC, um teorema Lean correspondente
(sequência nova **I517–I522**, `native_decide`, mesmo padrão de I511–I516):

| TLC (bloco 1000) | Lean proposto | Forma |
| :--- | :--- | :--- |
| `TypeOK` | I517 | cadeia ∈ Seq de registros bem-tipados |
| `Gravity1Inv` | I518 | `i > 1 ⇒ ts[i] > ts[i−1]` |
| `Loopseal2Inv` | I519 | `prev[i] = hash[i−1]`, `prev[1] = GenesisHash` |
| `GenesisAnchoredInv` | I520 | `chain = <<>> ∨ window[1] = GenesisWindow` |
| `UniqueWindowsInv` | I521 | `i < j ⇒ window[i] < window[j]` |
| `HashTagsDistinctInv` | I522 | `i < j ⇒ hash[i] ≠ hash[j]` |
| `Liveness ∧ WF_vars(AppendAct) ∧ Quiesce` | I523 | sob a guarda de finitude, a cadeia alcança `Len ≥ MaxWindows` (lema de progresso no modelo finito) |

**Hipótese de trabalho explícita (pedida pelo Parecer):** os teoremas I517–
I523 são provados **para o modelo finito** (`Len(chain) < MaxWindows`); o
enunciado Lean declara o horizonte como hipótese, nunca como fato infinito.
Cada teorema terá docstring citando o invariante TLC e a redução assumida
(tags Nat, payload omitido, tick lógico).

---

## 3. Trabalho 6.2 — Especificação finita traduzida para Lean

Módulo novos (`src/lean/nuclei/nuclei/FieldStabilityTLCSpec.lean`, pacote Lake
`src/lean/nuclei` — layout canônico `lake init`: root `nuclei.lean` + lib
aninhada; alvo `lake build` exit 0), sem Mathlib, sem `sorry`, espelhando
`ArkheCoherenceLedger.tla`:

- `Entry = { window, ts, phi, prev, hash }` sobre `Nat` (tags abstratas).
- `Init`, `AppendEntry` (com guarda `Len < MaxWindows`), `Quiesce`,
  `Spec = Init ∧ [][Next]_vars ∧ WF_vars(AppendAct)` — **como funções de
  traço** (lemas de preservação de passo, não semântica temporal completa).
- Relação de simulação estrutural com o `CoherenceEntry` real:
  `prev`/`previous_hash`, `ts`/`timestamp`, `window`/`window_id`.

**Limite honesto registrado:** o Lean formaliza o modelo finito; equivalência
total entre Lean e TLC é **futuro** (Proj. Apalache, §5/6.5). Esta camada dá
ao crate um artefato Lean executável por `lake build`, fechando A3.

---

## 4. Trabalho 6.3 — `verify_integrity()` com status semântico

- Corrigir A1 (se aprovado no §5): checar `entry.compute_hash()` contra o
  elo esperado re-derivado (o hash de cada entrada deve ser consistente com
  sua forma canônica **e** o `previous_hash` do sucessor).
- Introduzir enum de status:
  ```rust
  pub enum IntegrityStatus {
      Ok,                       // cadeia íntegra dentro do horizonte provado
      Broken { window_ids: Vec<u64> },
      BeyondHorizon { len: usize, max_windows: usize }, // além do provado pelo TLC
  }
  ```
  `verify_integrity()` passa a retornar `IntegrityStatus`:
  - `len ≤ MaxWindows` → `Ok` ou `Broken` (evidência TLC + hash).
  - `len > MaxWindows` → `BeyondHorizon` se íntegra (não é fraude — é
    **honestidade**: a prova finita cobre até `MaxWindows`); o checksum
    contínua validando integralidade real.
- Regressão: manter os 44/44 testes; adicionar testes para `BeyondHorizon`
  e para o caso de conteúdo adulterado da **última** entrada (A1).

---

## 5. Trabalho 6.4 — Integração contínua (job em CI)

Um job único que **bloqueia** se qualquer camada falhar:

1. `cargo test` (crate `arkhe-field-stability`) — `verify_integrity()`.
2. `lake build` no pacote `src/lean/nuclei` (núcleo Lean I511–I523 + `FieldStabilityTLCSpec`).
3. `java -jar tla2tools.jar -config ArkheCoherenceLedger.cfg ArkheCoherenceLedger.tla`
   — TLC **pinned** `v1.7.4` (sha1 `bee4a54f3ee3d4afc347c3240ec2d9e93b075104`),
   com o run final registrado em `bloco_1000/evidencia/`.

Saída exigida do job: quebra de build se "No error has been found" não
aparecer no log do TLC, ou se `cargo test`/`lake build` falhar. O job aponta
para os artefatos de evidência dos blocos 997/1000 (checksums).

---

## 6. Trabalho 6.5 — Apalache (trabalho futuro, preparo apenas)

- Preparar a instância symbolic do mesmo modelo (sem exaustão) para a banda
  real (phi x10⁴, dezenas de janelas) — **não é escopo de entrega da Fase 6**:
  apenas deixa-se a spec compatível (CONSTANTS já parametrizadas) e registra-se
  o limite no relatório. Execução fica condicionada à disponibilidade de
  tooling e ao Plano v377+.

---

## 7. Critérios de aceite da Fase 6

1. `src/lean/nuclei/nuclei/FieldStabilityTLCSpec.lean` + I517–I523: kernel Lean 4 v4.33.1,
   sem Mathlib, sem `sorry`, `lake build` exit 0.
2. `IntegrityStatus` implementado; testes novos passando; 44/44 antigos.
3. Job de CI integrando as 3 camadas, bloqueando em falha.
4. Relatório v377.0 atualizado com §11 (limite honesto TLC↔Lean↔Rust) e o
   texto padrão do §0.
5. Correção A1 (tautologia) com o teste de adulteração de conteúdo da última
   entrada — **falhando antes da correção, passando depois** (D1 aprovada).

---

## 8. Decisões D1–D4 — texto integral

> Parecer do Arquiteto: `ARKHE-PARECER-FASE6-2026-09-06` —
> **D1 APROVADA**; D2/D3/D4 aguardando este esclarecimento para selo.

### D1 — Corrigir a tautologia A1 em `ledger.rs:173`

**Status: ✅ APROVADA** (incondicional; ver §9 selo).

**Proposta completa:** em `verify_integrity()`, substituir a checagem vazia
`entry.compute_hash() != entry.compute_hash()` por uma verificação efetiva:
o hash canônico de cada entrada deve ser **consistente com o elo derivado**
(forma canônica re-serializada determinística) e, a cada posição i>1, o elo
`entry[i].previous_hash == compute_hash(entry[i−1])` continua sendo validado.
Garante-se que **adulteração de conteúdo de qualquer entrada** (inclusive a
última, que não tem sucessor para delatá-la) é detectada.

**Teste obrigatório (exigido pelo parecer):** cenário em 4 passos —
(1) ledger com 2 entradas via `push()`; (2) `verify_integrity()` → `Ok`;
(3) mutar o conteúdo da **última** entrada (payload/latência etc.) **fora do
fluxo normal** (acesso direto via `entries_mut`, bypass de `push`);
(4) `verify_integrity()` → `Broken`. **Critério de não-cosmética:** o teste
deve **falhar** no código anterior à correção e **passar** após — o teste
antigo aprovava; o novo reprova o bug.

**Regressão:** os 44/44 testes existentes permanecem verdes (a correção não
altera contratos públicos além do comportamento defeituoso).

### D2 — Nomenclatura e padrão dos teoremas Lean

**Status: recomendada — aguardando selo.**

**Proposta completa:** sequência **I517–I523**, prosseguindo o núcleo
I511–I516 (bloco 997). Cada teorema no padrão vigente:
`native_decide`, sem Mathlib, sem `sorry`, kernel Lean 4 v4.33.1 (elan
`C:\Users\Lemes\.elan\bin\lean.exe`); docstring com (a) o invariante TLC
fonte (bloco 1000), (b) a redução assumida (tags Nat, payload Ω/Σ/Λ fora,
tick lógico), (c) a **hipótese de finitude** no enunciado
(`Len(chain) < MaxWindows`), nunca como fato infinito. Distribuição:
I517 TypeOK, I518 Gravity-1, I519 Loopseal-2, I520 GenesisAnchored,
I521 UniqueWindows, I522 HashTagsDistinct, I523 Liveness (progresso no
modelo finito sob `WF_vars(AppendAct)` + `Quiesce`).

### D3 — `IntegrityStatus::BeyondHorizon` como status definitivo

**Status: recomendada — aguardando selo.**

**Proposta completa:** `verify_integrity()` (mantendo a varredura integral
por SHA3-256) passa a retornar:

```rust
pub enum IntegrityStatus {
    Ok,                                     // íntegro dentro do horizonte provado
    Broken { window_ids: Vec<u64> },        // violação real de integridade
    BeyondHorizon { len: usize, max_windows: usize }, // íntegro, além da prova finita
}
```

Semântica: `BeyondHorizon` **não é erro nem fraude** — a integralidade real
(hashes) foi verificada; apenas a **reivindicação de prova formal** é restrita
até `MaxWindows`. API é uma extensão sem quebra (novo enum substitui o retorno
`Vec<u64>`; chamadores internos e `generate_report()` ajustados; nenhum
contrato externo rompido). Testes novos: cadeia de 5+ entradas com
`MaxWindows=4` → `BeyondHorizon`.

### D4 — Registro em blocos da Fase 6

**Status: esclarecida — aguardando parecer.**

**Proposta completa:** registrar **plano e execução em blocos separados**
(rastreabilidade e auditoria independentes, mesmo padrão 999/1000):

- **`bloco_1001`** — registro do **plano aprovado** (este documento +
  decisões seladas; handover 1000 → 1001): README com JSON do bloco,
  `evidencia/` com `plano_fase6_integracao_tlc_lean.md` + SHA256SUMS,
  JSON validado. Conteúdo: decisões D1–D4 e escopo; **nenhum código**.
- **`bloco_1002`** — registro da **execução da Fase 6**: evidência mecânica
  (`FieldStabilityTLCSpec.lean`, testes A1, `IntegrityStatus`, job CI),
  SHA256SUMS, JSON validado, selo de conclusão. Nenhuma mistura de "plano"
  com "execução".

Cadeia no ledger: `… → bloco_1000 → bloco_1001 → bloco_1002`.

---

**Selo pendente:** a definir no `bloco_1001` mediante aprovação deste plano
(D2, D3, D4).

---

## 9. Status de aprovação (registro de parecer)

| Decisão | Parecer | Selo |
| :-: | :--- | :--- |
| D1 (corrigir tautologia) | ✅ **APROVADA** — com teste explícito de adulteração da última entrada | `ARKHE-PARECER-FASE6-2026-09-06` |
| D2 (I517–I523) | 🟡 recomendada — aguardando selo | — |
| D3 (`BeyondHorizon`) | 🟡 recomendada — aguardando selo | — |
| D4 (blocos 1001/1002) | 🟡 esclarecida — aguardando parecer | — |

**Condições release do `bloco_1001`** (parecer): registro apenas após (1)
esclarecimento/parecer de D4; (2) aprovação formal de D1 (✅) e, se aplicável,
D2/D3; (3) plano atualizado refletindo as decisões seladas. Nenhuma alteração
de código da Fase 6 antes disso.