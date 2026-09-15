# `icons/` — **PROVISÓRIO**. Não é arte. Substituir.

**Todo** o conteúdo desta pasta é um marcador de lugar geométrico, gerado por
aritmética. Não há um único pixel desenhado por uma pessoa. Serve para fechar
gates mecânicos de build, e para mais nada.

Se estás a ler isto à procura de um recurso de marca: **não existe marca aqui**.
Substituir o conjunto inteiro antes de qualquer publicação.

## Porque é que existe

Duas razões, ambas mecânicas.

**1. Windows — pré-requisito de compilação, não de empacotamento.**

`tauri-build` exige um `.ico` **em tempo de build-script**, e falha se ele não
existir — o `Err` é incondicional e não há chave de configuração que o dispense
(`tauri-build-2.6.3/src/lib.rs`, no bloco `if target_triple.contains("windows")`:
o caminho resolve para `windows_attributes.window_icon_path`, senão para o
primeiro `.ico` de `bundle.icon`, senão para `"icons/icon.ico"`; se o ficheiro
não existir, devolve `Err`). Consequência medida, antes de haver `.ico`:

```text
`icons/icon.ico` not found; required for generating a Windows Resource file during tauri-build
cargo check → exit 101
```

Ou seja, neste alvo o ícone não é um problema do empacotador (`tauri build`) —
é um pré-requisito de **compilação** (`cargo check`).

**2. Linux e macOS — o conjunto que faltava.**

Até esta iteração existia **apenas** `icon.ico`. Não havia `32x32.png`,
`128x128.png`, `128x128@2x.png` nem `icon.icns`, portanto o `tauri build`
noutras plataformas não tinha ícones para empacotar. Este conjunto fecha isso.

## Como foi gerado

Com a ferramenta oficial, não com um gerador caseiro. O CLI do Tauri tem o
subcomando `icon`, confirmado existente antes de escrever qualquer linha:

```console
$ npx tauri icon --help      # exit 0 — o subcomando existe (CLI 2.11.4)
$ npx tauri icon src-tauri/icons/app-icon.png
```

O fonte é `app-icon.png` — 1024 × 1024, RGBA, sem interlace — gerado por
aritmética inteira sobre os bytes: nenhuma biblioteca de imagem, nenhum pacote
instalado ou descarregado. O desenho é o mesmo do `.ico` original, para não
inventar marca nenhuma: **losango de contorno âmbar sobre fundo escuro**, com as
arestas dadas por distância L1 (`d2 = |2x−1022| + |2y−1022|`, raio `R2 = 760` em
coordenadas dobradas), uma rampa de 1 px (`|d2−R2|` entre 44 e 76) para o
downscale não serrilhar, e fundo opaco. Do runtime do Node veio apenas o
`deflate` e o `crc32` do stream PNG — primitivas de compressão, não de imagem.

As cores são as do `.ico` provisório anterior, e foram escolhidas para *parecer*
um marcador de lugar, não uma marca:

| | R, G, B |
| --- | --- |
| Fundo | `28, 32, 43` |
| Forma (âmbar) | `198, 154, 62` |

## Inventário

`bundle.icon` lista o conjunto de referência do Tauri (`32x32.png`,
`128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico`) — a mesma lista que
está embutida no binário do CLI e que o `tauri init` escreve. **O `icon.ico`
permanece na lista**: sem ele o build no Windows quebra (razão 1 acima).
`64x64.png` e os `Square*Logo.png` são produzidos pela ferramenta mas não
constam de `bundle.icon`.

Ficheiros de nível superior:

| Ficheiro | Bytes | Dimensões medidas | Cabeçalho lido |
| --- | --- | --- | --- |
| `32x32.png` | 623 | 32 × 32 | `IHDR` `00000020 00000020` |
| `64x64.png` | 1066 | 64 × 64 | `IHDR` `00000040 00000040` |
| `128x128.png` | 1587 | 128 × 128 | `IHDR` `00000080 00000080` |
| `128x128@2x.png` | 2235 | 256 × 256 | `IHDR` `00000100 00000100` |
| `icon.png` | 4360 | 512 × 512 | `IHDR` `00000200 00000200` |
| `icon.icns` | 30847 | 12 elementos, até 1024 × 1024 | magic `icns`, tamanho declarado 30847 = real |
| `icon.ico` | 11498 | 6 imagens: 16, 24, 32, 48, 64, 256 | magic `00000100`, `count = 6` |
| `app-icon.png` | 11216 | 1024 × 1024 | `IHDR` `00000400 00000400` (fonte) |
| `Square{30,44,71,89,107,142,150,284,310}Logo.png` | 660…2837 | N × N conforme o nome | `IHDR` |
| `StoreLogo.png` | 871 | 50 × 50 | `IHDR` `00000032 00000032` |
| `android/` | 17 ficheiros | — | ver ressalva abaixo |
| `ios/` | 18 ficheiros | — | ver ressalva abaixo |

O `128x128@2x.png` tem mesmo 256 × 256 e não 128 × 128 — o nome é a convenção
do macOS (2× do tamanho nominal), e a dimensão real foi medida no `IHDR`, não
assumida.

O `.icns` contém 12 elementos e nenhum é lixo: cada um foi percorrido pelo
`length` do seu próprio cabeçalho, e os que embrulham PNG têm o `IHDR` interno
lido e conferido contra o tipo declarado:

```text
ic11 32x32(16@2x)   is32 (não-PNG)    s8mk (mask)
ic12 64x64(32@2x)   ic14 512x512(256@2x)  ic07 128x128
il32 (não-PNG)      l8mk (mask)       ic13 256x256(128@2x)
ic09 512x512        ic10 1024x1024    ic08 256x256
```

O `.ico` mudou de forma em relação ao provisório anterior. Antes era 1 imagem
DIB/BMP crua de 256 × 256 (270 398 B); agora são **6 imagens, todas
PNG-comprimidas**, que é o que `tauri icon` produz. Para cada entrada,
`bytesInRes` foi conferido contra o comprimento real do stream PNG que ela
aponta — 6/6 iguais — e o `IHDR` interno contra as dimensões nominais da
entrada — 6/6 iguais.

sha256 do que importa:

```text
a6ff5244132b6c691590b7e509dbcf2dafa69ad5ef2615c8f5a29edf74b887c5  icon.ico
bf64186b9bc1e018ceec774fb486a31aabfd0e131ccbca208a01c30a67abe95e  icon.icns
8264579d4924e755d39a35baffa75bcc3e1a57d7fcf8e634333da420f1394f56  128x128.png
9463514b085bdc25cf0a671dd5c192922a1b05d0c389389f3d59e9d924b112a2  128x128@2x.png
de92a29a994b709103fe12d2bac4cb7f5b1572aa2c4fa1c26774f8efe66e1cc9  32x32.png
3c3afd0f245e687b418d491e7f5cbeb33c0992aeea5bb5e4017012c6e6b97764  app-icon.png
```

## Como foi provado válido

Não por se afirmar que é válido. Quatro medições independentes:

1. **Cabeçalhos lidos do disco**, não do escritor: `IHDR` nos offsets 16–23 de
   cada PNG, `icns` + tamanho declarado, `00000100` + contagem de imagens no
   `.ico`. Tudo o que está na tabela acima é medição.
2. **Descodificação pelo próprio sistema (GDI+)** — `System.Drawing.Image` via
   PowerShell. Os PNGs (`32x32`, `128x128`, `128x128@2x`, `icon.png`)
   descodificam nas dimensões certas, com o fundo `28,32,43` no centro e no
   canto. O mesmo para os 6 payloads extraídos do `.ico`. Isto prova que o
   sistema **descodifica as imagens**, não apenas que lê os cabeçalhos.
   *Ressalva:* `System.Drawing.Icon.ToBitmap()` **não** suporta entradas
   PNG-comprimidas em ICO e devolveu pixéis fora da paleta (lido no centro:
   `A=139,R=156,G=216,B=216`). É uma limitação conhecida desse caminho do GDI+,
   não um defeito do ficheiro — o mesmo GDI+ descodifica os payloads extraídos
   sem erro, e o descodificador do `tauri-build` descodifica o `.ico` inteiro
   (ponto 3).
