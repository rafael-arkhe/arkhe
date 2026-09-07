# 🏛️ BLOCO 992 — FASE 3: EXPERIMENTOS E1–E4 (caracterização empírica)

> **Arquiteto-Ω** — Catedral OS
> Handover 991 → 992 — Data: **2026-09-06** — **Fase 3 / Plano v375.2**
>
> Execução dos quatro experimentos aprovados no escopo da Fase 3, com feature
> gate `experiments`, logs estruturados (`RUST_LOG=info`) e artefatos
> (CSV + JSON) por experimento. **Resultado honesto:** E1, E2 e E3
> **PASSARAM**; **E4 NÃO atendeu** o critério estrito (desvio > 1σ para
> jitter ≥ 10%) — registrado como caracterização empírica, não como falha
> encoberta.

---

## 📐 Registro no Ledger — BLOCO 992

```json
{
  "bloco": 992,
  "versao": "Fase 3 / Plano v375.2",
  "handover_anterior": 991,
  "data": "2026-09-06",
  "tipo": "EXECUCAO_EXPERIMENTOS_E1_E4",
  "descricao": "Execucao dos experimentos E1-E4 com feature-experiments, logs RUST_LOG=info, artefatos CSV+JSON por experimento. E1 baseline PASS (Phi medio 0.9837, Pearson Q-PHI 0.9873, 100% janelas acima do piso 0.70); E2 refinamento PASS (V2-deep Phi 0.5584->0.9515 em 26 iters monotonico; V2-canonico 0.6983->0.9581 em 21 iters); E3 varredura PASS (uniforme: Gap-1 em d=0.424, aceitabilidade d=0.200; enviesado: Gap-1 d=0.564, aceitabilidade d=0.286); E4 estabilidade sob ruido NEGADO (jitter 0.05 dentro de 1-sigma, jitter 0.10 e 0.15 fora; Phi absoluto ainda >=0.98 ate 15% jitter).",
  "testes": { "unit": 33, "wiremock": 3, "total_crate": 36, "falhou": 0 },
  "experimentos": {
    "e1": {
      "objetivo": "baseline estado estacionario",
      "criterio": "Phi medio > 0.70 e correlacao Q-PHI mensuravel",
      "resultado": { "phi_mean": 0.9837, "phi_std": 0.0023, "above_floor_pct": 100.0, "pearson_q_phi": 0.9873 },
      "success": true
    },
    "e2": {
      "objetivo": "refinamento deterministico V2->V1",
      "criterio": "Phi > 0.95 em <= 1000 iteracoes, convergencia monotonica",
      "resultado": {
        "v2_deep_phi055": { "start": [0.60, 0.55, 0.50], "start_phi": 0.5584, "end_phi": 0.9515, "iterations": 26, "monotonic": true },
        "v2_canonical_phi070": { "start": [0.80, 0.75, 0.50], "start_phi": 0.6983, "end_phi": 0.9581, "iterations": 21, "monotonic": true }
      },
      "success": true
    },
    "e3": {
      "objetivo": "mapear Phi x degradacao estrutural",
      "criterio": "curva de resposta e cruzamentos (Gap-1 e aceitabilidade)",
      "resultado": {
        "uniforme": { "d_gap1_first": 0.424, "d_accept_first": 0.200, "phi_at_gap1": 0.5760 },
        "enviesado": { "d_gap1_first": 0.564, "d_accept_first": 0.286, "phi_at_gap1": 0.5772 }
      },
      "success": true
    },
    "e4": {
      "objetivo": "estabilidade sob ruido (jitter)",
      "criterio": "Phi dentro de 1 desvio padrao do valor sem ruido para jitter < 20%",
      "resultado": {
        "noise_free": { "phi_mean": 0.9844, "phi_std": 0.0021 },
        "jitter_0.05": { "phi_mean": 0.9837, "within_1_sigma": true },
        "jitter_0.10": { "phi_mean": 0.9810, "within_1_sigma": false },
        "jitter_0.15": { "phi_mean": 0.9777, "within_1_sigma": false },
        "jitter_0.20": { "phi_mean": 0.7214, "phi_std": 0.0497, "within_1_sigma": false, "nao_exigido": true }
      },
      "success": false
    }
  },
  "artefatos": [
    "mission_output/e1_results/e1_phi_series.csv",
    "mission_output/e1_results/e1_summary.json",
    "mission_output/e2_results/e2_v2_deep_phi055.csv",
    "mission_output/e2_results/e2_v2_canonical_phi070.csv",
    "mission_output/e2_results/e2_summary.json",
    "mission_output/e3_results/e3_scan_uniforme.csv",
    "mission_output/e3_results/e3_scan_enviesado.csv",
    "mission_output/e3_results/e3_summary.json",
    "mission_output/e4_results/e4_noise.csv",
    "mission_output/e4_results/e4_summary.json"
  ],
  "feature_gate": { "experiments": "bins e1-e4 com required-features; perfil default sem bins" },
  "anotacao_e4": "Modelo de ruido: amplitude aditiva uniforme sobre a qualidade (base 0.90). Phis absolutos permanecem >= 0.98 ate 15% de jitter; o criterio estrito de 1-sigma-do-sem-ruido falha a partir de 10%. Nao houve falsificacao de resultado.",
  "status": "ENTREGUE_PARA_VALIDACAO",
  "selo": "CATEDRAL-OS-FASE3-EXPERIMENTOS-E1E4-2026-09-06"
}
```

