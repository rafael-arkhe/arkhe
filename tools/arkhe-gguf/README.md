# tools/arkhe-gguf — pipeline do `arkhe.gguf`

Ferramentas do pipeline `safetensors → GGUF → sign → extend → anchor` para o
artefacto `arkhe.gguf` (modelo GGUF com metadados de atestação
`arkhe.attestation.*`).

## Estado: o que corre hoje e o que não corre

| Ficheiro | Estado | Porquê |
|:---|:---|:---|
| `fixture.py` | **executável** | Só usa a biblioteca padrão (`hashlib`, `struct`, `pathlib`). Gera a fixture determinística de 24 bytes. |
| `convert.sh` | **não executável aqui** | Depende de um modelo base HF (decisão pendente do Arquiteto) e de um clone local do `llama.cpp`. Descarrega nada por si, mas nada neste repositório o satisfaz. |
| `extend_metadata.py` | **não executável aqui** | Depende do módulo Python `gguf` (`pip install gguf`) e de um GGUF assinado de entrada — nenhum dos dois existe no repositório. |
| `sign.sh` | **não executável aqui** | Assinatura *keyless*: requer identidade OIDC, contacto com Fulcio e com o Rekor. Não há identidade nem autorização para o fazer deste lado. |

Ver também `MODEL_CARD.md` (campos `<...>` por preencher: modelo base,
quantização e trust root, as três decisões pendentes do Arquiteto),
`capabilities_manifest.json` e `provenance.json`.

## Dependências por ferramenta

### `fixture.py`
- Python 3 (testado com 3.13.12). Biblioteca padrão apenas.
- `blake3` (opcional): `pip install blake3`. Sem ele, o script imprime
  `[instalar: pip install blake3]` em vez de um digest — nunca um valor falso.

Gera `arkhe.gguf` no diretório de trabalho atual (24 bytes: magic `GGUF`,
versão 3, 0 tensores, 0 pares chave-valor) e imprime o hex e os digests.

```sh
python3 tools/arkhe-gguf/fixture.py
```

### `convert.sh`
- `python3` no `PATH`.
- Um clone local de `llama.cpp` **no diretório de trabalho atual**
  (`llama.cpp/convert_hf_to_gguf.py`): `git clone https://github.com/ggerganov/llama.cpp`.
- `pip install -r llama.cpp/requirements.txt`.
- Um diretório de modelo HF já descarregado, passado como argumento.

```sh
tools/arkhe-gguf/convert.sh <model_dir> [outfile] [outtype]
```

### `extend_metadata.py`
- `pip install gguf` (módulo `gguf`: `GGUFReader`, `GGUFWriter`).
- Um GGUF de entrada. O script reescreve **todos** os pares chave-valor
  existentes e acrescenta os `arkhe.attestation.*`.

```sh
python3 tools/arkhe-gguf/extend_metadata.py <input.gguf> <output.gguf>
```

Nota: o `__main__` traz *placeholders* (`<hash base>`, `<log_id>`,
`<base64>`, `size_bytes: 0`) — são para preencher com os valores reais do
artefacto, não valores medidos.

### `sign.sh`
- `pip install model-signing` (OpenSSF Model Signing v1.0).
- Identidade OIDC com um *identity provider* reconhecido pelo Fulcio
  (o script usa `token.actions.githubusercontent.com`, isto é, CI do GitHub
  Actions) e acesso de rede ao Fulcio e ao Rekor.
- `git config user.email` definido.

```sh
tools/arkhe-gguf/sign.sh <model.gguf>
```

## O que este pipeline **não** é

Os digests da fixture de 24 bytes (em
`safe-core-monorepo/crates/arkhe-verify/fixtures/arkhe.gguf`) são **da
fixture**, não do modelo final: um GGUF mínimo com zero tensores e zero pares
chave-valor, usado para exercitar o leitor de cabeçalho de
`arkhe_verify::gguf` e para fixar o formato dos 24 bytes. Nenhuma assinatura,
nenhuma ancoragem no Rekor e nenhum modelo real estão envolvidos.

A fixture vive em `crates/arkhe-verify/fixtures/` (não em `tools/`) porque é o
teste `tests/gguf_fixture.rs` do crate que a lê, por caminho relativo à raiz do
crate. O `fixture.py` escreve no diretório de trabalho atual; para a
reconstruir no lugar certo:

```sh
cd safe-core-monorepo/crates/arkhe-verify/fixtures && python3 ../../../../tools/arkhe-gguf/fixture.py
```
