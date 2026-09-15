//! O script de build da app.
//!
//! `tauri_build::build()` é o que lê o `tauri.conf.json`, resolve os
//! `frontendDist`/`devUrl` e gera o código de contexto que
//! `tauri::generate_context!()` consome no `src/lib.rs` — inclusive os recursos
//! do Windows (manifest e versão do executável). Sem esta chamada o
//! `generate_context!` não compila.
//!
//! Nota: este script **precisa** do ícone no Windows, e isso foi medido contra a
//! expectativa. O bloco `if target_triple.contains("windows")` do `tauri-build`
//! gera um *Windows Resource file* a partir do `.ico` e devolve `Err` se o
//! ficheiro não existir — sem chave de configuração que o dispense
//! (`tauri-build-2.6.3/src/lib.rs:608-675`). Não é, portanto, um requisito só do
//! empacotador (`tauri build`): sem `icons/icon.ico` o próprio `cargo check`
//! falhava com exit 101. O ícone que existe é **provisório** — ver
//! `icons/PROVISORIO.md` e o README deste directorio.

/// O manifesto de aplicação do Windows, reduzido à única coisa que os **testes**
/// precisam dele.
///
/// O `tauri-build` já embute um manifesto equivalente no binário
/// (`tauri-build-2.6.3/src/windows-app-manifest.xml`), mas esse vai com o ícone e
/// a informação de versão e é ligado só aos binários — ver
/// [`ligar_manifesto_aos_testes`] para o porquê de os testes ficarem de fora.
///
/// Aqui só interessa a dependência de `Microsoft.Windows.Common-Controls` v6. O
/// `<assemblyIdentity>` do próprio executável é omitido de propósito: um
/// manifesto sem ele é válido, e escrever um nome inventado só criaria uma
/// identidade falsa. O mesmo vale para `requestedExecutionLevel` — este alvo não
/// precisa de privilégios e não os deve pedir.
const MANIFESTO_DOS_TESTES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#;

/// As regras de *quoting* do `rc.exe` dentro de uma string entre aspas.
///
/// Uma aspa literal escreve-se **dobrada** (`""`) e a barra invertida também
/// (`\\`) — esta segunda não é decorativa: o `rc.exe` processa as sequências de
/// escape à moda do C dentro das strings, e um `\r` ou um `\a` de um caminho do
/// Windows deixam de ser dois caracteres e passam a ser um. Foi medido num
/// `.rc` de teste: `"C:\rctest\ascii\m.xml"` é lido como `C:<CR>ctest<BEL>scii\m.xml`
/// e o `rc.exe` responde `RC2135: file not found`. É a razão de o
/// `tauri-winres` ter uma `escape_string` a dobrar barras
/// (`tauri-winres-0.3.6/src/helpers.rs:44-54`) e a razão de o manifesto ir
/// **inline** aqui, sem caminhos nenhuns lá dentro. As restantes regras que o
/// `tauri-winres` implementa (`\'`, `\n`, …) não são usadas porque o manifesto é
/// ASCII sem apóstrofos nem quebras de linha internas.
fn escapar_para_rc(linha: &str) -> String {
    let mut saida = String::with_capacity(linha.len());
    for caractere in linha.chars() {
        match caractere {
            '"' => saida.push_str("\"\""),
            '\\' => saida.push_str("\\\\"),
            outro => saida.push(outro),
        }
    }
    saida
}

