# arkhe-wormgraph

§1.5 do plano Arkhe OS — o **grafo causal**. Um log append-only de nós e
arestas tipados, com hash encadeado BLAKE3, no mesmo espírito do `AuditLog` de
`arkhe-governance`.

## Especificação §1.5

| Tarefa | Descrição | Critério | Onde |
|:---|:---|:---|:---|
| Nós e arestas | `Node`, `Edge` com tipo e timestamp | Serialização JSON | `src/types.rs`, testes `*_round_trips_through_json` |
| Consulta | Filtros por tipo, signatário, invariante | Teste de consulta | `WormGraph::nodes_by_type` / `nodes_by_signer` / `nodes_by_invariant`, `NodeFilter`, testes `query_*` |
| Imutabilidade | Append-only | Hash encadeado | `append_node` / `append_edge` sem remoção nem edição; `chain_hash()`, `verify_chain()` |

## Uso

```rust
use arkhe_wormgraph::{Edge, Node, WormGraph};

let mut graph = WormGraph::new();

let alpha = Node::new("alpha", "invariant.check", 1_700_000_000_000)
    .with_signer("witness-a")
    .with_invariant("INV-009")
    .with_payload(serde_json::json!({ "verdict": "pass" }));
graph.append_node(alpha)?;
graph.append_node(Node::new("beta", "artifact", 1_700_000_000_001))?;
graph.append_edge(Edge::new("alpha", "beta", "depends_on", 1_700_000_000_002))?;

// Consultas devolvem iteradores — sem alocação intermediária.
let checks: Vec<&str> = graph.nodes_by_type("invariant.check")
    .map(|node| node.id.as_str())
    .collect();
assert_eq!(checks, vec!["alpha"]);

// A cadeia inteira é recomputável e conferível.
graph.verify_chain()?;
```

## Modelo

- **`Node` / `Edge` são dados puros.** Não carregam hash nem posição — o
  encadeamento vive no envelope `WormEntry`. Assim um nó pode ser serializado,
  comparado e transportado sem arrastar consigo a posição que ocupava numa
  cadeia específica.
- **Nós e arestas numa única cadeia.** A ordem causal entre um nó e a aresta
  que o liga a outro é justamente o que a cadeia precisa preservar, então não
  há duas cadeias separadas.
- **Não há `&mut` para as entradas.** `entries` é privado; entradas só nascem
  de `append_node` / `append_edge`. Como em `arkhe-governance`, `from_entries`
  e `Deserialize` constroem **sem validar** — a integridade é estabelecida por
  `verify_chain()`, e é isso que torna a adulteração *detectável* em vez de
  apenas impossível de escrever por esta API.
- **A rejeição não suja a cadeia.** `append_node` recusa `id` duplicado
  (`DuplicateNode`) e `append_edge` recusa pontas inexistentes
  (`UnknownNode`); nos dois casos nada é anexado e o `head_hash` não muda.

### Integridade do encadeamento

```
hash(sequence, prev_hash, entrada) = BLAKE3(
    "arkhe-wormgraph/entry/v1" ∥ sequence_be ∥ prev_hash ∥ tag ∥ conteúdo_canônico
)
```

- **Separador de domínio**: versiona o formato — mudá-lo muda todos os hashes,
  então este hash não pode ser confundido com o de outro protocolo.
- **Prefixo de comprimento em toda string**: sem ele, `("ab","c")` e
  `("a","bc")` produziriam os mesmos bytes e o mesmo hash.
- **Tag explícita de nó/aresta** (mais o *shape* dos campos) impede que as duas
  formas colidam.
- **`signer: Option<String>`** distingue "não assinado" de "assinado por `""`";
  o hash marca a ausência com um byte diferente do prefixo de presença.

