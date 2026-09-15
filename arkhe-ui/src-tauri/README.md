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
| `icons/` | **Conjunto de ícones provisório, não é arte** — PNG, `.icns` e `.ico`, gerados por `tauri icon`; ver `icons/PROVISORIO.md` |

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

Existe um **conjunto** de ícones em `icons/`. É **provisório e não é arte** — um
losango geométrico gerado por aritmética, que existe por razões mecânicas e não
por razões de marca: o `.ico` porque o `tauri-build` o exige para compilar no
Windows, e os PNG + `.icns` porque o empacotamento de Linux/macOS os pede. A declaração completa está em
[`icons/PROVISORIO.md`](icons/PROVISORIO.md), com o layout dos bytes, o sha256 e
como foi validado.

`bundle.icon` está declarado apontando para esse ficheiro:

```json
"bundle": { "icon": ["icons/32x32.png", "icons/128x128.png",
                      "icons/128x128@2x.png", "icons/icon.icns",
                      "icons/icon.ico"] }
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
- **As outras plataformas.** O conjunto existe (`32x32.png`, `128x128.png`,
  `128x128@2x.png`, `icon.icns`) e `bundle.icon` lista os cinco, mas **nenhum
  build de Linux ou macOS foi corrido** — esta máquina é Windows. Os formatos
  são estruturalmente válidos e descodificam; que um empacotador de Linux/macOS
  os aceite **não foi medido**.

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

## O updater: referência, relays e servidores Blossom

Configuração medida, não preferida. Tudo abaixo foi verificado em **2026-09-15**
contra o código dos crates no registo e contra a rede real. As linhas citadas são
as do crate tal como está em `~/.cargo/registry/src/index.crates.io-*/`.

### A referência: porque **não** tem `stable`

```json
"reference": "htree://npub1ql0…/releases%2Farkhe-os/latest"
```

O `UpdateRef::parse` (`hashtree-updater-0.2.85/src/reference.rs:22-38`) lê a
referência como **três** partes, não quatro: o primeiro segmento é o `npub`, o
segundo é o `tree_name` (descodificado de `%2F`), e **tudo o resto é `path`**.

`…/releases%2Farkhe-os/latest` → `tree_name = "releases/arkhe-os"`, `path = "latest"`.

Acrescentar um `stable` — `…/releases%2Farkhe-os/stable/latest` — daria
`path = "stable/latest"`, um subdirectório que não existe na árvore publicada.
**Quebrava a resolução.** O `stable` é o valor do campo `channel` do manifesto
(`hashtree-updater-0.2.85/src/manifest.rs:13`, e o manifesto 0.2.1 publicado tem
`"channel": "stable"`) — um campo, não um segmento de caminho.

A confusão tem origem identificada: o exemplo no doc-comment do próprio plugin
(`tauri-plugin-hashtree-updater-0.2.52/src/lib.rs:9`) escreve
`htree://npub1.../releases%2Fmyapp/stable/latest`. É um **exemplo ilustrativo**,
e é inválido pela gramática do próprio crate que o documenta.

### `relays`: declarado, e porque é que cada entrada está lá

Declarar `relays` **substitui** o conjunto por omissão do resolvedor — não
acrescenta (`tauri-plugin-hashtree-updater-0.2.52/src/updater.rs:81-83`). O
fallback é `NostrResolverConfig::default()` =
`{relay.damus.io, relay.primal.net, relay.snort.social}`
(`hashtree-resolver-0.2.85/src/nostr.rs:50-62`).

Com o filtro real do resolvedor (`kinds:[30064,30078]`, `author`,
`#d:releases/arkhe-os`), medido na rede:

| Relay | Responde? | Tem o evento `0aefc42b…`? |
| --- | --- | --- |
| `wss://relay.snort.social` | sim, EOSE ~0.9 s | **sim** |
| `wss://temp.iris.to` | sim, EOSE ~1.1 s | **sim** |
| `wss://relay.primal.net` | sim, EOSE ~0.8 s | não (0 eventos) |
| `wss://nos.lol` | sim, EOSE ~0.7 s | não (0 eventos) |
| `wss://relay.damus.io` | **2 de 4 tentativas falharam** (1× TIMEOUT, 1× ERROR) | não — e quando responde devolve um evento **obsoleto** |
| `wss://relay.iris.to` | **não** — `NXDOMAIN` | irrelevante |