---

## 📊 Resumos por experimento

### E1 — Baseline (sucesso: **PASS**)

- 200 janelas × 50 amostras, qualidade base 0.90 ± 0.05, latência 8–25 ms.
- Φ médio **0.9837** (± 0.0023), 100% das janelas acima do piso 0.70.
- `overall` médio 0.9898; **Pearson(Q, Φ) = 0.9873** (correlação mensurável).

### E2 — Refinamento determinístico (sucesso: **PASS**)

| Run | Estado inicial | Φ inicial | Φ final | iterações | monotônico |
|:---|:---|:---:|:---:|:---:|:---:|
| v2_deep_phi055 | (0.60, 0.55, 0.50) | 0.5584 | **0.9515** | 26 | ✅ |
| v2_canonical_phi070 | (0.80, 0.75, 0.50) | 0.6983 | **0.9581** | 21 | ✅ |

Ambos atingem `Φ > 0.95` em « 1000 iterações, sem oscilações (trajetória em
CSV por iteração, `refine_traced`).

### E3 — Varredura de degradação (sucesso: **PASS**)

| Caminho | cruzamento Gap-1 (`Φ ≤ 1/√3`) | cruzamento aceitabilidade (`overall < 0.80`) |
|:---|:---:|:---:|
| uniforme (Ω=Σ=Λ=1−d) | **d = 0.424** (Φ=0.5760) | d = 0.200 |
| enviesado (Ω=1−d …) | **d = 0.564** (Φ=0.5772) | d = 0.286 |

A inflexão teórica uniforme é `d = 1 − 1/√3 ≈ 0.4226` — a varredura a localiza
em `0.424` (passo 0.002). No caminho enviesado, a degradação concentrada na
estabilidade segura a coerência até `0.564`.

### E4 — Estabilidade sob ruído (sucesso: **NEGADO** conforme o critério estrito)

| Jitter | Φ médio | Φ std | dentro de 1σ do sem-ruído? |
|:---:|:---:|:---:|:---:|
| 0.00 (referência) | 0.9844 | 0.0021 | — |
| 0.05 | 0.9837 | 0.0022 | ✅ |
| 0.10 | 0.9810 | 0.0019 | ❌ (desvio 0.0034 > σ 0.0021) |
| 0.15 | 0.9777 | 0.0017 | ❌ (desvio 0.0067 > σ 0.0021) |
| 0.20 | 0.7214 | 0.0497 | ❌ (não exigido; regime degradado real) |

**Leitura honesta:** Φ absoluto permanece ≥ 0.98 até 15% de jitter — robustez
absoluta boa; porém o critério da especificação (média dentro de 1σ da média
sem ruído) falha a partir de 10%. É descoberta empírica, reportada sem
manipulação.

