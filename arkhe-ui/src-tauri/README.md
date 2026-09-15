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

## Lacuna registada: ícones

O **v2 espera `bundle.icon`** (um `.ico` para Windows, `.icns` para macOS, PNGs
para Linux). **Não existe nenhum desses arquivos neste repositório**, e nenhum
foi fabricado: gerar um `.ico` a partir do nada seria pôr um binário inventado
no lugar de um recurso de marca que alguém tem de desenhar.

Por isso a chave `bundle.icon` está **ausente** do `tauri.conf.json` (não
apontada para arquivos inexistentes — isso seria uma configuração que mente).

### Medido, e contra a expectativa

A expectativa era que `cargo check` passasse sem ícones e que só o bundle
precisasse deles. **Não é o que acontece no Windows.** O `build.rs` corre
sempre, e o `tauri-build` gera um *Windows Resource file* a partir do `.ico`
nesse mesmo passo:

```text
`icons/icon.ico` not found; required for generating a Windows Resource file during tauri-build
```

O `Err` é incondicional — `tauri-build-2.6.3/src/lib.rs`, no bloco
`if target_triple.contains("windows")`: o caminho do ícone é
`attributes.windows_attributes.window_icon_path`, senão o primeiro `.ico` de
`config.bundle.icon`, senão `"icons/icon.ico"`; e, se esse caminho não existir,
a função devolve `Err` (linhas 608–675). Não há chave de configuração que
dispense o ícone.

Consequências reais:

| | Sem `.ico` (estado deste repositório) | Com um `.ico` válido |
| --- | --- | --- |
| `cargo check` (Windows) | **falha** (exit 101, no `build.rs`) | passa (exit 0) |
| `tauri build` (Windows) | falha no mesmo ponto | prossegue para WiX/NSIS |

Ou seja: **neste alvo, o ícone não é um problema do empacotador — é um
pré-requisito de compilação.** A app não compila no Windows enquanto não houver
um `icons/icon.ico`.

Para fechar a lacuna quando houver arte: colocar os arquivos em `icons/` e
acrescentar

```json
"bundle": { "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.ico"] }
```

(`npm run tauri icon <fonte.png>` gera o conjunto completo a partir de uma
imagem quadrada. O `.ico` tem de existir como arquivo — o `tauri-build` lê o
conteúdo, não basta a chave apontar para lá.)

Enquanto não houver ícone, o `cargo check` pode ainda assim ser corrido **para
diagnóstico**, injectando um `.ico` existente por variável de ambiente (o
`TAURI_CONFIG` é fundido sobre o `tauri.conf.json` como *merge patch* RFC 7386 —
`tauri-build/src/lib.rs:487`), sem tocar em nenhum arquivo do repositório:

```bash
TAURI_CONFIG='{"bundle":{"icon":["C:/Windows/ACU.ico"]}}' cargo check --all-targets
```

Isto **não** é a configuração do projecto: é uma medição. Produz um binário
cujo ícone de recurso é o de outro programa, e serve para responder a "o código
compila?" sem responder "que ícone é este?".

## Versões

Medidas com `cargo info`, não presumidas:

| Crate | Versão | Porquê |
| --- | --- | --- |
| `tauri` | `2.11.5` | o stable; `3.0.0-alpha.0` existe e **não** é usado |
| `tauri-build` | `2.6.3` | o par do `tauri` 2.11.5 |
| `tauri-cli` (npm) | `2.11.4` | casa com o `tauri-cli` do registo |

## Workspace próprio

O `Cargo.toml` da raiz deste repositório declara um `[workspace]` (o do
`kernel`) que **não** inclui `arkhe-ui/src-tauri`. O cargo sobe a árvore,
encontra esse workspace e recusaria este pacote como membro implícito. A tabela
`[workspace]` (vazia) no `Cargo.toml` deste directório faz dele a raiz do seu
próprio workspace — que é o que uma app Tauri é — e mantém o `Cargo.lock` da app
separado do lock da raiz.
