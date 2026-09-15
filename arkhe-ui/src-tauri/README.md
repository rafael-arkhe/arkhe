# `arkhe-ui/src-tauri` — o casco Tauri (Fase 5)

Transforma a app de verificação (`arkhe-ui/dist-app/`) num **aplicativo
instalável**, com uma janela nativa e um comando que lê um modelo GGUF do disco
e o entrega ao núcleo de verificação nativo.

## O que está aqui

| Arquivo | O que é |
| --- | --- |
| `Cargo.toml` | O pacote `arkhe-ui-app` e a sua própria raiz de workspace |
| `build.rs` | `tauri_build::build()` — lê o `tauri.conf.json` e gera o contexto |
| `src/main.rs` | O ponto de entrada do binário |
| `src/lib.rs` | `inspect_gguf_model` (o comando) e `run()` (a janela) |
| `tauri.conf.json` | A configuração da app, no schema do **v2** |
| `capabilities/default.json` | As permissões da janela principal |
| `icons/icon.ico` | **Ícone provisório, não é arte** — ver `icons/PROVISORIO.md` |

## Como correr

A partir de `arkhe-ui/` (é onde vive o `package.json`; o `tauri` CLI corre o
`beforeDevCommand`/`beforeBuildCommand` nesse directório):

```text
npm run tauri:dev     # janela com HMR do Vite
npm run tauri build   # os instaladores
```

O `frontendDist` é `../dist-app` — **o mesmo build da Fase 4**, embutido no
binário em vez de servido. Não há um segundo pipeline de frontend.

## O comando: `inspect_gguf_model`

Recebe o caminho de um arquivo e devolve:

```jsonc
{
  "path": "...",
  "bytes": 217204,
  "digest_hex": "…64 hex…",     // arkhe_verify::gguf::model_digest
  "header": { "ok": true, "magic_ok": true, "available_bytes": 217204,
              "version": 3, "tensor_count": 0, "metadata_kv_count": 0,
              "error": null }     // arkhe_verify::gguf::parse_header
}
```

Duas notas que valem mais do que a assinatura:

1. **O I/O vive no comando, não na biblioteca.** `arkhe-verify` é `&[u8]`-only
   por desenho — sem `Path` e sem I/O, para que o mesmo core compile para wasm
   (`arkhe-verify-wasm`). Este casco é o lado que tem sistema de ficheiros:
   lê **uma vez** e entrega os mesmos bytes às duas funções, para que o digest
   e o cabeçalho não possam descrever arquivos diferentes.
2. **Conteúdo inválido não é erro.** O único `Err` é a falha de leitura. Um
   arquivo que existe e não é GGUF — ou é um GGUF truncado — devolve
   `header.ok: false` com a causa em `header.error`, e o digest calculado de
   qualquer forma.

Leitura não confiável: o `path` vem do frontend. Ver a nota de segurança no
doc-comment de `inspect_gguf_model` em `src/lib.rs` — hoje é aceitável porque o
frontend é o `dist-app/` embutido (sem conteúdo remoto), e deixa de ser se a app
passar a carregar JS de terceiros.

## Identificador

`org.arkhe-os.arkhe`, derivado do que o projecto já declara — não é inventado
nem é o `com.tauri.dev` do template:

- `safe-core-monorepo/Cargo.toml`, `[workspace.package]`:
  `authors = ["Arkhe OS Architects <arkhe@arkhe-os.org>"]` → o domínio é
  `arkhe-os.org` → reverse-DNS `org.arkhe-os` → `org.arkhe-os.arkhe`.

## Ícones: um provisório, e o que isso fecha (e o que não fecha)

Existe **um** ficheiro de ícone: `icons/icon.ico`. É **provisório e não é
arte** — um losango geométrico gerado por aritmética, que existe por uma razão
mecânica e não por uma razão de marca. A declaração completa está em
[`icons/PROVISORIO.md`](icons/PROVISORIO.md), com o layout dos bytes, o sha256 e
como foi validado.

`bundle.icon` está declarado apontando para esse ficheiro:

```json
"bundle": { "icon": ["icons/icon.ico"] }
```

### Medido: o ícone é pré-requisito de *compilação*, não só do empacotador

