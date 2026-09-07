# 🏛️ BLOCO 991 — FASE 2: `phi()` + `IterativeRefiner` (com errata de teorema)

> **Arquiteto-Ω** — Catedral OS
> Handover 990 → 991 — Data: **2026-09-06** — **Fase 2 / Plano v375.2**
>
> Implementação das recomendações 1 e 2 do parecer (bloco 990): função `phi()`
> com suíte de regressão V1–V3 + bordas, e o `IterativeRefiner` com `Φ` como
> função objetivo. **Contém ERRATA IMPORTANTE**: o teste de regressão **pegou um
> teorema invertido no documento original** (`Φ ≥ overall` → o correto é
> `Φ ≤ overall`, Cauchy–Schwarz). Corrigido em `coherence.rs`, no
> `docs/coherence_metric.md` (§3, P3, §7) e na tabela do bloco 989 (ainda não
> canonizado). A recomendação 3 (I461/I462) permanece **deferida** por falta de
> substrato no monorepo.

---

## 📐 Registro no Ledger — BLOCO 991

```json
{
  "bloco": 991,
  "versao": "Fase 2 / Plano v375.2",
  "handover_anterior": 990,
  "data": "2026-09-06",
  "tipo": "IMPLEMENTACAO_PHI_REFINER",
  "descricao": "Implementacao da Fase 2: (1) src/coherence.rs com phi/phi_default/phi_from_field_stability/WEIGHTS_DEFAULT e suite de regressao V1-V3 + testes de borda + monotonia (P2) + limite Cauchy-Schwarz (P3) + faixa Gap-1; (2) src/refiner.rs IterativeRefiner (gradiente ascendente numerico com backtracking, criterios ReachedTarget/Converged/MaxIterations, alvo Phi>0.95, max 1000 iteracoes, epsilon 1e-6); (3) binario e1 reporta Phi lado a lado com overall. ERRATA: teorema Phi <= overall (Cauchy-Schwarz, igualdade sse Omega=Sigma=Lambda) corrigiu o vicio de sinal Phi >= overall do documento original.",
  "arquivos_gerados": [
    "src/coherence.rs",
    "src/refiner.rs",
    "src/lib.rs (mods + re-exports + docs)",
    "src/constants.rs (REFINER_TARGET_PHI, REFINER_MAX_ITERATIONS, REFINER_PHI_EPSILON)",
    "src/main.rs (saida Phi no binario e1)",
    "docs/coherence_metric.md (SS3, P3, SS7, SS9, SS10 corrigidos)"
  ],
  "testes": {
    "unit": 24,
    "wiremock": 3,
    "total": 27,
    "falhou": 0
  },
  "testes_por_modulo": {
    "coherence": 10,
    "refiner": 6,
    "field_stability": 5,
    "quality_report": 3,
    "mcp_stub_wiremock": 3
  },
  "errata": {
    "original": "Phi >= overall em todo o dominio ((1-x)^2 <= 1-x)",
    "corrigido": "Phi <= overall em todo o dominio (Cauchy-Schwarz: (Sum w*d)^2 <= Sum w*d^2, d=1-x; igualdade sse Omega=Sigma=Lambda)",
    "evidencia": "V1: Phi 0.9646 <= overall 0.9727 (dados do e1); contraexemplo (1,1,0): overall 0.80 mas Phi ~ 0.553 < 1/sqrt(3); teste test_phi_le_overall_in_domain",
    "locais": [
      "src/coherence.rs",
      "docs/coherence_metric.md SS3, P3, SS7, SS9, SS10",
      "bloco_989/README.md (tabela da auditoria) - bloco ainda nao canonizado, correcao registrada"
    ],
    "impacto_constitucional": "portoes de aceitabilidade e constitucionalidade sao INDEPENDENTES (nunca ordens derivaveis); bussola do refiner inalterada: objetivo Phi>0.95 continua valido"
  },
  "deferido": {
    "recomendacao_3": "integracao com malha de tensores I461 e SingletonDecoder I462 - sem substrato materializado no monorepo (grep: 0 ocorrencias) - aguarda evidencia"
  },
  "verificacao_deste_host": {
    "cargo_test_all_targets": "27/27 OK",
    "cargo_clippy_all_targets": "limpo, sem warnings",
    "cargo_build": "Finished dev profile",
    "bin_e1": "Estabilidade 95.17%, overall 97.27%, Phi (coerencia) 96.46% (Phi <= overall, teorema confirmado)"
  },
  "status": "ENTREGUE_PARA_VALIDACAO",
  "selo": "CATEDRAL-OS-FASE2-PHI-REFINER-2026-09-06"
}
```

---

## 🧭 O que foi implementado

### `src/coherence.rs` (recomendação 1)

- `phi(components: [f64;3], weights: [f64;3]) -> f64` — funcional fechado
  `Φ = 1 − sqrt(Σ wᵢ·(1−xᵢ)²)`.
- `phi_default` e `phi_from_field_stability` — pesos canônicos 0.4/0.4/0.2 e
  leitura direta de `FieldStability`.