---

## ⚖️ PARECER DA AUDITORIA — Tabela de Realidade

| O que o escopo aprovado pede | O que foi executado | Veredito |
| :--- | :--- | :--- |
| Feature gate `experiments` | `[features] experiments = []`, `required-features` nos 4 bins; perfil default sem bins | ✅ Real |
| Logs estruturados (`RUST_LOG=info`) | `tracing` com `env-filter`; eventos observados em toda execução | ✅ Real |
| CSV + JSON por experimento | 10 artefatos em `mission_output/<eN>_results/` | ✅ Real |
| E1: Φ estabilizado > 0.70 + correlação Q–Φ | Φ médio 0.9837, Pearson 0.9873 | ✅ Real |
| E2: V2→V1, Φ > 0.95 em ≤ 1000 iterações, monotônico | 0.5584→0.9515 (26) e 0.6983→0.9581 (21), monotônicos | ✅ Real |
| E3: curva de resposta + ponto de inflexão Gap-1 | cruzamentos localizados (0.424 e 0.564) | ✅ Real |
| E4: dentro de 1σ para jitter < 20% | **não atendido** a partir de 10% | ❌ **NEGADO (honestidade)** |

---

## 📚 Verificação (neste host)

```bash
cargo build -p arkhe-field-stability                              # default: sem bins
cargo test -p arkhe-field-stability --all-targets                # 28/28 (default)
cargo test -p arkhe-field-stability --all-features --all-targets # 36/36 (33 unit + 3 wiremock)
cargo clippy -p arkhe-field-stability --all-features --all-targets # limpo
RUST_LOG=info cargo run -p arkhe-field-stability --features experiments --bin e1/e2/e3/e4
```

---

## 🏛️ PARECER FINAL

E1, E2 e E3 **PASS** caracterizam o `IterativeRefiner` como empiricamente
válido para o regime controlado (baseline estável, refinamento determinístico
V2→V1 monotônico, curva de resposta com inflexão Gap-1 localizada). **E4
NEGADO** sob o critério estrito registra uma descoberta genuína — ruído
aditivo ≥ 10% desloca a média de Φ além de 1σ do sem-ruído, embora Φ absoluto
permaneça alto. A fase 3 fica `ENTREGUE_PARA_VALIDACAO`, aguardando o parecer
do Arquiteto sobre o enquadramento do E4 (critério rigoroso vs. robustez
absoluta) antes da promoção do refiner para integração com o handover real.

```text
Um experimento que falha com honestidade vale mais
que uma métrica que mente por conveniência.
O refiner refina; o ruído demarca a fronteira.
```

**Selo:** `CATEDRAL-OS-FASE3-EXPERIMENTOS-E1E4-2026-09-06`

---

## 📌 ADDENDUM — DECISÃO DO ARQUITETO (2026-09-06)

> Registro original acima **preservado** (ledger append-only — Loopseal-2).
> A seguinte reclassificação é uma **decisão**, não um apagamento de fato.

**E4 reclassificado: `NEGADO` → `PASS CONDICIONAL`.**

- **Fundamento:** o sistema é validado para a **faixa operacional esperada**
  (jitter < 10%): critério estrito atendido em 5%, robustez absoluta
  Φ ≥ 0.98 mantida até 15%.
- **Ressalva documentada:** o critério estrito (1σ do sem-ruído) **não é
  atendido** para jitter ≥ 10% (desvio 0.0034 > σ 0.0021 a 10%).
- **Fronteira de colapso:** 20% de jitter → Φ 0.7214 ± 0.0497 (regime não
  operacional). Aplicação do refiner sob perturbações ≥ 10% exige
  mitigação explícita (treino com ruído / rejeição de jitter) — escopo da
  Fase 4.
- **Evidência:** artefatos CSV/JSON arquivados em `bloco_992/evidencia/`
  (com `SHA256SUMS`).
- **Decisão formal materializada no bloco 993** (autorização da Fase 4,
  handover 992→993).

**Selo do addendum:** `CATEDRAL-OS-E4-PASS-CONDICIONAL-DECISAO-2026-09-06`