Duas medições que decidem a lista:

1. **O `damus.io` serve uma raiz obsoleta.** O evento que devolve é o
   `21343c0211a4ee62…`, de `2026-09-15T15:43:33Z`; a raiz viva é a
   `0aefc42b5eb7f09f…`, de `2026-09-15T16:31:48Z` — 48 minutos mais recente.
   Não é um caso de "chega ou não chega": é um NIP-33 que o `damus` não
   substituiu. E é perigoso, porque o resolvedor **devolve cedo** quando atinge
   o quórum (`hashtree-resolver-0.2.85/src/nostr.rs:518`): com os 3 relays por
   omissão o quórum é 2, e o quórum conta relays que **respondem**, não relays
   que **têm o evento** (`:503-519`). Se o `damus` (obsoleto) e o `primal`
   (0 eventos) fecharem antes do `snort`, a função devolve o conjunto com **só o
   evento obsoleto**. Excluir o `damus` elimina a única fonte de raiz obsoleta
   que foi medida.
2. **O `relay.iris.to` não existe.** Não é um relay lento nem um relay morto: o
   nome não resolve (`NXDOMAIN`, confirmado por DNS). Aparece **uma única vez**
   em todos os crates `hashtree-*` e no plugin —
   `tauri-plugin-hashtree-updater-0.2.52/src/lib.rs:11`, dentro do exemplo do
   doc-comment. Não é omissão do plugin; não é omissão de crate nenhum. É a
   mesma origem do erro da referência acima.

A lista declarada, e a razão de cada entrada:

| Entrada | Papel | Porquê |
| --- | --- | --- |
| `wss://relay.snort.social` | **fonte do evento** | medido com a raiz correcta `0aefc42b…` |
| `wss://temp.iris.to` | **fonte do evento** | medido com a raiz correcta `0aefc42b…` |
| `wss://relay.primal.net` | **só quórum** | responde depressa (~0.8 s) mas devolveu **0 eventos**: contribui para o quórum de 2, **não** é redundância para o dado |

São 3 relays porque o quórum é `min(n, 2)`
(`hashtree-resolver-0.2.85/src/nostr.rs:36,470`): com 3, um relay em baixo não
custa os 3 s do *soft timeout* (`:521-529`). O `primal` foi mantido do conjunto
por omissão de propósito — o delta é **tirar um** (`damus`) e **acrescentar um**
(`temp.iris.to`), o mínimo explicável. O `nos.lol` também respondeu (mais
depressa, ~0.7 s) e também só serve de quórum; ficou de fora por não estar no
conjunto por omissão do resolvedor. **Se algum dos dois relays-fonte morrer, a
resolução continua a funcionar** (soft timeout devolve o que houver, `:521-529`)
**mas fica com uma só fonte independente do dado** — as entradas de quórum não
cobrem isso.

### `blossomServers`: **deliberadamente ausente** — declarar seria piorar

O campo **não** está declarado, e a razão é uma medição, não uma omissão.

**Substitui, não acrescenta.** `updater.rs:67-69` só chama `with_servers` quando
a lista não é vazia; e `with_servers` faz `self.read_servers = servers.clone();
self.write_servers = servers;`
(`hashtree-blossom-0.2.83/src/lib.rs:464-467`). Isto é, declarar `blossomServers`
**descarta** os servidores por omissão — tanto para ler como para escrever.

Os por omissão, efectivos: `BlossomClient::new`
(`hashtree-blossom-0.2.83/src/lib.rs:402-404`) usa `all_read_servers()`, que é
`read_servers + write_servers` (`hashtree-config-0.2.83/src/lib.rs:315-326`) =
`{https://blossom.primal.net, https://cdn.iris.to, https://upload.iris.to}`
(constantes em `hashtree-config-0.2.83/src/lib.rs:11,14`).

Medido contra o blob da raiz publicada (`1586c88d…`, 372 bytes):

| Servidor | Resultado |
| --- | --- |
| `https://cdn.iris.to` | **200** (308 → `/<hash>.bin`), 372 bytes, sha256 = `1586c88d…` ✓ |
| `https://upload.iris.to` | **200**, 372 bytes, sha256 = `1586c88d…` ✓ |
| `https://blossom.primal.net` | **404** — não tem o blob |
| `https://hashtree.iris.to` | **`NXDOMAIN`** — não existe |

