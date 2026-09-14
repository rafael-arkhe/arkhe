# ADR-003: Verificação Criptográfica do Target-of-Record

- **Data:** 2026-09-13 (proposta)
- **Status:** Proposto — aguarda decisão do Arquiteto-Chefe (sem código)
- **Autor:** Kronos (agente de pesquisa)
- **Revisores:** Arquiteto-Chefe, Líder de Segurança (Safe-Core), Líder de
  Infraestrutura, Líder de Pesquisa (traceability/TOON)

## Contexto

A hive (Swarmboard DAO) mantém uma tesouraria partilhada de **43.055 ETH** e um
conjunto de propostas de despesa que dependem de *target-of-record* (ToR):
registos publicamente acessíveis que documentam a entrega de um fornecedor.

A proposta **#001** (public target-of-record) venceu com 27/0/2, e o alvo
escolhido é `target-of-record.bytethebuilder.workers.dev` — um subdomínio
`workers.dev` alojado em Cloudflare Workers. A proposta promete uma
*"server-assigned monotonic sequence"* como prova de ordem, mas **não especifica
nenhum mecanismo criptográfico** que permita a um terceiro confirmar que o
registo não foi adulterado.

O problema é exatamente o mesmo que Arkhe já resolve nos próprios substratos:

| Prioridade | Invariante Arkhe | Equivalente na hive |
|:---|:---|:---|
| Ghost-1 | Substrate Integrity — hashes devem bater o manifest | ToR íntegro após escrita |
| Loopseal-2 | Proof Log Immutability — append-only e tamper-evident | Histórico do ToR imutável |
| Loopseal-3 | Audit Trail Completeness | Todo o ciclo auditoria→verificação registado |
| Gravity-1 | Temporal Consistency — timestamps monotónicos | Sequência server-assigned verificável |
| Provenance-1 | TLSNotary Notarization | Vinculação da origem ao conteúdo |

A sequência monotónica de servidor (`#001`) prova **ordenação**, não
**integridade**. Um atacante com acesso ao serviço pode reescrever entradas
passadas mantendo a monotonia. O que falta na pilha da hive é o mecanismo que
fecha esta lacuna.

## Decisão

Definir uma **spec de verificação de target-of-record** em camadas,
reutilizando os mecanismos já canonizados no monorepo (`CoherenceLedger`,
SHA3-256 output, Gravity-1 nativo):

1. **Formato de entrada (entry)** — JSON canónico com `prev_hash` de
   encadeamento (Ghost-1).
2. **Hash chain** — cada entrada temela ao `prev_hash` da anterior; a cadeia é
   append-only por construção (Loopseal-2).
3. **Âncora obrigatória** — o *head* da cadeia (ou um root periódico) deve ser
   comprometido num destino imutável: TLSN (TLSNotary), um ledger de âncora,
   ou asseveração multisig na Safe. **Sem âncora, a cadeia só é
   tamper-evident dentro de si — não tamper-proof.**
4. **Procedimento de verificação** — uma sequência exata que um terceiro
   executa com entrada pública (URL do ToR) e saída binária (VÁLIDO / INVÁLIDO).

A spec não descreve a implementação da hive — descreve o **contrato de
verificação** que qualquer fornecedor (Hetzner, Palmyr, workers.dev, ...) deve
cumprir para que a despesa tenha entrega observável.

### Modelo de dados

```json
{
  "spec": "arkhe/tor/v1",
  "entry": 17,
  "timestamp": 1757746800,
  "payload": { "event": "export_created", "export_url": "https://.../export-17.tar.gz" },
  "prev_hash": "9f2b...",
  "hash": "c5a8..."
}
```

- `hash = SHA3-256( spec || entry || timestamp || payload || prev_hash )`
- `prev_hash` da entry 0 = **`0` de 64 caracteres** (hash vazio da operação).
- `timestamp` deve ser **estritamente crescente** ao longo da cadeia
  (Gravity-1): verificação rejeita cadeia não-monotónica.
- `payload` é a parte semântica (qualquer JSON); o encadear e o carimbo são a
  parte verificável.

### Âncora (root commitment)

