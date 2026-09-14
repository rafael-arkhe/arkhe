# 🏛️ BLOCO 987 — ARKHE-FIELD-STABILITY (estabilidade de campo + stub MCP)

> **Arquiteto-Ω** — Catedral OS
> Handover 986 → 987 — Data: **2026-09-06** — **v376.1**
>
> O crate proposto como `arkhe-safe-core` foi auditado, **renomeado** para
> **`arkhe-field-stability`** (`packages/arkhe-field-stability`) e **ACEITO**:
> **11/11 testes** (8 unit + 3 wiremock), clippy limpo (`--all-targets`),
> **zero `unsafe`**, binário `e1` executando end-to-end, ADR-001 formalizando a
> substituição do `arkhe-mcp-client` por `reqwest`. O watchdog anti-alucinação
> existente `services/safe-core` permanece intacto.

---

## 📐 Registro no Ledger — BLOCO 987 v376.1

```json
{
  "bloco": 987,
  "versao": "v376.1",
  "handover_anterior": 986,
  "data": "2026-09-06",
  "tipo": "GERACAO_ARQUIVOS_ARKHE_FIELD_STABILITY",
  "descricao": "Aceitação do crate arkhe-field-stability (renomeado de arkhe-safe-core para não colidir com services/safe-core). 11/11 testes (8 unit + 3 wiremock), clippy limpo (--all-targets), zero unsafe, bin e1 executando end-to-end, ADR-001 (substituição do arkhe-mcp-client por reqwest), 7 correções aplicadas frente ao rascunho.",
  "arquivos_gerados": [
    "Cargo.toml",
    "src/lib.rs",
    "src/main.rs",
    "src/mcp_stub.rs",
    "src/field_stability.rs",
    "src/quality_report.rs",
    "src/constants.rs",
    "docs/ADR-001-substituicao-mcp.md",
    "mission_output/e1_results/.keep"
  ],
  "correcoes_auditoria": [
    "chrono estava em uso sem estar declarado em [dependencies] → adicionado",
    "wiremock estava em uso em testes sem estar em [dev-dependencies] → adicionado",
    "proptest e tempfile em [dev-dependencies] nunca usados → removidos",
    "thiserror = \"2.0\" conflitava com o workspace (\"1\") → corrigido para { workspace = true }",
    "[workspace] members = [\".\"] dentro do crate colidiria com o workspace raiz → removido",
    "[profile.release] removido (o workspace raiz já o define)",
    "FieldStability::latency_ms era o campo errado: guardava score normalizado [0,1], não ms bruto → renomeado latency_score (Default 50.0 quebraria o somatório ponderado)"
  ],
  "testes": {
    "unit": 8,
    "wiremock": 3,
    "total": 11,
    "falhou": 0
  },
  "status": "ACEITO",
  "selo": "CATEDRAL-OS-FIELD-STABILITY-VERIFIED-2026-09-06",
  "verificacao_deste_host": {
    "cargo_test": "cargo test -p arkhe-field-stability --all-targets -> 11/11 OK",
    "clippy": "cargo clippy -p arkhe-field-stability --all-targets -> limpo",
    "bin_e1": "e1 executado: estabilidade 95.17%, sucesso 100%, overall 97.27% (>= 80%), MCP falha controlada sem servidor"
  }
}
```

> **Nota de encadeamento:** este bloco segue a numeração da cadeia v376 da
> Catedral OS — os blocos 973–986 pertencem a esse ledger (cadeia v3xx/v3yy),
> hoje **externo a este monorepo** (mesmo precedente dos blocos 964–970,
> notado no bloco 971). O último bloco materializado aqui é o **972**
> (ARKHE-BITCOIN, v360.0). O presente bloco 987 consolida a *emenda de
> integração*: o crate renomeado é um novo membro do workspace, sem colisão
> com `services/safe-core`.

---

## ⚖️ PARECER DA AUDITORIA — Tabela de Realidade