Ao contrário do que se esperava, `cargo check` **não** passa sem ícones no
Windows. O `build.rs` corre sempre e o `tauri-build` gera um *Windows Resource
file* a partir do `.ico` nesse mesmo passo. O `Err` é incondicional —
`tauri-build-2.6.3/src/lib.rs`, no bloco `if target_triple.contains("windows")`:
o caminho do ícone é `attributes.windows_attributes.window_icon_path`, senão o
primeiro `.ico` de `config.bundle.icon`, senão `"icons/icon.ico"`; e, se esse
caminho não existir, a função devolve `Err` (linhas 608–675). Não há chave de
configuração que dispense o ícone.

| | Sem `.ico` | Com `icons/icon.ico` |
| --- | --- | --- |
| `cargo check` (Windows) | **falhava** (exit 101, no `build.rs`) | **passa (exit 0)**, sem override |
| `tauri build` (Windows) | falhava no mesmo ponto | deixa de falhar **aqui** (não corrido) |

### Medido: o que o `cargo check` realmente prova sobre o ícone

Vale a pena registar, porque condiciona o valor do gate. Não é só "o ficheiro
existe": o `tauri::generate_context!()` **analisa o ícone**. Substituindo-o por
lixo, o check volta a 101:

```text
---- 0 bytes ----
failed to parse icon …/icons/icon.ico: failed to fill whole buffer

---- 270398 bytes de 0xFF (tamanho certo, conteúdo lixo) ----
failed to parse icon …/icons/icon.ico: Invalid reserved field value in ICONDIR (was 65535, but must be 0)
```

A segunda mensagem vem do parser do `ico` crate. Ou seja: **exit 0 é evidência de
estrutura válida, não apenas de presença** — mas continua a não ser evidência de
que o desenho presta. Quem fecha essa parte é o GDI+ (ver `PROVISORIO.md`).

### O que continua aberto

- **A arte.** Substituir o provisório. `npm run tauri icon <fonte.png>` gera o
  conjunto completo a partir de uma imagem quadrada.
- **As outras plataformas.** Só existe o `.ico`. Não há `32x32.png`,
  `128x128.png`, `128x128@2x.png` nem `icon.icns`, portanto `bundle.icon` lista
  um ficheiro que serve o Windows e **não** serve Linux/macOS. Quando a arte
  existir, a lista deve crescer para os incluir.

### Nota: o `TAURI_CONFIG` como instrumento de diagnóstico (histórico)

Antes de haver ícone, o check só podia ser corrido injectando um `.ico` de outro
programa por variável de ambiente — o `TAURI_CONFIG` é fundido sobre o
`tauri.conf.json` como *merge patch* RFC 7386 (`tauri-build/src/lib.rs:487`):

```bash
TAURI_CONFIG='{"bundle":{"icon":["C:/Windows/ACU.ico"]}}' cargo check --all-targets
```

Isso **já não é preciso**: com `icons/icon.ico` presente, `cargo check` passa
limpo. O comando fica registado porque foi assim que o bloqueio foi isolado, e
não como configuração do projecto — produzia um binário cujo ícone de recurso era
o de outro programa.

## Versões

Medidas no registo (`cargo info`, `npm view`), não presumidas:

| Crate / pacote | Versão | Porquê |
| --- | --- | --- |
| `tauri` | `2.11.5` | o stable; `3.0.0-alpha.0` existe e **não** é usado |
| `tauri-build` | `2.6.3` | o par do `tauri` 2.11.5 |
| `tauri-cli` (npm) | `2.11.4` | casa com o `tauri-cli` do registo |
| `@tauri-apps/api` (npm) | `2.11.1` | o `latest` do registo; é o lado JS do IPC (`invoke`) |

## Workspace próprio

O `Cargo.toml` da raiz deste repositório declara um `[workspace]` (o do
`kernel`) que **não** inclui `arkhe-ui/src-tauri`. O cargo sobe a árvore,
encontra esse workspace e recusaria este pacote como membro implícito. A tabela
`[workspace]` (vazia) no `Cargo.toml` deste directório faz dele a raiz do seu
próprio workspace — que é o que uma app Tauri é — e mantém o `Cargo.lock` da app
separado do lock da raiz.
