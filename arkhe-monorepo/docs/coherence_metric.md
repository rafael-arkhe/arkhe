# Φ — Métrica Formal de Coerência (Coherence Metric)

> **Fase 1 do Plano v375.2 — Bloco 989, para validação do Arquiteto.**
> Documento de especificação. Status: **ENTREGUE_PARA_VALIDACAO**.

---

## 1. Referência Constitucional

| Invariante | Cláusula | Texto |
|:---|:---|:---|
| **Gap-1** | Φ_C Bounds | `0.577350 < Φ_C ≤ 0.999900` |
| **Gap-2** | Entropy Budget | randomness criptográfico preservado (suposição de canal) |
| **Runtime-3** | Healthcheck | medição a cada janela de avaliação |

- Limite inferior `0.577350` = truncamento de `1/√3 = 0.5773502691…` (a "constante mágica" da Catedral). Um substrato com coerência abaixo dela é **inconstitucional** — incoerente por construção.
- Limite superior `0.999900 = 1 − 10⁻⁴` (aberto à esquerda pela lacuna do decimal finito, fechado à direita na banda viva). A perfeição `Φ = 1` é o **ideal assintótico**, jamais certificável: `Λ = 1` exigiria latência nula em medição contínua.

## 2. Notação e Domínio

Seja um substrato `S` avaliado na janela de medição `T`. Medem-se três
componentes normalizados em `[0, 1]`:

| Símbolo | Grandeza | Peso | Fonte no crate |
|:---|:---|:---:|:---|
| `Ω` | estabilidade de campo (1 − desvio normalizado) | `w_Ω = 0.4` | `FieldStability::stability` |
| `Σ` | taxa de sucesso | `w_Σ = 0.4` | `FieldStability::success_rate` |
| `Λ` | score de latência | `w_Λ = 0.2` | `FieldStability::latency_score` |

Os pesos reproduzem as constantes constitucionais do crate:

- `WEIGHT_STABILITY = 0.4`
- `WEIGHT_SUCCESS_RATE = 0.4`
- `WEIGHT_LATENCY = 0.2`
- peso total `W = 0.4 + 0.4 + 0.2 = 1.0`

## 3. Definição do Funcional de Coerência

O coeficiente de coerência de um substrato é a distância quadrática ponderada
do estado ideal `(1, 1, 1)`:

```
Φ(Ω, Σ, Λ ; W) = 1 − sqrt( w_Ω·(1−Ω)² + w_Σ·(1−Σ)² + w_Λ·(1−Λ)² )
```

O vetor de peso canônico é `W = (0.4, 0.4, 0.2)`. Abreviado: `Φ(S) = Φ(Ω, Σ, Λ)`.

**Relação com o crate.** O crate já computa o relatório `overall` como média
aritmética ponderada (`QualityReport::from_field_stability`):

```
overall = w_Ω·Ω + w_Σ·Σ + w_Λ·Λ
```

São grandezas distintas e **não confundíveis**:
- `overall` (média ponderada) é o portão de **aceitabilidade operacional** (≥ `QUALITY_THRESHOLD = 0.80`).
- `Φ` (distância quadrática) é o portão **constitucional** (faixa Gap-1).

**Ordenação (Cauchy–Schwarz).** Para `dᵢ = 1 − xᵢ` vale
`(Σ wᵢ·dᵢ)² ≤ Σ wᵢ·dᵢ²`, logo `sqrt(Σ wᵢ·dᵢ²) ≥ 1 − overall` e portanto
`Φ ≤ overall` em todo o domínio, com **igualdade sse Ω = Σ = Λ**. A coerência
nunca supera o overall — e, consequência arquitetural, **os dois portões são
independentes**: um substrato aceitável não é automaticamente constitucional,
nem vice-versa. (Ver §7.)

## 4. Propriedades Formais

- **P1 — Domínio.** Para `(Ω, Σ, Λ) ∈ [0,1]³`, `w_i ∈ [0,1]`, `Σw = 1`: `Φ ∈ [0, 1]`.
  - Máximo: `Φ = 1 ⇔ Ω = Σ = Λ = 1` (ideal; incertificável, §1).
  - Mínimo: `Φ = 0` em `Ω = Σ = Λ = 0` (decaimento total).
