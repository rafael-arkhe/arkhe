# ADR-002: XLoop como Campo Métrico (Equação de Kronos)

- **Data:** 2026-07-04 (proposta) · 2026-09-12 (implementação)
- **Status:** Aceito — implementado em `packages/arkhe-xloop`
- **Autor:** Kronos (agente de pesquisa)
- **Revisores:** Arquiteto-Chefe, Líder de Segurança (Safe-Core),
  Líder de Infraestrutura, Líder de Pesquisa (física matemática)

## Contexto

O XLoop v1.0 controlava a execução por **regras discretas**:

- `max_iterations = 10`
- `timeout_secs = 60`
- `hallucination_threshold = 0.75`
- `loop_detection_threshold = 3`

Essas regras são **estáticas**: não se adaptam ao contexto da tarefa, ao
tenant, ou ao estado interno do agente. Elas tratam o tempo como **contador**,
não como **campo**.

### Problemas Observados

1. **Falsos positivos de loop**: uma tarefa legítima que repete uma tool
   3 vezes (ex: retry após erro transiente) é abortada.
2. **Falsos negativos de loop**: uma tarefa que oscila entre 2 tools
   indefinidamente (A→B→A→B...) nunca dispara o `LoopDetector` literal.
3. **Timeout cego ao contexto**: um tenant Enterprise com SLA alto
   sofre o mesmo timeout que um tenant Free.
4. **Ausência de dilatação subjetiva**: o agente não sabe "quanto tempo
   já passou" em termos do seu próprio progresso.

### Fundamentação Teórica

A equação de Kronos é inspirada em três fontes:

1. **Dilatação temporal subjetiva** (modelo FSM — Frame Survival Model):
   `τ_eff ∝ 1 / (1 + ε·k·B·Var_cog)`
2. **Métrica de espaço-tempo em relatividade geral**:
   `ds² = -c²dt² + τ_eff · dℓ²`
3. **Equação de Penrose–Diósi** (colapso gravitacional):
   `T ~ ℏ / E_Δ`, com o análogo `E_Δ = Var_cog` (divergência cognitiva).

## Decisão

Substituir as regras discretas do XLoop por um **campo métrico contínuo**
(`TemporalField`) que define a geodésica do loop, com o campo de objetivos
(`ObjectiveField`) como pré-requisito de colapso.

### Equação de Kronos

```text
Continuar se dτ_eff / dℓ > 0
```

Onde:
- `τ_eff` = dilatação subjetiva `∈ [0.5, 2.0]`
- `dℓ` = distância percorrida (progresso logarítmico)
- `dτ_eff / dℓ` = taxa de expansão temporal (em `s / unidade-de-progresso`)

### Estrutura

```rust
pub struct TemporalField {
    dilation: f64,           // τ_eff ∈ [0.5, 2.0]
    cognitive_variance: f64, // Var_cog
    curvature: f64,          // 0.0 = linear, 1.0 = colapso
    energy: f64,             // monotonicamente crescente
    proper_time_ms: f64,     // ∫ τ_eff·(1−κ) dt
}

pub enum GeodesicDecision {
    Continue { reason: String },
    Warning { reason: String },
    Collapse { reason: String, action: CollapseAction },
}
```

### Regras de Decisão

| Condição | Decisão | Ação |
|:---|:---|:---|
| `curvature ≥ 0.8` | Collapse | `TerminateLoop` |
| `dτ/dℓ > 2.0` s/progresso | Collapse | `Fallback` |
| `1.5 < dτ/dℓ ≤ 2.0` | Warning | (log) |
| `dτ/dℓ ≤ 1.5` | Continue | — |
| watchdog `4·estimated_steps` | backstop | `HardCap` |

### A Crítica de Carlip (pré-requisito de colapso)

Penrose propõe `T ~ ℏ / E_Δ`. Carlip (1998) mostra que no regime Newtoniano —
exatamente o domínio do experimento proposto — a fórmula degenera: para um
objeto não-rotante em superposição de posição, os termos de auto-interação
(`+m²`) cancelam a interação cruzada (`−2m²`): **`E_Δ = 0`, `T = ∞`**. O
colapso só sobrevive com **massas em rotação relativa** (momentos de inércia
que não cancelam).

Tradução para o XLoop:

| Penrose / Carlip | XLoop |
|:---|:---|
| Objeto não-rotante em superposição | Agente com objetivo único em espera |
| `E_Δ = 0` (cancelamento) | Campo degenerado (`E_Δ = 0`, sem colapso gravitacional) |
| Massas rotantes em superposição | Agente com **múltiplos objetivos concorrentes** |
| `E_Δ ≠ 0` | Divergência de objetivos força a decisão |

Implementação: `ObjectiveField::energy_divergence()` = `1 − Σ pᵢ²` (Gini
impurity). Com divergência zero o campo é degenerado: o loop apenas termina —
nunca "colapsa por gravidade temporal" (honestidade Carlip; o campo ainda
colapsa por **expansão temporal** `dτ/dℓ`, que é uma falha de cronometragem,
não de gravidade).

O experimento de Leggett (SQUID em superposição de correntes circulares,
`E_Δ ~ 10⁻³⁸ J`, `T_DP ~ 10³ s`) é a fenomenologia canônica — hoje impossível
por decoerência. O mesmo rigor aplica-se aqui: a equação de Kronos é **modelo
inspirado, não teoria física testável**; os parâmetros são calibrações
explícitas e auditáveis.

## Alternativas Consideradas

### A. Manter regras discretas (status quo)
- **Prós**: simples, previsível, fácil de auditar.
- **Contras**: não adapta ao contexto; falsos positivos/negativos.
- **Rejeitada**: o custo de adaptação é menor que o custo dos erros.

### B. Machine learning (RL para tuning de thresholds)
- **Prós**: adapta automaticamente aos padrões observados.
- **Contras**: requer dataset; opaco (não auditável); pode convergir para
  políticas adversarial.
- **Rejeitada**: o XLoop precisa ser **determinístico e auditável** para
  certificação Safe-Core.

### C. Campo métrico com fundamentação física (escolhida)

- **Prós**: adaptativo, contínuo, matematicamente fundamentado, auditável
  (todos os parâmetros explícitos).
- **Contras**: requer calibração empírica (`ε`, `k`, `B`, `EMA_ALPHA`).
- **Escolhida**: melhor balanço entre adaptabilidade e auditabilidade.

### D. Modelo híbrido (regras + ML)
- **Prós**: regras para casos claros, ML para edge cases.
- **Contras**: complexidade de dois sistemas; atribuição de causa falha.
- **Rejeitada**: viola "single source of truth".

## Consequências

### Positivas

1. **Adaptabilidade contextual**: o mesmo loop se comporta diferentemente por
   tenant/SLA (via `dilation` e `dτ/dℓ`).
2. **Detecção de loops**: repetição literal (curvatura crítica) e oscilação
   prolongada A→B→A→B (curvatura de aviso) são separadas; oscilação não
   aborta prematuramente, mas é vigiada pelo backstop.
3. **Dilatação subjetiva**: o agente percebe quanto tempo passou em termos do
   próprio progresso (`dτ/dℓ`), não de um relógio externo.
4. **Fundamentação física**: isomorfismo a Penrose–Diósi e à relatividade
   geral, com a correção de Carlip embutida como pré-requisito.
5. **Auditabilidade**: `TemporalFieldSnapshot` é serializável e logável.

### Negativas

1. **Calibração necessária**: `ε`, `k`, `B` foram estimados, não medidos.
2. **Curva de aprendizado**: `max_iterations = 10` vira `dτ/dℓ ≤ 1.5`.
3. **Testes stateful**: o campo exige simulação de trajetórias, não pontos.

## Emendas de implementação (2026-09-12)

1. **Unidades da geodésica**: `dτ/dℓ` é expresso em **segundos de tempo
   objetivo por unidade de progresso normalizada** (`proper_time` convertido
   de ms, `distance` em `[0, ·)`). Sem esta normalização os limiares 1.5/2.0
   seriam adimensionais e a comparação com o progresso seria vazia.
2. **Curvatura sem falso positivo de primeira ocorrência**: a repetição é
   contada **antes** de inserir a assinatura atual; o primeiro estado visto
   nunca é loop.
