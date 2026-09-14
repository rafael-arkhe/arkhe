# arkhe-verify-wasm

§2.1–2.3 do plano Arkhe OS — **verificação client-side que carrega no
navegador**. Compila para `wasm32-unknown-unknown` e expõe, via
`wasm-bindgen`, a API com que uma página confere uma atestação **sem confiar
no servidor que a produziu**.

## Especificação §2.1

| Função | Descrição | Critério de aceitação |
|:---|:---|:---|
| `verify_sha256` | Hash SHA-256 | Teste com vetor conhecido (`sha256("abc")`, `sha256("")`) |
| `verify_signature` | Ed25519 contra um trust root | Chave válida **e** inválida; chave correta fora do trust root |
| `verify_inclusion` | Prova de inclusão Merkle RFC 6962 | Reconstrução da raiz + vetores conhecidos do RFC 6962 |
| `verify_witness_quorum` | Quórum ≥ 2 | 2 witnesses (passa) e 1 witness (falha) |
| `verify_attestation` | Pipeline completo compondo as quatro | 22 testes unitários (§ abaixo) |

Mais três auxiliares de integração: `sha256_hex`, `merkle_root_hex`,
`attestation_subject_hex`.

## Build

```bash
# O alvo precisa estar instalado:
rustup target add wasm32-unknown-unknown

# Verificação de que compila para WASM (o teste que importa):
cargo check --target wasm32-unknown-unknown -p arkhe-verify-wasm

# Testes nativos (a lógica é Rust normal; ver `crate-type` abaixo):
cargo test -p arkhe-verify-wasm

# O .wasm + glue JS:
cargo build --release --target wasm32-unknown-unknown -p arkhe-verify-wasm
wasm-bindgen --target web --out-dir pkg \
  target/wasm32-unknown-unknown/release/arkhe_verify_wasm.wasm
```

`crate-type = ["cdylib", "rlib"]`: o `cdylib` é o artefato do navegador, o
`rlib` é o que permite rodar os testes no host.

## API (§2.1)

Todas as funções recebem e devolvem tipos amigáveis (`String`, `&[u8]`,
`u32`, `bool`). **Nenhuma lança**: entrada malformada devolve `false`, ou um
relatório JSON com `ok: false` e a causa — um verificador que lança sobre bytes
não confiáveis transfere ao chamador a tarefa de distinguir "entrada inválida"
de "bug no verificador", e no navegador isso vira `Uncaught`.

```js
import init, { verify_sha256, verify_signature, verify_inclusion,
                verify_witness_quorum, verify_attestation } from "./pkg/arkhe_verify_wasm.js";
await init();

verify_sha256(new TextEncoder().encode("abc"),
  "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"); // true

verify_signature(message, signature, publicKeyHex, JSON.stringify([trustedKeyHex]));

verify_inclusion(leaf, leafIndex, treeSize, proofBytes, rootHex);

verify_witness_quorum(subject, JSON.stringify(witnesses), trustRootJson, 2);

const report = JSON.parse(verify_attestation(attestationJson, trustRootJson));
// {"ok":true,"sha256":true,"signature":true,"inclusion":true,"quorum":true,"error":null}
```

**Atenção ao `bigint`:** os parâmetros `u64` (`leaf_index`, `tree_size` em
`verify_inclusion`) são expostos ao JavaScript como `bigint`, não `number` —
é assim que o `wasm-bindgen` mapeia `u64`. Passe `0n`, não `0`. `threshold`
é `u32` e continua `number`.

### Convenções de codificação

- **hex** para valores de tamanho fixo e curto: chaves públicas (32 bytes),
  assinaturas (64), raízes Merkle (32), digests (32), provas (n × 32).
- **base64** (standard, com padding) para o payload, que é arbitrário e pode
  ser binário.

### `verify_attestation`: o subject

O que é assinado não é o payload cru, e sim um *subject* que amarra o
enunciado inteiro:

```
"arkhe-attestation/v1" ∥ SHA-256(payload) ∥ raiz Merkle ∥ leaf_index ∥ tree_size
```

Todos os campos são de tamanho fixo, então a concatenação é unívoca sem
prefixos de comprimento. Duas consequências deliberadas:

- O subject usa o digest **calculado** do payload, não o digest declarado no
  JSON. Se divergirem, a etapa `sha256` falha; usar o calculado garante que,
  quando a assinatura verifica, ela cobre os bytes reais.
- Assinar o subject (e não o payload) faz a assinatura cobrir também a
  **posição** e a **raiz**. Isso é necessário porque — propriedade documentada
  do RFC 6962, capturada no teste
  `an_inclusion_proof_does_not_by_itself_pin_the_tree_size` — uma prova de
  inclusão pode verificar para mais de um tamanho de árvore. A prova sozinha
  não fixa o tamanho; o subject assinado fixa.

Os quatro estágios são avaliados e reportados **independentemente**, para que
uma falha parcial seja diagnosticável (ex.: `quorum: false` com os outros três
`true`). `attestation_subject_hex` existe para que o signatário JavaScript
monte os bytes exatos sem reimplementar a concatenação — uma segunda
implementação seria uma segunda chance de divergir.

## Pins (§2.1) — e uma divergência forçada