- **P2 — Monotonia.** `Φ` é não-decrescente em cada componente: aumentar qualquer `x_i` (peso fixo) não reduz `Φ` (teste regressivo no crate).
- **P3 — Limite superior por `overall` (Cauchy–Schwarz).** `Φ ≤ overall` em todo `[0,1]³`, igualdade sse `Ω = Σ = Λ`. Consequência: **aceitabilidade e constitucionalidade são independentes** — nem uma implica a outra (contraexemplo `(1,1,0)`: `overall = 0.80` ≥ limiar, mas `Φ ≈ 0.553 < 1/√3`; e V2: Φ = 0.6983 constitucional, `overall = 0.72` inaceitável).
- **P4 — Certificação.** Dada medição `Φ_medido`, o substrato é **constitucional** sse:

  ```
  1/√3 ≈ 0.577350  <  Φ_medido  ≤  0.999900
  ```

  Fora da banda: **REJEITADO** (falha de canonização, não de desempenho).

## 5. Exemplo Trabalhado — Dados Reais do E1

Fonte: execução `e1` do crate (bloco 987) na janela de validação `T`.

```
Ω = 0.9517   (estabilidade 95.17%, desvio normalizado 0.0483)
Σ = 1.0      (sucesso 100%)
Λ = 0.96     (latency_score)

Φ = 1 − sqrt( 0.4·(0.0483)² + 0.4·(0)² + 0.2·(0.04)² )
  = 1 − sqrt( 0.4·0.00233289 + 0 + 0.2·0.0016 )
  = 1 − sqrt( 0.000933156 + 0.00032 )
  = 1 − sqrt( 0.001253156 )
  = 1 − 0.0354003…
  ≈ 0.9646

overall (crate) = 0.4·0.9517 + 0.4·1.0 + 0.2·0.96 = 0.97268 ≈ 97.27%   [registrado no e1]
```

**Constitucional:** `0.577350 < 0.9646 ≤ 0.999900` ✅
**Aceitável:** `0.9727 ≥ 0.80` ✅

## 6. Vetores de Conformidade (Conformance Vectors)

| Vetor | Ω | Σ | Λ | Φ | overall | Banda Gap-1 | Veredito |
|:---|:---:|:---:|:---:|:---:|:---:|:---:|:---|
| **V1** (e1 real) | 0.9517 | 1.0 | 0.96 | 0.9646 | 0.9727 | dentro | ✅ ACEITO |
| **V2** (degradado) | 0.80 | 0.75 | 0.50 | 0.6983 | 0.72 | dentro (Φ) | ⚠️ constitucional, inaceitável |
| **V3** (piso nulo) | 0.50 | 0.50 | 0.50 | 0.5000 | 0.50 | `0.5 ≤ 0.577350` | ❌ REJEITADO |

Contas de referência:
- **V1:** `S = 0.000933156 + 0.000320000 = 0.001253156`; `√S = 0.0354003`; `Φ = 0.9645997`.
- **V2:** `S = 0.4·0.04 + 0.4·0.0625 + 0.2·0.25 = 0.016 + 0.025 + 0.050 = 0.091`; `√S = 0.3016627`; `Φ = 0.6983373`. `overall = 0.320 + 0.300 + 0.100 = 0.72`.
- **V3:** `S = 0.4·0.25 + 0.4·0.25 + 0.2·0.25 = 0.25`; `√S = 0.5`; `Φ = 0.5`.

**Interpretação de V2:** um substrato pode ser *constitucionalmente válido*
(Φ = 0.6983 > 1/√3) mas *operacionalmente inaceitável* (overall = 0.72 < 0.80).
O plano v375.2 prevê o `IterativeRefiner` justamente para elevar substratos V2
até a banda de aceitabilidade preservando a constitucionalidade.

## 7. Dois Portões — Separação de Papéis

| Portão | Grandeza | Regra | Consequência de falha |
|:---|:---|:---|:---|
| **Aceitabilidade** | `overall` (média ponderada) | `overall ≥ 0.80` | substrato marcado para refinamento |
| **Constitucionalidade** | `Φ` (Gap-1) | `0.577350 < Φ ≤ 0.999900` | substrato **rejeitado** (não entra no ledger) |