Esquema recomendado, por ordem de força:

| Âncora | Força | Custo | Verificação |
|:---|:---|:---|:---|
| **[565] TLSNotary** | Alta — prova quer a origem quer o conteúdo da resposta | Alto (MPC-TLS) | Third-party re-executa o prove; passa na 565-BRIDGE |
| **Ledger de âncora (tipo `CoherenceLedger`)** | Média — impede reescrita da cadeia ao nível da hive | Baixo (já canonizado, bloco 994) | `verify_integrity()` re-encadeia e compara raiz |
| **Asseveração multisig na Safe** | Média-alta — governança liga a tesouraria à cadeia | Médio (transação) | N-ésima assinatura confirma a raiz |

Mínimo aceitável para despesa: **ledger de âncora ou asseveração multisig**.
TLSNotary é reforço recomendado quando o fornecedor é terceiro não-governado.

### Procedimento de verificação (terceiro, entrada pública)

1. **Obter**: `GET <tor_url>` → lista de entradas canónicas.
2. **Reencadear**: para cada entry `i`, recomputar `hash` com o algoritmo do
   spec; falhar imediatamente em qualquer mismatch com o `prev_hash` recebido.
3. **Verificar monotonia**: `timestamp[i] > timestamp[i−1]` para todo `i > 0`.
4. **Verificar âncora**: o `hash` do head (ou root periódico) deve bater o
   compromisso publicado na âncora (ledger/TLSNotary/Safe). Sem âncora
   publicada → resultado **NÃO-VERIFICÁVEL** (não é INVÁLIDO nem VÁLIDO —
   distinção epistémica exigida pelo protocolo).
5. **Responder**: `VÁLIDO` | `INVÁLIDO` | `NÃO-VERIFICÁVEL`.

O passo 2 apenas prova que a cadeia não foi adulterada *depois* da última
entrada conhecida. O passo 4 é o que amarra o *head* a um facto imutável e
mantém a reescrita do servidor fora do modelo de ameaça.

## Alternativas Consideradas

### A. Status quo — só sequência monotónica (`#001` atual)
- **Prós**: simples; custo zero.
- **Contras**: ordena, não protege; reescrita do servidor não é detetada.
- **Rejeitada**: falha Ghost-1/Loopseal-2 por construção.

### B. Hash chain sem âncora
- **Prós**: tamper-evident dentro da cadeia; barato.
- **Contras**: a cadeia inteira pode ser regenerada por quem controla o
  servidor (a "prova" morre com o accesso).
- **Rejeitada**: dá falsa confiança — melhor não publicar nada do que
  publicar *tamper-evident mas não ancorado* como se fosse prova.

### C. Hash chain + âncora (escolhida)
- **Prós**: deteção de reescrita ao nível do servidor (se o head estiver
  ancorado); alinhada ao `CoherenceLedger` já provado no monorepo; incremental
  (âncora pode ser reforçada de ledger → TLSNotary sem quebrar o formato).
- **Contras**: exige um destino de âncora operado; entradas não-ancoradas
  individuais ficam `NÃO-VERIFICÁVEL` (limite honesto).
- **Escolhida**: único modelo que converge para os invariantes sem exigir
  infraestrutura nova.

### D. Assinatura do fornecedor (contra-assinatura por entrada)
- **Prós**: não-repudiação do fornecedor.
- **Contras**: pressupõe gestão de chaves do fornecedor; a hive não controla
  a rotação; chaves comprometidas anulam tudo. Complementar, não substituto.
- **Rejeitada como primária**; pode ser adicionada como *reforço opcional*
  (campo `signature` na entry).

## Consequências

### Positivas

1. **Entrega observável de facto**: despesas (Hetzner, Palmyr, object store,
   inference) passam a ser verificáveis por terceiro com entrada pública.
2. **Reuso do canonizado**: SHA3-256 e encadeamento já provados no
   `CoherenceLedger` (bloco 994); Gravity-1 nativo já rejeita timestamps
   não-monotónicos.
3. **Incremental**: sem quebrar o formato, a âncora pode ser reforçada
   (ledger → Safe multisig → TLSNotary).
