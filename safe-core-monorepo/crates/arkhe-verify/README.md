# arkhe-verify

§2.3 do plano Arkhe OS — a **casca nativa** sobre o core de verificação do
[`arkhe-verify-wasm`](../arkhe-verify-wasm), com o modelo de dados no formato
Rekor/CT e um adaptador HTTP para uma instância Rekor.

## Um core, duas cascas

A verificação **não** é reimplementada aqui. Ela vive no core do
`arkhe-verify-wasm` — que declara `crate-type = ["cdylib", "rlib"]` justamente
para ter duas cascas —, e este crate depende dele como **`rlib`**:

```
                       arkhe-verify-wasm
                       ├── hash::verify_sha256_inner
                       ├── signature::verify_signature_inner
                       ├── merkle::verify_inclusion_inner
                       ├── quorum::verify_witness_quorum_inner
                       └── attestation::verify_attestation_inner
                                │                │
         casca #[wasm_bindgen] ─┘                └─ casca nativa
         (wasm.rs: bool / JSON)                     (facade.rs: relatórios)
```

**O ponto exato do reuso** é `src/facade.rs`: cada uma das cinco funções chama
`arkhe_verify_wasm::*_inner` — a mesma chamada que o `wasm.rs` faz. Não existe
nesta crate uma segunda implementação de SHA-256, de Ed25519, do RFC 6962 nem da
contagem de quórum; nem de hex ou base64, que também vêm do core
(`arkhe_verify_wasm::encoding`, reexportado como `arkhe_verify::encoding`).

| Verificação | Casca nativa | Casca wasm | Core (uma só) |
|:---|:---|:---|:---|
| SHA-256 | `verify_sha256` → `Sha256Report` | `bool` | `hash::verify_sha256_inner` |
| assinatura | `verify_signature` → `SignatureReport` | `bool` | `signature::verify_signature_inner` |
| inclusão | `verify_inclusion` → `InclusionReport` | `bool` | `merkle::verify_inclusion_inner` |
| quórum | `verify_witness_quorum` → `QuorumReport` | `bool` | `quorum::verify_witness_quorum_inner` |
| pipeline | `verify_attestation` → `AttestationReport` | `String` | `attestation::verify_attestation_inner` |

O que a casca nativa acrescenta:

- **Relatórios em vez de `bool`.** O veredito vem com os dados contra os quais
  foi produzido — inclusive `valid_witnesses` no quórum e `trusted` na
  assinatura, que um `bool` não carrega e que obrigariam quem chama a refazer a
  comparação para descobrir o que falhou.
- **Tipos em vez de strings.** O trust root é um `TrustRoot` já interpretado.
  Consequência que vale registrar: um trust root malformado **não tem como
  chegar** a `verify_signature` nem a `verify_witness_quorum` — não existe valor
  de `TrustRoot` malformado. A casca wasm precisa tratar isso como caso (e
  trata); aqui o tipo elimina o caso.

## Modelo Rekor/CT (`src/rekor/`)

| Tipo | Para quê |
|:---|:---|
| `LogEntry` (`log_index`, `body`, `integrated_time`, `log_id`, `verification`) | uma entrada do log; `LogEntry::inclusion_report()` monta `(folha, leaf_index, tree_size, prova, raiz)` e delega ao core |
| `InclusionProof` (`logIndex`, `rootHash`, `treeSize`, `hashes`) | a prova de inclusão; `proof_bytes()` produz a concatenação crua dos digests irmãos que o core consome |
| `Checkpoint` / `SignedCheckpoint` | o checkpoint e as **witness signatures**; `witness_count()`, e `quorum_report()` monta `(subject, witnesses)` e delega ao core |
| `ConsistencyProof` | a prova de consistência |

`InclusionProof::root_hash` é **hex** e `Checkpoint::root_hash` é **base64** —
dois formatos da mesma API (hex no JSON do Rekor, base64 na convenção de
*signed note*). Não é deslize de transcrição, e misturá-los produziria uma
comparação entre codificações diferentes.

O `key id` de um witness tem 4 bytes e **não é** a chave pública: o de-para vive
num `WitnessKeyring` explícito, porque em CT ele mora numa lista de chaves
distribuída fora da note, e inventar uma resolução aqui seria inventar uma raiz
de confiança.

## Modelo GGUF (`src/gguf.rs`)

O caminho crítico: verificar um **modelo** em bytes.