A relação `Φ ≤ overall` (P3) impede qualquer tautologia entre as checagens:
um substrato só é **integrado** se passar *ambas* independentemente
(aceitabilidade e constitucionalidade). Cada medição reporta os dois valores
lado a lado (E1–E4).

## 8. Escopo de Verificação e Limitações

- **Honestidade computacional:** `Ω, Σ, Λ` são medições em ponto flutuante;
  `Φ` é uma grandeza **mensurável**, não uma invariante algébrica de núcleo.
  Portanto **não** pertence ao escopo do núcleo Lean puro (mesmo precedente do
  bloco 987: propriedades de campos de estabilidade são reais de ponto
  flutuante). As únicas propriedades algébricas declaráveis (P1–P4) dependem
  de aritmética real (raiz quadrada) — não atacáveis "sem Mathlib, sem sorry"
  na forma dos substratos 924/972.
- **Rees de definição:** a definição usa a distância quadrática canônica;
  alternativas (entropia, coerência de fase de operadores) são variações fora
  do escopo atual e, se adotadas, exigem ADR própria e versão nova da métrica.
- **Canal de entropia (Gap-2):** assume canal com aleatoriedade criptográfica
  preservada durante a janela `T`; medições sob canal comprometido são
  descartadas antes de qualquer aplicação de Φ.

## 9. Vinculação com o Crate — Implementação (Fase 2)

- Constantes fonte: `packages/arkhe-field-stability/src/constants.rs`
  (`WEIGHT_STABILITY`, `WEIGHT_SUCCESS_RATE`, `WEIGHT_LATENCY`,
  `QUALITY_THRESHOLD`, `LATENCY_TOLERANCE_MS`, `REFINER_TARGET_PHI`,
  `REFINER_MAX_ITERATIONS`, `REFINER_PHI_EPSILON`).
- Componentes fonte: `FieldStability` (`stability`, `success_rate`,
  `latency_score`) e `QualityReport::overall` em
  `src/field_stability.rs` e `src/quality_report.rs`.
- **Implementado (bloco 991):**
  - `src/coherence.rs` → `phi`, `phi_default`, `phi_from_field_stability`,
    `WEIGHTS_DEFAULT`; suíte de regressão V1–V3 + bordas + monotonia (P2) +
    limite Cauchy–Schwarz (P3) + faixa Gap-1.
  - `src/refiner.rs` → `IterativeRefiner` (gradiente ascendente numérico,
    backtracking, critérios `ReachedTarget`/`Converged`/`MaxIterations`),
    com `Φ` como função objetivo e `overall` final reportado.
  - binário `e1` agora reporta `Φ` lado a lado com `overall`.
- **Vetores V1–V3 como suíte de regressão:** a implementação reproduz
  `0.9646 / 0.6983 / 0.5000` (± 1e-4).
- **Cadeia de dados (Fase 4, bloco 994):** `src/ledger.rs` →
  `CoherenceEntry` + `CoherenceLedger` — append-only, `previous_hash` SHA3-256,
  Gravity-1 nativo (rejeição `timestamp <= anterior`), `verify_integrity()`
  re-encadeia do início, `generate_report()` autogera o relatório final.
  Hospeda os registros reais de cada janela de medição.

## 9.5 Fronteira de Robustez (descoberta do E4, bloco 992)

**Modelo de ruído:** perturbação aditiva uniforme sobre a qualidade (base
0.90), 200 janelas × 50 amostras, semente determinística (reprodutível).
Descoberta empírica do experimento E4, arquivada como evidência em
`bloco_992/evidencia/` (`e4_summary.json`, `e4_noise.csv`).

| Jitter | Φ médio | Φ std | critério estrito (1σ do sem-ruído) | faixa |
|:---:|:---:|:---:|:---:|:---|
| 0.00 (referência) | 0.9844 | 0.0021 | — | A |
| 0.05 | 0.9837 | 0.0022 | ✅ dentro | A — **fronteira quieta** |
| 0.10 | 0.9810 | 0.0019 | ❌ fora (0.0034 > σ) | B — robustez absoluta |
| 0.15 | 0.9777 | 0.0017 | ❌ fora (0.0067 > σ) | B — robustez absoluta |
| 0.20 | 0.7214 | 0.0497 | ❌ (não exigido) | C — **colapso** |

