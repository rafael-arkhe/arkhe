# Evidencia — Bloco 1009 — Extensao field-stability: invariante I530 (decaimento em [0,1])

Data: 2026-09-08. Comandos executados mecanicamente no monorepo
(workdir: `arkhe-monorepo`), apos a entrega do nucleo Lean I530 e do
modulo `packages/arkhe-field-stability/src/decay.rs`.

## 1. Continuidade de numeracao e versao

- Bloco anterior no ledger: `bloco_1008` (v389.1). Proximo bloco: `1009`.
- Versao: `v390.0` (continua a linhagem real do ledger; o plano v509.1 era
  um documento paralelo nao canonizado, sem versao no ledger real).
- `handover_anterior`: **1008** (inteiro = numero do bloco, formato real —
  o plano v509.1 propunha um marcador `"sha256:___"` que nao existe no
  schema real; os blocos 1006-1008 registram os hashes em `evidencia/`).

## 2. Correcao dos defeitos do documento proposto v509.1

1. **Caminho**: o plano usava `crates/arkhe-field-stability`; o workspace
   real registra o crate em `packages/arkhe-field-stability` (Cargo.toml raiz).
2. **Prova Lean**: o plano usava `import Mathlib` (f64, `Real`, `mul_nonneg`,
   `mul_le_one`) e afirmava convencao "Bloco 966 (Sem sorry, Mathlib core)" —
   oximoro: **o bloco 966 proibe Mathlib**. Este bloco entrega o nucleo CORE
   (v4.33.1, sem Mathlib, sem `sorry`) em inteiros naturais, escala fixa
   x10^4, com `calc`/`native_decide`/lemas `Nat` core.
3. **Formato do registro**: JSON canonico real (campo `handover_anterior`
   inteiro; sem campo `hash` interno); hashes calculados mecanicamente abaixo.

## 3. Nucleo Lean — `src/lean/I530_CoherenceDecay.lean`

`ArkheCoherenceDecay`, 8 teoremas elaborados no kernel (`lean` exit 0):

| Teorema | Enunciado (escala x10^4) | Tacticas |
| :--- | :--- | :--- |
| `I530A_decayed_nonneg` | `0 <= decayed c k` | `Nat.zero_le` |
| `I530B_product_within_unit` | `c,k <= 10000 => c*k <= 10000*10000` | `Nat.mul_le_mul_right/left`, `Nat.le_trans` |
| `I530C_decayed_within_unit` | `c,k <= 10000 => decayed <= 10000` | `Nat.div_le_div_right`, `native_decide` |
| `I530D_decayed_within_ceiling` | `c,k <= 9999 => decayed <= 9998 < 9999` | idem + `native_decide` |
| `I530E_decayed_monotone_c` | `c1 <= c2 => decayed c1 k <= decayed c2 k` | `Nat.mul_le_mul_right`, `Nat.div_le_div_right` |
| `I530F_decayed_monotone_k` | `k1 <= k2 => decayed c k1 <= decayed c k2` | `Nat.mul_le_mul_left`, `Nat.div_le_div_right` |
| `I530G_worst_case_strictly_below_ceiling` | `(9999*9999)/10000 = 9998 < 9999` | `native_decide` |
| `I530_decay_bounded` | conjunto: banda [0,1] + teto preservado | `constructor` + acima |

- `def decayed c k : Nat := c * k / scale_x1e4` — mesma definicao do Rust
  `decay_scaled`.
- `scale_x1e4 = 10000` (1.0); `ceiling_x1e4 = 9999` (0.9999 teto Gap-1).
- Nenhum `sorry`; nenhum `import` alem do preludio core.

## 4. Modulo Rust — `packages/arkhe-field-stability/src/decay.rs`

- `SCALE_X1E4 = 10_000`, `CEILING_X1E4 = 9_999`.
- `decay_scaled(coherence_scaled: u64, retention: u64) -> u64` — espelha a
  `decayed` do nucleo (x10^4).
- `decayed(c: f64, k: f64) -> f64` — realizacao continua do modelo com
  `debug_assert` das premissas da prova (banda [0,1] de operandos e
  resultado) em build debug.
- `lib.rs`: `pub mod decay;` + re-exports
  (`decay_scaled`, `decayed`, `CEILING_X1E4`, `SCALE_X1E4`) + bullet no doc.

## 5. Verificacao mecanica

Lean — nucleo I530:
  lean.exe src/lean/I530_CoherenceDecay.lean -> exit 0 (8 elaboracoes)
  (kernel v4.33.1 local: C:\Users\Lemes\.elan\bin\lean.exe)

Cargo — testes arkhe-field-stability:
  test result: ok. 77 passed (70 pre-existentes + 7 novos decay::tests)
  Nomes novos: scaled_closure_within_unit, scaled_band_preserved_below_ceiling,
  scaled_worst_case_strict_below_ceiling, scaled_monotone_in_coherence,
  scaled_monotone_in_retention, f64_stays_in_band,
  f64_ceiling_product_strict_below_teto.

Cargo — clippy (all targets, -D warnings):
  cargo clippy -p arkhe-field-stability --all-targets -- -D warnings
  -> exit 0 (sem warnings)

Dependencias: nenhuma externa nova (aritmetica Nat/u64/f64 nativa) —
  Cargo.lock raiz inalterado por esta extensao (Simplicity-2).

## 6. Honestidade

- A prova Lean I530 e sobre **inteiros Nat x10^4** (quantizacao do modelo);
  `decayed(f64)` e a realizacao continua e **NAO e verificada literalmente
  pelo kernel** — os `debug_assert` (build debug) e os 2 testes de banda
  f64 sao sanidade, nao prova.
- I530 preserva o **teto** (0.9999) e a banda unitaria; o **piso** Gap-1
  (0.5774) nao e preservado por retencao persistente — decaimento empurra
  para baixo por definicao do modelo (nao e bug: e o esperado; a garantia
  deste invariante e superior).
- Nenhuma alteracao no Phi canonico quadratico (I511-I516) nem no
  `CoherenceLedger` (Fase 4): a extensao e ortogonal.

## 7. Hash real do bloco_1009 (reprodutibilidade)

- bloco_1009.json (UTF-8, LF): SHA-256 bruto
  `a0bc69079011d8ff3783ef834cef0b1ece14dc99d46929044c61d05c82e40b96`
- bloco_1009.json gzip (Optimal): SHA-256 comprimido
  `e0ab3193d1e5bb53fa1132af2776496e87777f57c6570baedecd122a522ea0ca`