/// Liga o manifesto acima aos **alvos de teste** (`tests/*.rs`).
///
/// # O sintoma, medido antes de ser compreendido
///
/// `cargo test` passou a morrer com `STATUS_ENTRYPOINT_NOT_FOUND` (`0xc0000139`)
/// — exit 127, zero testes corridos, nenhuma saída — no momento em que
/// `tests/ipc.rs` passou a usar `tauri::test`. O `dumpbin -dependents` mostrou o
/// binário de teste a importar `comctl32.dll` (o binário anterior, sem
/// `tauri::test`, não o importava); a comparação dos imports contra as
/// exportações reais do `C:\Windows\System32\comctl32.dll` isolou o símbolo em
/// falta: **`TaskDialogIndirect`**, que só existe na versão 6.0 do ComCtl32.
///
/// # A causa
///
/// 1. `tauri::test::mock_app()`/`mock_builder()` referenciam `InvokeMessage`, que
///    em `tauri` traz `#[default_runtime(crate::Wry, wry)]` — por isso o `tao` (e
///    o `muda`) passam a ser ligados ao binário de teste, e o `tao` importa
///    `TaskDialogIndirect` no seu event loop.
/// 2. O ComCtl32 v6 só é carregado se o executável declarar a dependência no seu
///    manifesto de aplicação. Sem manifesto o loader resolve o import contra o
///    v5.82 do sistema, não o encontra, e termina o processo antes do `main`.
/// 3. O `tauri-build` compila esse manifesto — mas através do `embed-resource`,
///    que emite `cargo:rustc-link-arg-bins=`
///    (`embed-resource-3.0.11/src/lib.rs:443`): **só binários**. Os alvos de
///    teste ficavam sem manifesto, e isso era invisível enquanto nada nos testes
///    tocasse no `tauri::test`.
///
/// # A correcção, e porque é aqui
///
/// `embed_resource::compile_for_tests` é a mesma máquina — o `embed-resource` é a
/// dependência que o `tauri-build` já usa — mas emite
/// `cargo:rustc-link-arg-tests=`. É o directivo que faltava, e obriga a que o
/// teste de despacho viva em `tests/` (um `mod tests` de uma `lib` não é um alvo
/// de teste para efeitos deste directivo: o `cargo` recusa-o com "The package
/// arkhe-ui-app does not have a test target"). Ver o topo de `tests/ipc.rs`.
///
/// Falha **alto** se não conseguir: um manifesto ausente não dá erro de
/// compilação, dá um binário de teste que não arranca — e um `cargo test` que
/// devolve 127 sem dizer nada é exactamente o que esta função existe para não
/// voltar a acontecer.
fn ligar_manifesto_aos_testes() {
    // O `embed-resource` já devolve `NotWindows` fora do Windows; o teste de
    // alvo evita, ainda assim, escrever ficheiros no `OUT_DIR` de builds que não
    // os vão usar.
    let alvo = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if alvo != "windows" && alvo != "android" {
        return;
    }

    let out_dir = std::path::PathBuf::from(
        std::env::var("OUT_DIR").expect("OUT_DIR definido pelo cargo num build script"),
    );
    let recurso = out_dir.join("arkhe-tests.rc");

    // `1 24` é `RT_MANIFEST` com o id 1 — `CREATEPROCESS_MANIFEST_RESOURCE_ID`, o
    // recurso que o loader procura. É a mesma forma que o `tauri-winres` escreve
    // (`tauri-winres-0.3.6/src/lib.rs:459-465`): o manifesto entra **inline**,
    // uma linha por string, em vez de ser referenciado por caminho — o que
    // também evita ter de escapar um caminho deste repositório (que tem um `ω`)
    // dentro de um `.rc`.
    let mut rc = String::from("#pragma code_page(65001)\n1 24\n{\n");
    for linha in MANIFESTO_DOS_TESTES
        .lines()
        .map(str::trim)
        .filter(|linha| !linha.is_empty())
    {
        rc.push_str("\" ");
        rc.push_str(&escapar_para_rc(linha));
        rc.push_str(" \"\n");
    }
    rc.push_str("}\n");
    std::fs::write(&recurso, rc).expect("escrever o .rc dos testes");

    // `manifest_required` traduz um `rc.exe` ausente ou uma compilação falhada
    // num `Err`, em vez de deixar passar em silêncio.
    if let Err(erro) =
        embed_resource::compile_for_tests(&recurso, embed_resource::NONE).manifest_required()
    {
        panic!(
            "não foi possível ligar o manifesto do ComCtl32 v6 aos alvos de teste: {erro}. \
             Sem ele o binário de teste não arranca (STATUS_ENTRYPOINT_NOT_FOUND, exit 127)."
        );
    }
}

fn main() {
    tauri_build::build();
    ligar_manifesto_aos_testes();
}
