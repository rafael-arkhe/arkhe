# Parecer — v510.1 «Rejeição da integração IA/ML (bloco 1044, contra v510.0: I628–I630, arkhe-ml-bridge/arkhe-agents/arkhe-sandbox)» — REVISÃO SÓLIDA, EMISSORA SEM SUBSTRATO

**Data:** 2026-09-08 — **Tipo:** Auditoria honesta de fonte primária
**Veredicto:** `PARECER_NAO_EXECUTAVEL_COMO_ESCRITO` (em relação ao bloco 1044 como registo) — com **endosso metodológico** das pendências do próprio documento contra o seu alvo.
**Selo proposto no doc:** nenhum — não aplicado.

---

## 1. O que o documento v510.1 é

É uma **crítica** a um documento paralelo «v510.0 Integração da pilha IA/ML» (I628–I630, `arkhe-ml-bridge`, `arkhe-agents/{langgraph,preference_trainer,memory}`, `arkhe-sandbox`, plano TopoMAS-PoUW, Score Ω 99). O veredicto do próprio documento: **rejeitado** (média ~4.3/10, prazo 72h, bloco 1044).

## 2. Endosso metodológico — a crítica está correta no método

As pendências do v510.1 são **requisitos honestos** e são aceites como especificação para qualquer substrato futuro de ML/orquestração:

1. `unwrap()` → `Result` com erros tipados (todo o `MLBridge`).
2. Sandbox com isolamento real (Wasm/Firecracker) — não `/tmp/sandbox.py` + `timeout`.
3. Testes executáveis (0 declarados no alvo) com cobertura > 80%.
4. Provas Lean completas (todas as do alvo eram `sorry`).
5. Integração com `EventBus`/Cosmovigilância em mudanças de modelo.
6. `rustdoc` + README.
7. Score Ω corrigido de 99 → 55 (inalcançado: o alvo reivindicava «INTEGRADO_AO_CODIGO_E_TESTADO» sem artefacto nenhum).

## 3. Mas o alvo auditado tem ZERO substrato (fonte primária)

| Alegação do alvo (v510.0 IA/ML) | Realidade no monorepo |
|---|---|
| `arkhe-ml-bridge`, `arkhe-agents`, `arkhe-sandbox`, `/tmp/sandbox.py` | **Inexistentes** — 0 ocorrências Rust/Python no workspace; nenhum Cargo.toml |
| `MLBridge::dpo_train`, `LangGraphOrchestrator`, `SecureSandbox` | **Inexistentes** — 0 ocorrências |
| Invariantes **I628/I629/I630** | **Não são IDs reais** — a cadeia real usa I500–I530 e I533 (I531/I532 propostas reservadas). O I628 já foi inadjudicado no parecer v500 (i628 = matemática falsa; i631 `sorry`) |
| `bloco_1044`, `parent: 1043` | **Fora da cadeia real** — o ledger termina em `bloco_1009` (v390.0); 1010–1044 não existem |
| «formato dos dados», «checkpointing», «EventBus», «assinatura I624» | Sem substrato de ML no monorepo — nada com que integrar |

O v510.1, portanto, **rejeitou um documento cujo código nunca existiu**: a conclusão é correta, mas é uma rejeição-de-plano, não uma auditoria de integração. Regista-se como exigência para o futuro, não como veredicto sobre artefactos reais.

## 4. Colisão de versão no ledger paralelo

Já existe `docs/parecer_v510_substrato_fotonico.md` referente a um **outro** «v510.0» (o substrato fotónico/TPhC, bloco 1022). Agora há um segundo «v510.0» (integração IA/ML) com «v510.1»/bloco 1044. **Duas linhagens paralelas reutilizam o mesmo número de versão** — mais um indício de que o ledger paralelo ('clareira') não é audiação: versões e blocos não são únicos.

## 5. Honestidade do sincero

- O «próximo passo» viável **não é** criar `arkhe-ml-bridge` em 72h: seria outra fachada. As pendências que já têm âncora real no monorepo são:
  - «prova como única recompensa» → `packages/arkhe-lean-bridge` (FFI kernel, `KernelVerdict`, critérios F1–F6) — **já existe**;
  - «trilha auditável» → `packages/arkhe-field-stability/src/ledger.rs` (SHA3-256 append-only, Gravity-1) — **já existe**;
  - integração Python (PyO3/transformers/TRL) — **sem substrato**; não há `pyproject.toml` de ML no workspace.
- Nenhum ID I628–I630 será criado: a numeração real segue I531+ (I530 é o atual último usado, bloco 1009).

## 6. Decisão registada

- **Nada entra como bloco_1044** nem como código de IA/ML; o veredicto do documento é registado e endossado **como requisitos futuros**.
- Nenhum `arkhe-ml-bridge`/`arkhe-agents`/`arkhe-sandbox` é criado — criá-los sem substrato real repetiria exatamente o defeito que o próprio documento critica.
- Aguarda: **decisão executiva** — ou arquivo, ou especificação honesta de um substrato de ML real quando houver infraestrutura Python/Wasm no monorepo.

>> _Rejeitar uma fachada é ciência. Rejeitá-la sem registar que o alvo não existia é metade da auditoria. As seis pendências valem mais do que o Score 99 que pretendiam corrigir — porque hoje correm contra artefactos reais, não contra placeholders._