- `WEIGHTS_DEFAULT` — `[WEIGHT_STABILITY, WEIGHT_SUCCESS_RATE, WEIGHT_LATENCY]`.
- Testes: V1 (0.9646), V2 (0.6983), V3 (0.5), bordas `(1,1,1)→1`, `(0,0,0)→0`,
  monotonia P2, limite P3 (`Φ ≤ overall`, Cauchy–Schwarz), faixa Gap-1,
  `phi_from_field_stability`.

### `src/refiner.rs` (recomendação 2)

- `IterativeRefiner` — gradiente ascendente numérico (diferença central,
  backtracking para garantir ascendência), componentes limitados a `[0,1]`.
- Critérios de parada exatos do parecer: `Φ > 0.95` (ReachedTarget),
  variação < 1e-6 (Converged), máximo 1000 iterações (MaxIterations).
- `RefinerResult` reporta `outcome`, `components`, `phi`, `overall`, `iterations`.
- Testes: de V2 chega ao alvo; V1 já no alvo (iteração 0); do piso (0,0,0)
  sobe; Converged com passo ínfimo; componentes em domínio; Φ não decresce.

### `e1` (binário)

- Passa a reportar `Φ (coerência)` ao lado de `overall` (Fase 3 reportará
  os dois valores lado a lado). Saída observada: `overall 97.27%`,
  `Φ 96.46%` — confirmando empiricamente `Φ ≤ overall`.

---

## ⚖️ PARECER DA AUDITORIA — Tabela de Realidade

| O que o parecer (990) pede | O que foi implementado | Veredito |
| :--- | :--- | :--- |
| `phi()` com regressão V1–V3 | `coherence.rs` reproduz 0.9646/0.6983/0.5000 (±1e-4) | ✅ Real |
| Testes de borda (1,1,1)→1, (0,0,0)→0, (0.5,0.5,0.5)→0.5 | presentes em `coherence.rs` | ✅ Real |
| `IterativeRefiner` otimiza Φ, alvo V1 a partir de V2 | `test_refiner_reaches_target_from_v2` (Φ > 0.95, < 1000 iterações) | ✅ Real |
| Critérios de parada: Φ>0.95 · 1000 iterações · Δ<1e-6 | `REFINER_*` em `constants.rs` + 3 outcomes | ✅ Real |
| Integração I461/I462 (malha de tensores / SingletonDecoder) | **sem substrato no monorepo** | ⚠️ Deferido |
| (doc original) `Φ ≥ overall` | **falso** — Cauchy–Schwarz: `Φ ≤ overall` | ❌→✅ **ERRATA** |

---

## 🐛 ERRATA DETALHADA (descoberta pelo teste de regressão)

O teste `test_phi_ge_overall_in_domain` **falhou na primeira execução** — e por
boa razão: a asserção estava falsa. A prova:

```
dᵢ = 1 − xᵢ ∈ [0,1];  (1−x)² ≤ 1−x  é verdadeira, MAS
sqrt é côncava... o passo errado foi achar que sqrt(Σ wᵢ·dᵢ²) ≤ Σ wᵢ·dᵢ.

Correto (Cauchy–Schwarz no produto interno ponderado):
  (Σ wᵢ·dᵢ)² ≤ (Σ wᵢ)(Σ wᵢ·dᵢ²) = Σ wᵢ·dᵢ²
  ⇒ sqrt(Σ wᵢ·dᵢ²) ≥ 1 − overall
  ⇒ Φ = 1 − sqrt(Σ wᵢ·dᵢ²) ≤ overall.
```

Consequência arquitetural: **aceitabilidade e constitucionalidade são
independentes** — não há ordenação derivável entre elas. Exemplos reais:
- V1 (e1): `Φ = 0.9646 ≤ overall = 0.9727` (teorema confirmado no binário).
- `(1, 1, 0)`: `overall = 0.80` (aceitável) mas `Φ ≈ 0.553 < 1/√3`
  (inconstitucional) — substrato aceitável que **não** passa o Gap-1.

O `IterativeRefiner` permanece correto (objetivo `Φ > 0.95`); sua bússola
não dependia do teorema invertido.

---

## 🏛️ PARECER FINAL

A Fase 2 foi implementada conforme as recomendações 1 e 2 do bloco 990
(27/27 testes, clippy limpo, `e1` executando). A recomendação 3 foi deferida
por inexistência de substrato I461/I462 no monorepo — registrada para quando
houver evidência. **Errata registrada**: o teorema de ordenação foi corrigido
para `Φ ≤ overall` (Cauchy–Schwarz), com impacto zero no objetivo de
refinamento e atualização do documento e do bloco 989 (pré-canonização).

```text
O teste que falha não é inimigo: é o termômetro da verdade.
O sinal trocado dobra a mentira em teorema —
o teste o desdobra, e a errata o exonera.
```

**Selo:** `CATEDRAL-OS-FASE2-PHI-REFINER-2026-09-06`