| O que o rascunho declara | O que foi verificado | Veredito |
| :--- | :--- | :--- |
| "arkhe-safe-core" (novo crate) | **Colisão**: `services/safe-core` já é `arkhe-safe-core` (watchdog 491-AGI-CORTEX). Dois crates não podem coexistir | **Renomeado → `arkhe-field-stability`** |
| Compilável | `cargo build -p arkhe-field-stability` sem warnings | **Real** |
| 11/11 testes | 8 unit + 3 wiremock, 0 falhou (`--all-targets`) | **Real** |
| Clippy limpo | `cargo clippy --all-targets` sem warnings | **Real** |
| Zero `unsafe` | `#![deny(unsafe_code)]` em `lib.rs` + lints do crate | **Real** |
| Binário `e1` | executa end-to-end; MCP falha controlada sem servidor (by design) | **Real** |
| `chrono`/`wiremock` declarados | faltavam no Cargo.toml do rascunho → declarados | **Real** |
| `[workspace]` interno | removido — colidiria com o workspace raiz | **Real** |
| Campo `latency_ms` (score) | semântica corrigida para `latency_score` [0,1] | **Real** |

> **Nota de honestidade (precedente bloco 966):** este bloco **não declara**
> invariantes formais em Lean (nenhum formato de chave/receipt é tocado aqui;
> as propriedades de campos de estabilidade são reais de ponto flutuante, fora
> do escopo do núcleo Lean puro). Rigor exigido deste crate: testes unitários +
> wiremock que cobrem o contrato HTTP do stub.

---

## 📂 Estrutura

```
packages/arkhe-field-stability/
├── Cargo.toml                  # reqwest, chrono; workspace serde/thiserror/tokio/tracing
├── src/
│   ├── lib.rs                  # #![deny(unsafe_code)], re-exporta API
│   ├── main.rs                 # bin «e1» — smoke-test end-to-end
│   ├── mcp_stub.rs             # cliente HTTP stub MCP (McpClient, HandoverPayload)
│   ├── field_stability.rs      # FieldStability (estabilidade, sucesso, latência)
│   ├── quality_report.rs       # QualityReport (overall ponderado, is_acceptable)
│   └── constants.rs            # WEIGHT_* e QUALITY_THRESHOLD
├── docs/
│   └── ADR-001-substituicao-mcp.md   # decisão + emenda de rename registrada
└── mission_output/
    └── e1_results/.keep
```

---

## 📚 Verificação (executada neste host)

```bash
# Build
cargo build -p arkhe-field-stability

# Testes — 11/11 (8 unit + 3 wiremock)
cargo test -p arkhe-field-stability --all-targets

# Clippy — limpo
cargo clippy -p arkhe-field-stability --all-targets

# Binário E1
cargo run -p arkhe-field-stability --bin e1
```

Saída observada do `e1`:

```text
🚀 Arkhe Field Stability — Validação E1
❌ Erro ao enviar handover: HTTP request failed …            (sem servidor — falha controlada)
❌ Erro ao obter qualidade: HTTP request failed …            (sem servidor — falha controlada)
📊 Estabilidade de Campo:
   Nome: campo_1788722678
   Estabilidade: 95.17%
   Taxa de sucesso: 100.00%
   Latência (score): 0.96
📊 Relatório de Qualidade:
   Overall: 97.27%
✅ Qualidade aceitável (>= 80%)
```

---

## 🏛️ PARECER FINAL

O crate **`arkhe-field-stability`** é **ACEITO**. A auditoria de merge teve
**7 correções** frente ao rascunho — a mais estrutural sendo o **renomeio**
de `arkhe-safe-core` (colisão de namespace com o watchdog existente) para
`arkhe-field-stability`, registrado como emenda ao ADR-001. O código
compila, testa (11/11, incluindo wiremock do contrato HTTP), passa clippy e o
binário `e1` executa de ponta a ponta. O `services/safe-core` (watchdog
anti-alucinação do 491-AGI-CORTEX) permanece intacto e no workspace.

```text
Uma dependência bloqueante é um dragão adormecido.
Substituí-la é acordar o sistema para o progresso.
O nome que colide não convive: renomear é coexistir.
```

**Selo:** `CATEDRAL-OS-FIELD-STABILITY-VERIFIED-2026-09-06`