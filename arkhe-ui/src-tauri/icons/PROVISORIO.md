# `icons/icon.ico` — **PROVISÓRIO**. Não é arte. Substituir.

Este ficheiro **não é um recurso de marca**. É um marcador de lugar geométrico
que existe por uma razão mecânica: **sem ele a app não compila no Windows**.

## Porque é que existe

`tauri-build` exige um `.ico` **em tempo de build-script**, e falha se ele não
existir — o `Err` é incondicional e não há chave de configuração que o dispense
(`tauri-build-2.6.3/src/lib.rs`, no bloco `if target_triple.contains("windows")`:
o caminho resolve para `windows_attributes.window_icon_path`, senão para o
primeiro `.ico` de `bundle.icon`, senão para `"icons/icon.ico"`; se o ficheiro
não existir, devolve `Err`).

Consequência medida, antes deste ficheiro:

```text
`icons/icon.ico` not found; required for generating a Windows Resource file during tauri-build
cargo check → exit 101
```

Ou seja, neste alvo o ícone **não é um problema do empacotador** (`tauri build`)
— é um pré-requisito de **compilação** (`cargo check`). Era o único bloqueio:
com o ficheiro presente, `cargo check` passa a exit 0 sem qualquer override.

## O que é, exactamente

| | |
| --- | --- |
| Formato | ICO, 1 imagem |
| Dimensões | 256 × 256 |
| Codificação | DIB/BMP **não comprimido** (`BI_RGB`), 32 bpp BGRA |
| Tamanho | 270398 bytes |
| sha256 | `882477b554d354f2758c56bec0c3b6b18dc4c5c382055d892bbe2227e13d6614` |

O desenho é um losango de contorno sobre fundo escuro — formas geradas por
aritmética sobre distância L1 (`|dx| + |dy|`), não desenhadas por ninguém. As
cores (âmbar sobre azul-escuro) foram escolhidas para **parecer um marcador de
lugar**, e não para parecer uma marca.

Layout dos bytes, para quem quiser reconstruí-lo:

```text
ICONDIR        6 B   reserved=0, type=1 (ícone), count=1
ICONDIRENTRY  16 B   width=0 (=>256), height=0 (=>256), colourCount=0,
                     planes=1, bitCount=32, bytesInRes=270376, offset=22
BITMAPINFOHEADER 40 B  biSize=40, biWidth=256, biHeight=512 (= 2x, XOR+AND),
                       biPlanes=1, biBitCount=32, biCompression=0 (BI_RGB)
XOR bitmap   262144 B  BGRA, bottom-up (a linha 0 do DIB é a *base* da imagem)
AND mask       8192 B  1 bpp, tudo a zero (opaco; a forma vive no alfa)
```

## Como foi provado válido

Não por se afirmar que é válido. Três medições independentes:

1. **Re-parse estrutural** do ficheiro já escrito, por um leitor separado do
   escritor — 21 invariantes (offsets, `bytesInRes`, `biHeight == 2 × altura`,
   `biSizeImage`, *stride* da máscara) e descodificação de pixels de volta dos
   bytes: centro = âmbar, canto = fundo, aresta do losango = âmbar. 21/21.
2. **Descodificador do próprio sistema (GDI+)** — `System.Drawing.Icon` +
   `ToBitmap()` + `GetPixel` via PowerShell: `256x256`, centro
   `198,154,62,255`, canto `28,32,43,255`. Isto prova que o sistema **descodifica
   a imagem**, não apenas que lê o cabeçalho.
3. **Falsificação** — para saber o que o `cargo check` realmente prova, o
   ficheiro foi substituído por lixo e o check repetido. Um ficheiro de 0 bytes
   falha (`failed to fill whole buffer`) e **lixo do tamanho certo também falha**:

   ```text
   failed to parse icon …/icons/icon.ico: Invalid reserved field value in ICONDIR (was 65535, but must be 0)
   ```

   A mensagem vem do `ico` crate, chamado por `tauri::generate_context!()` — o
   `cargo check` **analisa o conteúdo do ícone**, não se limita a verificar que o
   ficheiro existe. Portanto o exit 0 abaixo é evidência de estrutura válida, e
   não de presença. (O ficheiro original foi reposto e o hash conferido.)

## O que ainda **não** está feito

- **Não há arte.** Substituir por um ícone desenhado.
- **Não há o resto do conjunto**: apenas `icon.ico`. Não existem
  `32x32.png`, `128x128.png`, `128x128@2x.png` nem `icon.icns`. O `tauri build`
  noutras plataformas (Linux, macOS) continua por resolver — este ficheiro só
  fecha o gate no Windows.
- O caminho de substituição é `npm run tauri icon <fonte.png>`, que gera o
  conjunto completo a partir de uma imagem quadrada. Nessa altura,
  `bundle.icon` deve passar a listar os PNGs e o `.icns` — ver o `README.md`
  deste directório.

**O nome do ficheiro não pode mudar**: é o nome que o `tauri-build` procura por
omissão. Por isso esta declaração vive nos documentos e não num nome de
ficheiro como `icon-PROVISORIO.ico`, que o build não encontraria.