| Função | O que responde |
|:---|:---|
| `parse_header(&[u8])` → `GgufHeaderReport` | o cabeçalho é estruturalmente válido? magic `GGUF`, versão interpretável (2 ou 3), contagem de tensores e de pares chave-valor |
| `model_digest(&[u8])` → `String` | qual é o digest SHA-256 destes bytes (para publicar num manifesto) |
| `verify_model_digest(&[u8], esperado_hex)` → `GgufModelReport` | o digest confere com o esperado? **Reusa `facade::verify_sha256`** |
| `verify_model_attestation(&[u8], json, trust_root)` → `GgufAttestationReport` | uma atestação existente liga **este** modelo ao log? **Delega a `facade::verify_attestation`** |

**A API é sobre `&[u8]`, nunca sobre `Path`.** Não há `std::fs` no módulo: quem
lê o arquivo é quem chama (`examples/conferir_gguf.rs` é um chamador de
exemplo). O crate **inteiro** passa em `cargo check --target
wasm32-unknown-unknown` — não por acaso, mas porque nenhuma parte da
verificação toca o sistema de arquivos.

Três recusas são **relatórios com causa**, nunca `Err`: arquivo truncado, magic
inválido e versão não interpretada. O leitor é incremental, então um arquivo de
16 bytes reporta a versão e o primeiro contador que estavam presentes e `None`
no que faltou. Os contadores são lidos como `i64` e um valor negativo é
recusado, como no leitor de referência do `llama.cpp`; uma versão com a metade
alta preenchida é diagnosticada como *endianness* trocada.

Duas fronteiras, ditas em vez de contornadas:

- **Cabeçalho válido não é modelo válido.** `parse_header` lê 24 bytes de
  estrutura e não olha os pares chave-valor, os descritores de tensor nem o
  bloco de dados. Quem identifica os bytes é o **digest**; o cabeçalho é o
  diagnóstico que explica uma recusa.
- **A atestação liga o digest pelo payload.** O *subject* do core amarra
  `SHA-256(payload)`, e não existe campo para "o digest do artefato" distinto
  do payload — então a rota só vale quando o **payload da atestação é o próprio
  modelo**. Uma entrada de log cujo corpo seja um manifesto que *menciona* o
  digest responde negativo, e é o correto. Verificar essa outra forma exigiria
  um subject novo, com domínio próprio, e isso pertence ao core, não a esta
  casca.

## Adaptador HTTP e rede

**Só `RekorClient` faz rede**, e só os dois métodos dele. Todo o resto é puro
sobre dados já obtidos.

| Método | Requisição |
|:---|:---|
| `log_entry(log_index)` | `GET {base}/api/v1/log/entries?logIndex={n}` (a resposta é um mapa de UUID; o cliente desembrulha o único elemento) |
| `consistency_proof(first, last, tree)` | `GET {base}/api/v1/log/proof?firstSize=&lastSize=&treeSize=` |

URL base padrão: **`https://rekor.sigstore.dev`** (a instância pública do
Sigstore), configurável por `RekorClient::with_base_url`.

**Nenhum teste faz chamada de rede viva.** O adaptador é exercitado contra um
servidor **local** (`mockito`), como o `arkhe-orcid` faz; a instância pública é
um valor de configuração documentado, não um endereço que este crate já tenha
consultado.

`consistency_proof` **busca** a prova e **não a verifica**: o core não tem
verificador de consistência, e escrever um aqui seria duplicar lógica de
verificação fora do core. Também não é verificada a `signedEntryTimestamp`.

## Pins

```toml
reqwest      = { version = "0.13.4", default-features = false, features = ["json", "rustls"] }
serde        = { workspace = true }
serde_json   = { workspace = true }
thiserror    = { workspace = true }

[dev-dependencies]
tokio        = { workspace = true, features = ["rt", "macros", "sync"] }
mockito      = "1.7.2"
ct-merkle    = "0.3.0"
sha2         = "0.11"
ed25519-dalek = { workspace = true }
base64       = "0.22"
```

`reqwest` e `mockito` seguem os pins do `arkhe-orcid` (o outro cliente HTTP do
workspace) e **não** estão em `[workspace.dependencies]`, então a versão é
fixada aqui.

### Duas divergências dos pins pedidos, registradas