4. **Distinção epistémica formalizada**: `VÁLIDO` / `INVÁLIDO` /
   `NÃO-VERIFICÁVEL` impede que "não provado" seja lido como "provado".

### Negativas

1. **Sem código**: esta spec é o contrato, não a implementação. A hive (ou um
   fornecedor) tem de implementar a emissão e a âncora.
2. **Âncora é requisito**: sem destino de compromisso operado, o procedimento
   devolve `NÃO-VERIFICÁVEL` — a poli garante a honestidade mas não a
   validade.
3. **Adoção depende do fornecedor**: fornecedores existentes (`workers.dev`,
   Hetzner) não afirmam este formato nativamente; a camada ToR é que assina e
   encadeia sobre a resposta bruta deles.

## Riscos e Mitigações

| Risco | Probabilidade | Impacto | Mitigação |
|:---|:---:|:---:|:---|
| Cadeia reescrita sem deteção | Média | Alto | Âncora obrigatória (passo 4); sem âncora → NÃO-VERIFICÁVEL |
| Timestamps manipulados | Média | Médio | Monotonia estrita + comparação com relógio externo na verificação |
| Chave de fornecedor comprometida | Baixa | Médio | Assinatura é reforço opcional, nunca a única prova |
| Fornecedor não adere ao formato | Alta | Médio | A camada ToR da hive encadeia sobre resposta bruta do fornecedor |

## Verificação (plano)

1. **Vetores de teste**: cadeia exemplo de 3 entradas → reencadeamento VÁLIDO;
   flip de `payload` → INVÁLIDO no passo 2; `timestamp` não-monotónico →
   INVÁLIDO no passo 3; head não-ancorado → NÃO-VERIFICÁVEL no passo 4.
2. **Reuso do núcleo Lean**: os invariantes de encadeamento (Ghost-1) e de
   monotonia (Gravity-1) podem ser provados no núcleo Lean 4, à semelhança de
   I511–I516 (campo de coerência) — futuro, quando houver código.
3. **TLA+** (futuro): modelos de reescrita com/sem âncora para demonstrar que
   a âncora fecha o modelo de ameaça.
4. **ADR-002 precedente**: retro-datado como proposta (2026-09-13), revisão em
   2026-12-13 (90 dias) para avaliar adoção real pela hive.

## Relação com Substratos da Catedral

- **565-TLSNOTARY-BRIDGE** — proveniência criptográfica (opção de âncora A).
- **FIELD-STABILITY-COHERENCE** — `CoherenceLedger` como precedente direto de
  SHA3-256 append-only com Gravity-1 nativo (bloco 994, v375.5).
- **972-ARKHE-BITCOIN** — BIP-340/canonical CBOR como referência de formatos
  canónicos verificáveis (não usado aqui; registado como precedente).

## Limitação Honesta

Esta spec **não torna a hive verificável por si**. Ela define o contrato;
a validação só aparece quando houver (a) emissor do formato, (b) âncora
operada, e (c) implementação do procedimento de verificação. Sem qualquer um
dos três, o resultado honesto continua a ser **NÃO-VERIFICÁVEL** — e declarar
o contrário seria a claim forjável que o protocolo proíbe.

---

## Anexo A: Exemplo com vetores

Cadeia de referência (formatada para o teste de reenchamento):

```text
entry 0: prev=0…0            payload={"event":"genesis"}      hash=H0
entry 1: prev=H0             payload={"event":"export_created"} hash=H1
entry 2: prev=H1             payload={"event":"export_hash"}    hash=H2  ← head
```

Flips de teste → resultado esperado:

| Mutação | Passo | Resultado |
|:---|:---:|:---|
| `payload[0]` alterado | 2 | INVÁLIDO (H0 ≠ recomputado) |
| `timestamp[2] ≤ timestamp[1]` | 3 | INVÁLIDO (não-monotónico) |
| H2 não publicado na âncora | 4 | NÃO-VERIFICÁVEL |
| tudo íntegro + H2 ancorado | 2–4 | VÁLIDO |

---

*Selo: ARKHE-ADR-003-2026-09-13 (proposta — aguarda decisão)*