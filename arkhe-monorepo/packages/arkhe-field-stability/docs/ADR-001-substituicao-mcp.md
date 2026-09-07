# ADR-001: Substituição do arkhe-mcp-client

**Data:** 2026-09-06
**Status:** Aceito
**Autor:** ZeroTwo_dev
**Revisores:** Arquiteto-Chefe, Equipe Core

## Contexto

O crate `arkhe-mcp-client` era uma dependência externa que bloqueava o
desenvolvimento do núcleo seguro. Não havia evidência pública de sua existência
ou manutenção ativa:

- O repositório não era público ou estava inacessível.
- O código não compilava com as versões atuais das dependências.
- Não havia documentação ou suporte para a versão necessária.

Além disso, o protocolo MCP (Model Context Protocol) ainda está em evolução, e
uma dependência fixa poderia se tornar obsoleta rapidamente.

## Decisão

**Substituir o `arkhe-mcp-client` por uma camada de abstração baseada em
`reqwest`.**

A nova implementação (`mcp_stub.rs`) fornece:

- **Estruturas de dados** para handover e qualidade.
- **Cliente HTTP** usando `reqwest` para comunicação com serviços MCP.
- **APIs** para submissão de handovers e consulta de qualidade.
- **Testes** com `wiremock` para validação da lógica de comunicação.

A substituição é formalizada com:

- **Redução de dependências**: apenas `reqwest`, `serde`, `thiserror`.
- **Isolamento da lógica de comunicação**: módulo `mcp_stub` facilita
  substituição futura.
- **Documentação**: ADR mantido no repositório para rastreabilidade.

## Integração com o monorepo (emenda aprovada)

O rascunho original propunha criar este código como `arkhe-safe-core`. Durante
a auditoria de merge (`Ghost-1/`Ghost-3 subsistrate check) descobriu-se que o
monorepo **já contém** `services/safe-core` (`name = "arkhe-safe-core"`):
o watchdog anti-alucinação do substrate 491-AGI-CORTEX (gRPC/tonic,
`RecurrencyPolicy`, vibe clamp). Dois crates com o mesmo nome não podem
coexistir no workspace.

**Decisão do Arquiteto-Chefe:** renomear o crate para `arkhe-field-stability`
(mantém o ADR e o módulo `mcp_stub` intactos; o código de estabilidade de
campo é o núcleo de valor) e registrá-lo como novo membro do workspace em
`packages/arkhe-field-stability`. O `services/safe-core` permanece intacto.

Correções aplicadas frente ao rascunho:

1. `chrono` adicionado a `[dependencies]` (era usado sem declarar).
2. `wiremock` adicionado a `[dev-dependencies]` (era usado sem declarar).
3. `proptest` e `tempfile` removidos de `[dev-dependencies]` (nunca usados).
4. `thiserror = "2.0"` corrigido para o workspace (`thiserror = "1"`).
5. `[workspace] members = ["."]` removido — colidiria com o workspace raiz.
6. `[profile.release]` removido — o workspace raiz já o define para todos.
7. Campo `FieldStability::latency_ms` renomeado para `latency_score` — o valor
   armazenado é um score normalizado [0,1] (não ms bruto); o `Default`
   original (50.0) quebraria o somatório ponderado do relatório.

## Consequências

### Positivas
- **Desenvolvimento desbloqueado**: o crate compila e executa imediatamente.
- **Arquitetura mais flexível**: a lógica de comunicação fica isolada e pode
  ser substituída por um cliente MCP oficial no futuro.
- **Manutenção simplificada**: `reqwest` é um crate maduro, amplamente
  utilizado e com suporte ativo.
- **Testabilidade**: a camada de abstração pode ser mockada facilmente.

### Negativas
- A implementação atual **não é** o protocolo MCP completo — é um stub que
  cobre os endpoints necessários.
- **Sem suporte nativo** para autenticação ou streaming, mas isso pode ser
  adicionado quando necessário.

## Alternativas Rejeitadas

| Opção | Motivo da Rejeição |
|-------|-------------------|
| **A: Esperar** | Manter a dependência e aguardar correção. Não havia prazo para a correção. |
| **B: Implementar MCP completo** | Implementar o protocolo MCP do zero. Escopo excessivo para o crate atual. |
| **C: Usar outro cliente MCP** | Nenhum crate alternativo maduro disponível para o protocolo MCP. |
| **D: Manter nome `arkhe-safe-core`** | Colisão de namespace com `services/safe-core` já existente no workspace. |

## Plano de Migração

1. **Criar `mcp_stub.rs`** com a implementação baseada em `reqwest`. ✅
2. **Atualizar `Cargo.toml`** com as novas dependências. ✅
3. **Remover `arkhe-mcp-client`** das dependências. ✅ (nunca foi integrado)
4. **Atualizar `main.rs`** para usar o `McpClient` do stub. ✅
5. **Validar** com `cargo build` e `cargo test`. ✅
6. **Documentar** a decisão via este ADR. ✅
7. **Registrar o crate** no workspace raiz como `packages/arkhe-field-stability`. ✅

## Verificação

- [ ] `cargo build` passa sem erros
- [ ] `cargo test -p arkhe-field-stability` passa
- [ ] O binário `e1` executa sem erros de dependência
- [ ] O ADR está commitado no diretório `docs/`

---

**Aprovado por:** Arquiteto-Chefe
**Data da aprovação:** 2026-09-06