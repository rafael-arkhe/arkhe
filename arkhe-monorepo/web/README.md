# Painel da Catedral — ARKHE OS

Artefato p5.js de visualização constitucional. Abre direto no browser
(HTML estático + p5.js via CDN, sem build tooling).

## Status

**PROTÓTIPO VISUAL (mockup)**

| Aspecto | Estado |
|---------|--------|
| Dados | **ESTÁTICOS** — simulados no cliente, não conectados a backend |
| Cadeia | NÃO verificada criptograficamente neste artefato |
| Testes | `node --check` (sintaxe) apenas — não é teste de comportamento |
| Healthcheck | Renderizado, mas observa estado mock, não o sistema |

## Arquivos

- `catedral_dashboard.html` — página: canvas p5 + sidebar DOM + header.
- `catedral_sketch.js` — raiz: `state` global, `setup()`/`draw()`, registo de
  `updateCatedralState()` no loop.
- `catedral_ext.js` — camadas: gauge Φ_C (Gap-1), grade 19 invariantes,
  mini-ledger TemporalChain, healthcheck, rótulo de origem dos dados.

## Banda Φ_C (Gap-1)

- Piso `0.577350` = truncamento de `1/√3 = 0.5773502691…` — a "constante
  mágica" da Catedral. Fonte: `docs/coherence_metric.md §1`;
  `packages/arkhe-field-stability/src/phi.rs` (`GAP1_INFERIOR`).
- Teto `0.999900 = 1 − 10⁻⁴` — Φ=1 é ideal assintótico, jamais certificável
  (Λ=1 exigiria latência nula em medição contínua). Fonte:
  `docs/coherence_metric.md:17`.
- Realidade implementável existente (não conectada): `CoherenceLedger`
  (crate `arkhe-field-stability`, Fase 4) mede Φ real em
  `[0.577350, 0.999900]`; bloco* .json encadeiam por `hash_anterior`.

## Próximo passo (para operacional, auditoria bloco 1071)

1. Endpoint backend `/api/v1/catedral/state` retornando JSON real.
2. Verificação de cadeia com `parent_hash`, `this_hash`, `payload_hash`.
3. Testes de comportamento (não apenas `node --check`).

## Verificação feita

- `node --check catedral_ext.js` e `catedral_sketch.js` — sintaxe OK (parse).
- Loop simulado com stub p5/JS (10 ticks) — sem erro de runtime, Φ em banda.
  Isto é smoke test de desenho, não validação de dados.