3. **`cargo check` → exit 0** com o `.ico` novo. E — para saber o que isso
   realmente prova — a **falsificação**: o `IHDR` de um PNG embutido no `.ico`
   foi corrompido de propósito e o check repetido. Falhou:

   ```text
   error: proc macro panicked
     --> src\lib.rs:173:14
     = help: message: failed to decode icon …/icons/icon.ico:
       Malformed PNG data: IDAT or fdAT chunk is missing.
   cargo check → exit 101
   ```

   A mensagem vem do `tauri::generate_context!()`. Portanto o `cargo check`
   **descodifica o conteúdo do `.ico`**, não se limita a verificar que o
   ficheiro existe — o exit 0 é evidência de estrutura válida, e não de
   presença. (O ficheiro original foi reposto e o sha256 reconferido:
   `a6ff5244…b887c5`.)

4. **Integridade estrutural do `.icns`**: os 12 elementos percorrem
   exactamente os 30 847 bytes declarados, sem sobra nem falta, e o tamanho
   declarado no cabeçalho coincide com o tamanho real do ficheiro.

## O que continua declarado como provisório

- **Toda a arte.** Cada ficheiro desta pasta. Substituir por ícone desenhado.
- O desenho é um losango de contorno, gerado por aritmética. Não é uma marca,
  não é um logótipo, não tem significado.
- **Nada aqui foi revisto por ninguém** quanto a legibilidade a 16 px ou a
  contraste em temas claros/escuros de sistema.

## O que **não** foi verificado

- **Os ícones de Linux e macOS não foram validados** — esta máquina é Windows.
  O que foi provado é que os ficheiros que essas plataformas consomem
  (`*.png`, `icon.icns`) são estruturalmente válidos e descodificam; **não** foi
  provado que um `tauri build` em Linux ou macOS os aceita e empacota, nem que o
  macOS os mostra correctamente no Dock, no Finder ou a 16 × 16 na barra de
  menus. Isso exige build nessas plataformas.
- **Não foi corrido nenhum `tauri build`.** Não foram gerados instaladores, por
  instrução explícita. Os instaladores limpos pré-existentes
  (`target/release/bundle/msi/…msi`, `…/nsis/…setup.exe`) e o exe instalado
  **não foram tocados** — hashes e mtimes conferidos antes e depois.
- **As pastas `android/` e `ios/` são colaterais.** `tauri icon` gera-as sempre,
  mas este projecto não tem alvo móvel configurado no `tauri.conf.json`, e
  nenhuma delas entra em `bundle.icon`. Ficaram porque removê-las seria
  contrariar a ferramenta oficial que foi usada, e porque qualquer futura
  corrida de `tauri icon` as recria. Se não as quiseres no repositório, apaga-as
  e acrescenta-as ao `.gitignore` — é uma decisão à parte, e não muda nada do
  que está acima.

## Como substituir

```console
$ npx tauri icon <a-tua-arte-1024x1024.png> -o arkhe-ui/src-tauri/icons
```

A ferramenta **valida o fonte** (exige quadrado; PNG ou SVG com transparência) e
regenera todo o conjunto, incluindo o `.icns` e o `.ico`. Depois:

- `bundle.icon` em `src-tauri/tauri.conf.json` continua a apontar para o
  conjunto de referência — não precisa de mudar, a menos que acrescentes
  tamanhos com `-p`/`--png`.
- Correr `cargo check` em `src-tauri` para confirmar que o gate do Windows
  continua verde.
- Apagar este ficheiro. Ele existe para declarar o provisório; quando deixar de
  ser provisório, deixa de ter razão de existir.

**O nome `icon.ico` não pode mudar**: é o nome que o `tauri-build` procura por
omissão. Por isso esta declaração vive num documento e não num nome de ficheiro
como `icon-PROVISORIO.ico`, que o build não encontraria.
