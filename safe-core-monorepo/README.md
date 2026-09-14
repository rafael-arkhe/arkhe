# safe-core-monorepo — extraído do patch.md e verificado

**Data:** 2026-07-04
**Origem:** última iteração do "Monorepo AGI Safe-Core v1.2" contida em `patch.md` (um dos 8 documentos enviados), com lacunas cruzadas com `safe-core.md`.
**Toolchain de verificação:** rustc/cargo **1.91.1**, edition 2021.

## Estado real (o único que conta)

```
cargo check --workspace  →  Finished (0 erros)
```

**15 crates compilam.** São os membros listados no `Cargo.toml`:

arkhe-core, arkhe-identity, arkhe-governance, arkhe-pea, arkhe-memory, arkhe-inference, arkhe-session-evaluator, arkhe-reflector-agent, arkhe-agi, arkhe-cli, arkhe-lean-spec-derive, arkhe-rate-limit, arkhe-bom, arkhe-prompt-detector, arkhe-hallucination.

## O que foi consertado para compilar

O documento afirmava "29/29 defeitos resolvidos, zero APIs fantasma". Ao montar e compilar de verdade, apareceram problemas reais:

- **arkhe-core/lib.rs**: re-exportava `ArkheHash`, `SafetyCheck`, `MemoryLayer` de módulos que não os definem com esses nomes. Alinhado ao que os módulos realmente exportam.
- **arkhe-session-evaluator**: dois erros de borrow-checker legítimos (move de referência compartilhada; uso após move) — corrigidos com clone e reordenação.
- **arkhe-prompt-detector**: chamava a macro fantasma `new_regex!` (é função, não macro); `.contains(p)` com tipo errado; `Self::new()` inexistente. Três correções.
- **arkhe-rate-limit**: usava `#[derive(Serialize, Deserialize)]` sem importar serde.
- **arkhe-bom**: usava `uuid::Uuid` sem a dependência declarada.
- **Cargo.toml raiz**: `[workspace.dependencies]` com `optional = true` (ilegal); seção `[workspace.features]` inválida; membros fantasma (`arkhe-planning`, `arkhe-continual`) sem diretório.

## O que NÃO entrou (removido do workspace)

- **arkhe-artifact-signing**: 23 erros — usa APIs `SigningKey::generate`/`as_mut_bytes` que não existem no ed25519-dalek 2.x. É o "fantasma" clássico que o próprio patch.md dizia ter eliminado, mas não eliminou.
- **arkhe-input-validation, arkhe-output-filter, arkhe-tool-sandbox**: incompletos no documento (sem Cargo.toml ou sem src/lib.rs, com submódulos declarados mas não escritos).

## Aviso sobre os diretórios extras

`crates/` contém ~41 diretórios, mas o workspace só compila os 15 membros. Os ~26 restantes (arkhe-cloud-provider, arkhe-unified-fs, arkhe-vector-store, safe-core-crypto, safe-core-policy, etc.) foram extraídos do documento mas **não estão wired no Cargo.toml** — não são compilados nem verificados. Ficaram em disco porque o ambiente de extração não permite deletar arquivos. Para incorporá-los, adicione ao `members` e rode `cargo check` um a um; espere o mesmo tipo de correção feita acima.

## Reproduzir

```bash
cd safe-core-monorepo
cargo check --workspace     # requer rustc >= 1.85 (usei 1.91)
```

Veredito: dos ~41 crates que o documento apresentava como "completos e verificados", **15 realmente compilam** após correções pontuais. É um núcleo real e aproveitável — o resto precisa do mesmo tratamento, um por um.

## Atualização — 2026-07-13

A contagem acima ("15 crates") está desatualizada: commits posteriores
("Onda 1-4") já haviam elevado o workspace para ~30 membros antes desta
sessão (arkhe-evidence, arkhe-network, arkhe-reasoning, arkhe-storage,
arkhe-nostr-anchor, arkhe-blossom, arkhe-web-gateway, arkhe-agent-vm,
arkhe-rsi/-core, arkhe-web3-security, arkhe-crypto-pqc, arkhe-pqc-core,
entre outros) — nenhum desses foi reauditado aqui, apenas confirmado que
`cargo build --workspace` continua limpo.

Esta sessão adicionou dois membros novos, ambos com `cargo test` real e
verificação manual (não apenas `cargo build`):

- **`arkhe-geometric-verifier`** (FI-120–FI-125): GeometricVerifier +
  4 grafos tipados (proveniência, ontologia, memória, invariantes).
  30/30 testes passam. Ver `crates/arkhe-geometric-verifier/README.md`.
- **`arkhe-mcp-server`**: servidor MCP real sobre stdio (rmcp v2.2.0,
  resolvido do crates.io), expondo `AgiCoordinator` e `GeometricVerifier`
  como tools. Testado manualmente com JSON-RPC real (`initialize`,
  `process_turn`, `geometric_verify`, `stats`, `provenance_summary`) —
  transcript real em `crates/arkhe-mcp-server/README.md`. Roda sobre
  `AlwaysAllowVerifier`/`InMemoryAgentMemory`/`NullEngine` (stack de
  dev/teste, sem LLM de produção por trás ainda).

Total atual: **31 membros no workspace** (contagem exata de entradas
`"crates/..."` no array `members` do `Cargo.toml` raiz). Uma reauditoria
completa dos ~16 membros adicionados entre 2026-07-04 e esta sessão ainda
não foi feita — este parágrafo documenta apenas o que foi verificado
agora, não reafirma o que não foi reconferido.