`verify_chain()` checa, por posição: ligação (`prev_hash`) → `sequence` →
hash recomputado. A ligação vem primeiro porque uma remoção ou troca de posição
deixa a entrada internamente coerente mas apontando para o predecessor errado
— é isso que está quebrado, e `LinkBroken` diz o que de fato aconteceu.
`SequenceMismatch` fica reservado para o caso em que o campo `sequence` foi
forjado sem mexer na ligação.

## Reaproveitamento de `arkhe-core`

O hash é `arkhe_core::hash::blake3_hash` sobre `arkhe_core::ArkheHash`
(`[u8; 32]`) — a mesma primitiva que `arkhe-core` e `arkhe-evidence` já usam.
**Não há dependência direta de `blake3` neste crate.**

O `CanonicalEncoder` de `arkhe-governance` **não** foi reaproveitado: é
`pub(crate)` àquele crate e não é alcançável daqui. A codificação canônica é
reimplementada localmente (domínio + prefixos de comprimento + tag de tipo).

## Ponte para `arkhe-evidence`

O módulo `src/evidence.rs` liga o grafo ao log de evidências encadeado
(`arkhe-evidence`), espelhando o precedente `provenance_graph` do
`arkhe-geometric-verifier`:

| Função | O que faz |
|:---|:---|
| `build_from_evidence_chain(&EvidenceChain) -> Result<WormGraph, _>` | um `Node` por `EvidenceRecord` (com `index`, `timestamp` e o par `hash`/`prev_hash` **preservado** no payload do nó), e uma aresta por par de registros consecutivos |
| `verify_against_chain(&WormGraph, &EvidenceChain)` | cruza as duas verificações — o `WormGraph::verify_chain()` e o `EvidenceChain::verify_chain()` — e depois confere que o grafo representa aquela cadeia |

O payload do registro **não** é copiado para dentro do nó, e sim o seu
comprimento: o payload já vive na cadeia, e uma segunda cópia poderia discordar
da primeira.

O cruzamento pega duas coisas que o `EvidenceChain` sozinho **não** pega: o hash
de um `EvidenceRecord` cobre `(prev_hash, payload)`, portanto editar o
`timestamp` ou o campo `index` de um registro deixa a cadeia de evidências
verificando. O grafo preserva esses campos do momento da construção, então a
divergência aparece (`TimestampMismatch`, `RecordIndexMismatch`).

### Modelo de execução

O `WormGraph` continua **síncrono**; a assincronia fica contida na ponte, porque
`EvidenceChain` é assíncrona (`tokio::sync::RwLock`). As duas funções acima são
`async` — mas só para aguardar a cadeia: tudo o que fazem com o grafo é
síncrono. O tokio é dependência **de desenvolvimento** (só os testes precisam de
executor); a biblioteca não o usa diretamente, e nem a lógica de hash do
`arkhe-evidence` é duplicada — o wormgraph continua chamando
`arkhe_core::hash::blake3_hash`, e a ponte apenas grava com
`arkhe_core::hash::hash_to_hex` os hashes que o `arkhe-evidence` já calculou.

## Testes

```
cargo test -p arkhe-wormgraph
```

Cobrem round-trip JSON de `Node`/`Edge`/grafo, ligação ao genesis, adulteração
de cada campo (payload, signer, invariantes, timestamp), remoção, reordenação,
forja de `sequence`, as quatro consultas e o filtro conjuntivo, rejeição de
duplicata e de ponta inexistente, e as propriedades do `chain_hash`
(determinismo, sensibilidade, ausência de colisão de fronteira de campo).

Da ponte: cadeia vazia, 1 registro, 5 registros, encadeamento preservado,
carimbo/índice de cada aresta, round-trip JSON do grafo construído, detecção de
adulteração (hash, carimbo, remoção de entrada, remoção de aresta, aresta
religada, ponta fora do subgrafo), o caso que **só** o cruzamento pega (grafo e
cadeia íntegros, mas de cadeias diferentes) e a recusa de um grafo misto que
apenas convive com outros tipos de entrada.
