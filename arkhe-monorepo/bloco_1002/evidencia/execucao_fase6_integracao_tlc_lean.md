# Evidência — Fase 6 (bloco 1002) — Integração TLC ↔ Lean ↔ verify_integrity()

Execução da Fase 6 conforme o plano selado no `bloco_1001` (D1–D4). Toda
evidência abaixo é **mecânica e reproduzível** (nenhum valor fabricado).

Toolchain fixa:
- Rust: cargo 1.94.0 (workspace `arkhe-monorepo`)
- Lean 4: kernel v4.33.1 (elan), Lake 5.0.0-src+819816b
- TLC2: 2.19 (jar `tla2tools.jar` v1.7.4, sha1 `bee4a54f3ee3d4afc347c3240ec2d9e93b075104`)
- Java: OpenJDK 21.0.11 LTS

---

## 6.3 — D1: correção da tautologia com teste anti-cosmético (RED→GREEN)

**Achado (A1):** em `ledger.rs:173` a varredura de integridade comparava
`entry.compute_hash() != entry.compute_hash()` — tautologia que **nunca**
detecta adulteração de conteúdo da última entrada (nenhuma entrada seguinte
re-deriva o hash).

### Passo RED (teste anti-cosmético falhando)
Teste `verify_integrity_detects_last_entry_content_tamper`: mutação da **última**
entrada (phi 0.97 → 0.10) fora do `push`.

Resultado (commit `37abe98`):
```
running 37 tests
...
test ledger::tests::verify_integrity_detects_last_entry_content_tamper ... FAILED
test result: FAILED. 36 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

### Passo GREEN (correção)
- `CoherenceEntry.hash: String` — fingerprint SHA3-256 da forma canônica no
  momento da construção (`CoherenceEntry::from_parts`); `push` recalcula.
- `verify_integrity()` re-deriva e compara `entry.compute_hash() != entry.hash`
  (detecta a última entrada) + re-encadeia `previous_hash` (Loopseal-3).
- Novo `IntegrityStatus { Ok, Broken{window_ids}, BeyondHorizon{len,max_windows} }`
  (D3); `MAX_HORIZON_WINDOWS = 4` alinhado ao `MaxWindows = 4` do TLA+.
- `BeyondHorizon` **não é erro**: dados verificados, apenas fora do horizonte
  formal (D3).

Resultado (commits `1ba6979`, `a85fdbd`, `c712327`):
```
running 39 tests
...
test ledger::tests::verify_integrity_detects_last_entry_content_tamper ... ok
test ledger::tests::beyond_horizon_reported_above_max_windows ... ok
test ledger::tests::within_horizon_clean_chain_is_ok ... ok
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
   Doc-tests: 0 passed; 0 failed
```
Clippy: `cargo clippy --all-targets -- -D warnings` → **exit 0** (limpo).

### Artefatos e1 regenerados
`cargo run -p arkhe-field-stability --features experiments --bin e1`:
```
Ledger: 200 entradas, média Φ 0.9837
Integridade: OK (dados) — além do horizonte formal TLC/Lean (len=200 > MaxWindows=4)
```
Cada entry do `e1_coherence_ledger.json` agora carrega `"hash"` (SHA3-256).
Relatório: `e1_ledger_report.md`.

---

## 6.1/6.2 — Núcleo Lean `nuclei` (I517–I523)

Pacote Lake em `src/lean/nuclei` (layout canônico `lake init`):
`nuclei/nuclei/FieldStabilityTLCSpec.lean` — modelo finito `AppendEntryGuard`
(guarda `Len < MaxWindows`), `QuiesceGuard`, constantes `MaxWindows=4`,
`PhiMax=2`, instância e1–e3, teoremas **I517–I523** provados por `native_decide`
(sem Mathlib, sem `sorry`), com finitude declarada como hipótese (nunca fato
infinito — convenção blocos 966/971/972/997).

Resultado:
```
✔ [2/4] Built nuclei.FieldStabilityTLCSpec
✔ [3/4] Built nuclei
Build completed successfully (4 jobs).
=== EXIT: 0 ===
```

---

## 6.4 — Job CI (campo de verificação das 3 camadas)

`arkhe-monorepo/.github/workflows/field-stability-verify.yml` (commit `d72faf0`):

1. `cargo test -p arkhe-field-stability` — **39/39** (D1 + BeyondHorizon).
2. `cargo clippy --lib -- -D warnings` — limpo.
3. `lake build` (src/lean/nuclei, v4.33.1) — exit 0.
4. TLC v1.7.4 pinned (com verificação de sha1) sobre `ArkheCoherenceLedger.tla`,
   bloco: `grep "Model checking completed. No error has been found."`.

Validação local do passo TLC (idêntico ao CI):
```
sha1: bee4a54f3ee3d4afc347c3240ec2d9e93b075104  (confirmado)
TLC2 Version 2.19
202 states generated, 121 distinct states found
Model checking completed. No error has been found.
  calculated (optimistic): val = 5.3E-16   (probabilidade de colisão de fingerprint)
```

---

## Limite honesto (ref. relatório §11)

As provas Lean (I517–I523) e o model-check TLC cobrem o **modelo finito**
(`Len < MaxWindows`). A `verify_integrity()` real verifica a **integridade de
dados** de toda a cadeia (200 entradas no e1), mas jamais prova formal
infinita. Cadeias com `len > MAX_HORIZON_WINDOWS` retornam
`BeyondHorizon{len, max_windows}` — estados íntegros além do horizonte formal.
Apalache (symbolic) permanece como trabalho futuro (§6.5 do plano).

---

## Commits do bloco 1002 (branch/ref QC-0892)

- `37abe98` D1 RED (teste anti-cosmético falha na tautologia)
- `1ba6979` D1 GREEN (hash interno + IntegrityStatus + MAX_HORIZON_WINDOWS)
- `a85fdbd` D1 GREEN (e1.rs usa IntegrityStatus; force-add p/ gitignore_global)
- `bc2569b` 6.1/6.2 (pacote Lake nuclei I517–I523; plano atualizado)
- `c712327` D1 GREEN (artefatos e1 regenerados, BeyondHorizon honesto)
- `d72faf0` 6.4 (job CI field-stability-verify)

Bloco 1001 = plano (selado, zero código). Bloco 1002 = execução (este).