1. **`hex` não está instalado, e `base64` só como dependência de
   desenvolvimento.** O pedido listava os dois. Acontece que a biblioteca não
   precisa de nenhum: o core já expõe `arkhe_verify_wasm::encoding`
   (`decode_hex`, `encode_hex`, `decode_fixed_hex`, `decode_base64`), e reusar
   esses codificadores é exatamente o que a arquitetura de "um core, duas
   cascas" manda fazer. Instalar um segundo stack de hex/base64 ao lado do que o
   core já tem seria peso morto e uma segunda chance de divergir na
   codificação. O `base64` entra como dev-dependency por um motivo específico: o
   core expõe o **decodificador**, não o codificador, e os testes precisam
   *codificar* para montar fixtures.

2. **`tokio` só como dev-dependency.** A biblioteca não chama nenhuma API do
   tokio — o `reqwest` traz o próprio runtime e os testes é que precisam de um
   executor (`#[tokio::test]`, `mockito`). Mesma escolha do `arkhe-orcid`.

## `sigstore` — rota opcional, **não instalada**

`sigstore-rs` **não está instalado**, e deliberadamente. Se a rota for adotada,
o pin é:

```toml
sigstore = { version = "0.13", features = ["wasm"] }
```

Restrições a registrar:

- A feature `wasm` foi **removida na 0.14**, então a única versão utilizável é a
  **0.13**. `sigstore` 0.14+ não serve para WASM.
- **`wasm32-wasi` não é suportado.** O alvo do core é `wasm32-unknown-unknown`.

Esta crate não depende de `sigstore`: ela cobre SHA-256, Ed25519, inclusão Merkle
RFC 6962 e quórum de witnesses com as primitivas do core. Uma atestação Sigstore
completa (Rekor, Fulcio, bundles) exigiria a crate opcional — o que este crate
tem é o **formato** do Rekor e um adaptador para buscar as entradas, não a
verificação de bundles.

## Testes

```
cargo test -p arkhe-verify
```

48 testes de unidade, 11 de integração (o adaptador contra o mockito) e 1 de
documentação. Incluem vetores conhecidos de SHA-256; prova de inclusão de
verdade (5 folhas, montada com `ct-merkle`, o mesmo pin do core); quórum com 2 e
com 1 witness, chave fora do trust root, limiar abaixo do mínimo e lista
duplicada; parsing de note (corpo, `tree_size` não numérico, corpo com quatro
linhas, prefixo ausente, blob curto, base64 inválido); keyring (validação de
tamanho dos dois lados); o caminho Rekor → modelo → core de ponta a ponta; e o
mapeamento de status HTTP (404, 429, 5xx, outro, corpo não-JSON, mapa vazio, mapa
com duas entradas).

### Uma armadilha do mockito

Sem `match_query`, o mockito casa o **caminho e a consulta inteiros** como
string exata. Como toda requisição desta crate carrega consulta, um mock
declarado só com o caminho **nunca casa**, e a resposta que chega é um
`501 Not Implemented` do mockito — que na primeira leitura parece um erro do
cliente. Os testes ou casam a consulta explicitamente, ou declaram
`Matcher::Any`; nenhum depende do padrão. Este erro foi observado e corrigido
durante a implementação, não previsto.

## Não verificado nesta máquina

- **Nenhuma chamada de rede viva ao Rekor foi feita.** §5 acima: o adaptador só
  foi exercitado contra `mockito` em `127.0.0.1`.
- **O modelo Rekor foi construído a partir da descrição da API, não de uma
  instância real.** Os nomes de campo (`logIndex`, `logID`, `integratedTime`,
  `verification.inclusionProof`, `rootHash`, `treeSize`, `hashes`) seguem a
  descrição pública; nenhum corpo de resposta real foi capturado e comparado.
- **O formato do checkpoint assinado é uma leitura da convenção**, não uma
  verificação dela: (a) o que é assinado é o corpo da note incluindo o `\n`
  final; (b) o `key id` de 4 bytes viaja concatenado à assinatura dentro do
  mesmo base64. O modo de falha é seguro — um formato diferente produz um
  `key id` que não está no keyring, logo `UnknownWitnessKey`, não um veredito
  errado — mas não é o mesmo que estar certo.
- **A `signedEntryTimestamp` não é verificada**, e a **prova de consistência não
  é verificada** (só buscada).
- **Nenhum bundle Sigstore/Fulcio** foi exercitado: `sigstore` não está
  instalado, por instrução.
