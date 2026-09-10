# 🏛️ BLOCO 1009 — INVARIANTE I530 — DECAIMENTO DE COERÊNCIA EM [0,1] (v390.0)

> **Arquiteto-Ω** — Catedral OS
> Handover 1007 → **1008 → 1009** — Data: **2026-09-08** — **v390.0** (continua a linhagem v3xx.x)
>
> Extensão do substrato **`arkhe-field-stability`**: o invariante **I530**
> garante que o decaimento de coerência `c·k` (com `c` coerência atual e
> `k` retenção, ambas em [0,1]) permanece em [0,1] — com preservação do
> teto Gap-1 (0.9999) e margem estrita (o valor decaído máximo é 0.9998,
> nunca toca o teto). Prova em **core Lean 4.33.1, sem Mathlib, sem `sorry`**
> (convenção bloco 966), em `src/lean/I530_CoherenceDecay.lean`.

---

## 📐 Registro no Ledger — BLOCO 1009

```json
{
  "bloco": 1009,
  "versao": "v390.0",
  "handover_anterior": 1008,
  "data": "2026-09-08",
  "tipo": "EXECUCAO_I530_COHERENCE_DECAY",
  "status": "EXECUCAO_CONCLUIDA"
}
```

> **Hash real do JSON** (bruto/gzip): ver `evidencia/execucao_extensao_i530_decay.md`.

---

## 🩹 CORREÇÃO DOS 3 DEFEITOS DO DOCUMENTO PROPOSTO (v509.1)

| # | Defeito do plano v509.1 | Correção entregue neste bloco |
| :--- | :--- | :--- |
| 1 | Caminho `crates/arkhe-field-stability` (inexistente) | Crate real **`packages/arkhe-field-stability`** (registrado no Cargo.toml raiz) |
| 2 | `import Mathlib` + f64/ℝ (`mul_nonneg`, `mul_le_one`) e JSON «Bloco 966 (Sem sorry, Mathlib core)» — **oxímoro** (966 proíbe Mathlib) | Núcleo **core Nat ×10⁴**, `calc`/`native_decide`/lemas `Nat` — sem Mathlib, sem `sorry` (8 teoremas I530A..G + `I530_decay_bounded`) |
| 3 | `handover_anterior` como `"sha256:___"` placeholder e campo `hash` no JSON | Formato real: **`handover_anterior: 1008` (inteiro)**; sem campo `hash` no JSON; hashes brutos+gzip calculados mecanicamente em `evidencia/` |

---

## 🧬 NÚCLEO LEAN — `src/lean/I530_CoherenceDecay.lean` (`ArkheCoherenceDecay`)

- Escala única ×10⁴: `scale_x1e4 = 10000` (1.0), `ceiling_x1e4 = 9999` (teto Gap-1).
- Definição espelhada no Rust: `def decayed c k : Nat := c * k / scale_x1e4`.
- **8 teoremas** elaborados no kernel, exit 0:
  - `I530A` — piso 0 (`Nat.zero_le`).
  - `I530B` — clausura do produto: `c,k ≤ 10⁴ ⟹ c·k ≤ 10⁴·10⁴`.
  - `I530C` — decaído ≤ 1.0: `c,k ≤ 10⁴ ⟹ decayed ≤ 10⁴`.
  - `I530D` — teto preservado: `c,k ≤ 9999 ⟹ decayed ≤ 9998 < 9999`.
  - `I530E/F` — monotonia do decaimento em `c` e em `k` (análogo estrutural
    do clamp monotónico I524-C do núcleo da ponte Lean).
  - `I530G` — pior caso `(9999·9999)/10⁴ = 9998 < 9999` (margem constitucional estrita).
  - `I530_decay_bounded` — enunciado principal conjunto (banda [0,1] + teto).

## ⚙️ MÓDULO RUST — `packages/arkhe-field-stability/src/decay.rs`

| Item | Conteúdo |
| :--- | :--- |
| `SCALE_X1E4 = 10_000` / `CEILING_X1E4 = 9_999` | constantes da escala (espelham o núcleo) |
| `decay_scaled(u64, u64)` | `floor(C·K/10⁴)` — mesma definição `decayed` do núcleo Lean |
| `decayed(f64, f64)` | realização contínua com `debug_assert` das premissas da prova (banda [0,1]) em build debug |
| `lib.rs` | `pub mod decay;` + re-exports + bullet na doc de módulos |

## 🛡️ VERIFICAÇÃO MECÂNICA (resumo)

- `lean.exe src/lean/I530_CoherenceDecay.lean` → **exit 0** (8 elaborações,
  kernel v4.33.1, sem Mathlib, sem `sorry`)
- `cargo test -p arkhe-field-stability` → **77/77 exit 0**
  (70 pré-existentes + **7 novos** em `decay::tests`, espelhando I530-B..G)
- `cargo clippy -p arkhe-field-stability --all-targets -- -D warnings` → **exit 0**
- Deps: **zero dependências externas novas** (aritmética Nat/u64/f64 nativa — Simplicity-2)
- Evidência completa: `evidencia/execucao_extensao_i530_decay.md` + `SHA256SUMS`.

## ⚠️ LIMITAÇÃO HONESTA (registrada)

- A prova Lean I530 é sobre **inteiros Nat ×10⁴** (quantização do modelo); a
  `decayed(f64)` é a realização contínua do mesmo modelo e **não é verificada
  literalmente pelo kernel** — os `debug_assert` (build debug) e os 2 testes
  de banda f64 são sanidade, não prova.
- I530 preserva o **teto** e a banda unitária; o **piso** Gap-1 (0.5774) não é
  preservado sob retenção persistente — decaimento empurra para baixo por
  definição do modelo (esperado; a garantia deste invariante é superior).
- Nenhuma alteração no Φ canônico quadrático (I511–I516) nem no
  `CoherenceLedger` (Fase 4) — a extensão é ortogonal aos dois.

```text
O teto não se move quando o campo esfria:
cada tick retém o que o estado permite — e a banda permanece selada por cima.
```

**Selo:** `CATEDRAL-OS-EXTENSAO-DECAY-I530-BLOCO-1009-2026-09-08`