```toml
wasm-bindgen  = "0.2"     # OK
sha2          = "0.11"    # ⚠️ DIVERGÊNCIA: o plano pedia "0.10"
ed25519-dalek = "2.1"     # OK (resolve para 2.2.0)
base64        = "0.22"    # OK
hex           = "0.4"     # OK
serde         = "1.0"     # OK
serde_json    = "1.0"     # OK
thiserror     = "1.0"     # OK
ct-merkle     = "0.3.0"   # OK
```

### Por que `sha2 = "0.11"` e não `"0.10"`

**Os dois pins do plano são mutuamente incompatíveis.** `ct-merkle` 0.3.0
depende de `digest = "0.11"`; `sha2` 0.10 implementa `digest = "0.10"`.
`digest` 0.10 e 0.11 são incompatíveis em semver, então o Cargo liga as duas
versões como crates distintos no grafo e o `Sha256` de `sha2` 0.10 **não**
satisfaz a bound `H: digest::Digest` do `ct-merkle`. Confirmado por compilação,
não por inferência:

```
error[E0277]: the trait bound `Sha256: ct_merkle::digest::Digest` is not satisfied
  = note: there are multiple different versions of crate `digest` in the dependency graph
```

Escolha feita: alinhar em `sha2 = "0.11"`, mantendo o `ct-merkle` (o pin
especializado, e o que traz o RFC 6962 auditado) em vez de reimplementar
Merkle à mão. `sha2` 0.11 produz o mesmo SHA-256 de `sha2` 0.10.

Nota que reduz o custo desta escolha: **`sha2` 0.10 continua no grafo de
qualquer forma**, porque `ed25519-dalek` 2.2 → `ed25519` 2.x → `sha2` 0.10. As
duas versões estão presentes independentemente da escolha; o que muda é apenas
qual delas *este* crate chama. (A alternativa de fixar `sha2 = "0.10"` e
adicionar um alias de 0.11 só para o `ct-merkle` teria o mesmo custo de código,
com duas fontes de verdade para SHA-256.)

### Features para WASM

Nenhum ajuste de feature foi necessário. `ct-merkle` 0.3.0 é `#![no_std]` com
`default = []` e não puxa RNG; `ed25519-dalek` mantém `std` (que o alvo
`wasm32-unknown-unknown` fornece) e só usa RNG para **assinar**, não para
verificar. Como esta crate só verifica, não há `getrandom` no caminho.

### `sigstore` — rota opcional (§2.3)

`sigstore-rs` **não está instalado**, e deliberadamente: a feature `wasm` foi
**removida na 0.14**, então a única versão utilizável é a **0.13**.

Se a rota for adotada, o pin é:

```toml
sigstore = { version = "0.13", features = ["wasm"] }
```

Restrições a registrar:

- **`wasm32-wasi` não é suportado.** O alvo é `wasm32-unknown-unknown`.
- `sigstore` 0.14+ não serve para WASM.
- Esta crate **não** depende de `sigstore`; ela cobre SHA-256, Ed25519 e
  inclusão Merkle RFC 6962 com as primitivas acima. Uma atestação Sigstore
  completa (Rekor, Fulcio, bundles) exigiria a crate opcional.

## Testes

```
cargo test -p arkhe-verify-wasm
```

81 testes nativos, dos quais **22 em `verify_attestation`** (o plano pedia 8):
caminho feliz; cada estágio falhando isoladamente (sha256, signature,
inclusion, quorum); prova adulterada; índice errado; payload fora da árvore;
raiz errada; quórum ausente / acima do número de witnesses / abaixo do mínimo;
JSON malformado; campo ausente; base64 inválido; raiz hex inválida; trust root
vazio; forma do relatório; e as propriedades do subject.

### Vetores conhecidos e oráculos independentes

O `ct-merkle` **não** é tratado como fonte de verdade. A raiz é conferida
contra duas implementações escritas de forma independente:

1. Uma implementação do RFC 6962 §2.1 no módulo de teste (`rfc6962`), só com
   `sha2` — prefixos `0x00`/`0x01`, promoção do nó ímpar, `k` = maior potência
   de 2 menor que `n`.
2. Os **vetores de teste padrão do RFC 6962**
   (`certificate-transparency-go`, 8 folhas), usados como vetores conhecidos
   tanto para as raízes (`MTH` de n = 1..8) quanto para as provas de inclusão
   de cada folha. Esses valores foram calculados de forma independente em
   Python (`hashlib.sha256`) antes de virarem constantes no teste.

Este arranjo pegou um erro real: eu havia anotado `6e340b9c…` como sendo
`SHA-256(0x00 ∥ "abc")`, quando é o hash da folha **vazia** — `SHA-256(0x00)`.
`SHA-256(0x00 ∥ "abc")` é `609f6e36…`. A implementação estava correta; a
expectativa do teste é que estava errada.

## Não verificado nesta máquina

- **Execução no navegador.** O crate compila para `wasm32-unknown-unknown`
  (`cargo check`, exit 0) e o `wasm-bindgen` CLI está presente, mas o `.wasm`
  não foi carregado nem exercitado em um runtime JS/WASM.
- **Atestação Sigstore real** — `sigstore` não está instalado (por instrução).