Uma lista declarada a partir do que circula (`cdn.iris.to` + `hashtree.iris.to`)
seria, medidamente, **pior que o silêncio**: descartaria o `upload.iris.to` (que
serve o blob, provado acima) para acrescentar um nome que não resolve. O
`hashtree.iris.to` não ocorre **em crate nenhum** — nem constante, nem omissão,
nem exemplo (zero ocorrências em todos os `hashtree-*` e no plugin); não é "o
servidor do `hashtree-cli`".

Dois efeitos secundários confirmam que o silêncio é a escolha certa:

- **O daemon local.** O `BlossomClient::new` insere um daemon local em
  `127.0.0.1:8080` no topo da lista de leitura se o detectar
  (`hashtree-blossom-0.2.83/src/lib.rs:407-417`), porque é por aí que o
  `hashtree` faz leitura por pares. O `with_servers` corre **depois** disso e
  apaga essa entrada. (Não foi medido com daemon a correr nesta máquina — estava
  parado, `htree status` → *Daemon not running*.)
- **A configuração do utilizador.** O `new` carrega o `~/.hashtree/config.toml`
  **do utilizador** (`lib.rs:403`). Declarar a lista no `tauri.conf.json` da app
  passa a sobrepor-se à escolha de quem usa a app.

Por isso: `blossomServers` fica de fora. Os por omissão já são um superconjunto
do que foi verificado a servir o blob, e declarar só pode **remover**.

### Como a lista declarada foi verificada (0.2.2)

Publicado a 0.2.2, o evento de raiz passou a ser o `af5a38c090ee7929…`
(`hash=728b9eddb106…`, `2026-09-15T17:10:33Z`). Medido nos quatro relays, logo
depois de publicar:

| Relay | Resultado |
| --- | --- |
| `wss://relay.snort.social` | **a raiz nova** (1 evento) |
| `wss://temp.iris.to` | **a raiz nova** (1 evento) |
| `wss://relay.primal.net` | 0 eventos — o papel de quórum previsto, confirmado |
| `wss://relay.damus.io` | `ERROR` — confirmado outra vez |

A verificação de que a lista declarada **basta**, e não apenas que o release
resolve, foi feita isolando o conjunto. O `htree install --check` do CLI lê os
relays do `~/.hashtree/config.toml` do utilizador — **não** do `tauri.conf.json`
— portanto mede o conjunto, não o ficheiro. Com o `config.toml` reduzido aos três
declarados, o `--check` resolveu `0.2.2` com **zero erros de relay**. Com o
`config.toml` reduzido a `wss://relay.iris.to` — o nome que circula como "o relay
padrão" — o mesmo comando falha, exit 1:

```text
Error: failed to resolve update root: Network error: Failed to get events from
configured relays: wss://relay.iris.to: recv message response timeout
```

Esse controlo negativo faz dois trabalhos ao mesmo tempo: prova que o comando
estava mesmo a ler o `config.toml` (senão não falhava), e prova pelo próprio
caminho de código do resolvedor que o `relay.iris.to` não serve. O
`config.toml` foi restaurado ao estado original; nada nele ficou alterado.

O teste `tests/updater_relays.rs` guarda o invariante que é verificável sem rede:
que a lista declarada é exactamente a medida, que o `relay.iris.to` e o `damus`
não podem reaparecer, que o `blossomServers` continua ausente e que a referência
não ganhou o segmento `stable`. O teste de rede ao lado está `#[ignore]` por uma
razão medida (falta o `CryptoProvider` de processo fora do runtime do Tauri) —
a razão completa está no doc-comment dele.

## Workspace próprio

O `Cargo.toml` da raiz deste repositório declara um `[workspace]` (o do
`kernel`) que **não** inclui `arkhe-ui/src-tauri`. O cargo sobe a árvore,
encontra esse workspace e recusaria este pacote como membro implícito. A tabela
`[workspace]` (vazia) no `Cargo.toml` deste directório faz dele a raiz do seu
próprio workspace — que é o que uma app Tauri é — e mantém o `Cargo.lock` da app
separado do lock da raiz.