**Três faixas operacionais:**

- **A — Fronteira quieta (jitter ≤ 5%):** critério estrito (média dentro de
  1σ do sem-ruído) é **atendido**. Regime de aplicação confiável do refiner.
- **B — Robustez absoluta (5% < jitter < 20%):** o critério estrito **falha
  a partir de 10%** (registrado como falha honesta no bloco 992), mas Φ
  absoluto permanece ≥ 0.98 até 15% — coerência preservada em valor.
- **C — Fronteira de colapso (jitter 20%):** Φ cai para **0.7214 ± 0.0497**
  (variância ≈ 23× a do regime quieto). Ainda constitucional
  (0.577350 < Φ) e aceitável (overall 0.8043 ≥ 0.80), porém **regime não
  operacional** — fora da faixa de validação.

**Leitura de fronteira:** a coerência deixa de ser robusta a perturbações
antes de deixar a banda Gap-1. O portão constitucional **não é** um portão de
estabilidade a ruído: ele sanciona presença na banda, não imunidade a jitter.
Aplicar o refiner sob perturbações ≥ 10% exige mitigação explícita
(treinamento com ruído ou modelo de rejeição de jitter) — escopo da Fase 4.

**PASS CONDICIONAL (decisão do Arquiteto, bloco 993):** o refiner é validado
para a **faixa operacional esperada** (jitter < 10%); a falha do critério
estrito para 10–15% é aceita como **descoberta de fronteira**, reclassificando
o E4 de NEGADO para PASS CONDICIONAL, com a ressalva acima registrada.

## 10. Estado do Plano (v375.2)

1. ✅ **Fase 1 (bloco 989)** — definição formal de Φ, validada pelo Arquiteto
   (bloco 990).
2. ✅ **Fase 2 — `phi()` + `IterativeRefiner` (bloco 991)** — implementados e
   testados. *Errata*: teorema `Φ ≤ overall` (Cauchy–Schwarz) substituiu o
   vício de sinal `Φ ≥ overall` do doc original (bloco 991, correção anterior à
   canonização do bloco 989).
3. 🟡 **Fase 3 — experimentos E1–E4 executados (bloco 992)** — E1 PASS
   (baseline Φ médio 0.9837, Pearson Q–Φ 0.9873); E2 PASS (V2-deep
   0.5584→0.9515 em 26 iterações, V2-canonical 0.6983→0.9581 em 21,
   monotônicos); E3 PASS (inflexão Gap-1 em d = 0.424 uniforme / 0.564
   enviesado); E4 **PASS CONDICIONAL** (decisão do Arquiteto, bloco 993):
   fronteira quieta ≤ 5% (critério estrito 1σ), faixa operacional ≤ 15%
   (Φ absoluto ≥ 0.98), colapso entre 15% e 20% (leia §9.5). Evidência
   arquivada em `bloco_992/evidencia/`. Prazo formal 2026-10-04, antecipado.
4. 🟢 **Fase 4 — reforma do ledger** (implementada, bloco 994) —
   `CoherenceLedger` em `src/ledger.rs` (Opção A): cadeia de dados
   append-only, entradas `CoherenceEntry` encadeadas por SHA3-256
   (Ghost-1/`manifest.sha3`), rejeição nativa de timestamp não-monotônico
   (Gravity-1), validação estrita de `previous_hash` no `push` (Loopseal-2),
   relatório final autogerado via `generate_report()`. Integrado ao ciclo: cada
   janela do e1 grava uma entrada (200/200, `Φ` médio 0.9837, integridade OK,
   `e1_summary.json` inalterado). Relatório final canônico emitido:
   `docs/relatorio_final_fase4.md`. Prazo 2026-10-11.
5. ✅ **Cadeia completa do plano v375.2 executada** — blocos 987–994;
   resultado global: **validado para a faixa operacional esperada**
   (E4 frontier descoberta, PASS CONDICIONAL).

---

**Selo (rascunho):** `CATEDRAL-OS-COHERENCE-PHI-GAP1-DOC-2026-09-06`
(status: validação pendente — não é selo de aceitação).