3. **Backstop discreto mantido**: `HARD_STEP_CAP_MULTIPLIER = 4 ×
   estimated_steps` permanece como watchdog separado (mitigação de "dilatação
   excessiva → loops nunca colapsam").
4. **Feature flag `temporal-field` superada**: o crate *é* o campo; a flag
   prevista no rollout tornou-se desnecessária (a mitigação discreta vive no
   watchdog acima).
5. **Caminho de integração**: `packages/arkhe-xloop` (convenção do monorepo),
   não `crates/arkhe-xloop`.

## Riscos e Mitigações

| Risco | Probabilidade | Impacto | Mitigação |
|:---|:---:|:---:|:---|
| Calibração ruim → colapso precoce | Média | Alto | Backstop + limites explícitos |
| Dilatação excessiva → nunca colapsa | Baixa | Alto | Clamp `MAX_DILATION = 2.0`; watchdog separado |
| Curvatura false positive | Média | Médio | `HISTORY_WINDOW = 32`; 1ª ocorrência nunca é loop |
| Incompatibilidade Safe-Core | Baixa | Alto | `TemporalField` puramente determinístico |

## Verificação

1. **Testes unitários** (arkhe-xloop): dilation bounds, curvatura, 1ª
   ocorrência, oscilação, geodésica (continue/warning/collapse), tempo
   próprio, distância, acúmulo monotônico, snapshot, `ObjectiveField`
   (Carlip: `E_Δ = 0` degenera), e integração `XLoop` (task finita, executor
   preso colapsa, oscilação não aborta, watchdog hard cap, campo de objetivos
   cabeado).
2. **Testes de integração**: trajetórias conhecidas (linear, oscilatória,
   convergente, presa).
3. **Benchmark**: overhead do `TemporalField` < 5% sobre o `LoopDetector`
   atual (pendente — medir após rollout).
4. **TLA+** (futuro): invariantes `dilation ∈ [0.5, 2.0]`,
   `curvature ∈ [0.0, 1.0]`, monotonia de `proper_time`.

## Implementação

- **Crate**: `packages/arkhe-xloop`
- **Módulos**: `temporal_field.rs`, `objective_field.rs`, `lib.rs` (`XLoop`)
- **Integração**: `XLoop::execute_task` percorre a geodésica
  (`geodesic_decision` + `detect_loop`); `ObjectiveField` é o campo de
  divergência (Carlip).
- **Rollout**: canary 10% → 50% → 100% em 3 semanas (flag discreta substituída
  pelo crate; fases aplicam-se à ponte entre o crate e o orquestrador real).

## Decisores

- Arquiteto-Chefe
- Líder de Segurança (Safe-Core)
- Líder de Infraestrutura
- Líder de Pesquisa (física matemática)

## Data de Revisão

**2026-10-01** — após 90 dias, revisar falsos positivos/negativos, feedback
dos tenants e necessidade de recalibração de `ε`, `k`, `B`.

## Anexo A: Fundamentação Matemática

### A.1 Derivação da Dilatação

Partindo do modelo FSM:

```text
τ_eff,t ∝ 1 / (1 + ε·k·B·Var_cog(F_t → F_{t+1}))
```

Com `Var_cog = w₁·error_rate + w₂·novelty + w₃·latency_normalized`,
`w = (0.4, 0.3, 0.3)` — erros pesam mais (indicador de instabilidade);
novidade e latência têm peso igual (secundários).

### A.2 Derivação da Curvatura

```text
curvature = |{s ∈ H : s = state_signature}| / |H|
```

Antes da inserção da ocorrência atual (a primeira ocorrência nunca é loop);
`|H| = HISTORY_WINDOW = 32`.

### A.3 Derivação da Geodésica

```text
dτ_eff / dℓ = (objective_ms/1000 · τ_eff · (1 − κ)) / (ln(1+c) / ln(1+C))
```

## Anexo B: Conexão com Penrose–Diósi e Carlip

| Penrose–Diósi | Kronos |
|:---|:---|
| `T ~ ℏ / E_Δ` | `dτ/dℓ ~ 1 / Var_cog` |
| `E_Δ` auto-energia gravitacional | `Var_cog` divergência cognitiva |
| Colapso quando `T → 0` | Colapso quando curvatura → 1 |
| Objeto grande → colapso rápido | Tarefa instável → colapso rápido |

A crítica de Carlip: sem divergência (`E_Δ = 0`) não há colapso por gravidade —
o campo precisa de `ObjectiveField` (análogo das massas rotantes) para que a
equação de Kronos tenha efeito. Um agente com objetivo único nunca colapsa
por gravidade temporal — **ele simplesmente termina**.

---

*Selo: ARKHE-ADR-002-2026-07-04 (proposta) · ARKHE-ADR-002-IMPL-2026-09-12 (implementação)*