//! `arkhe-ui-app` — a app de verificação do Arkhe como **aplicativo
//! instalável** (Plano Arkhe OS, Fase 5).
//!
//! # O que este casco é
//!
//! Até à Fase 4 a app era uma página: `arkhe-ui/dist-app/`, com o verificador
//! em WebAssembly carregado pelo Vite. Aqui ela ganha o que faltava para ser um
//! aplicativo — uma janela nativa e um instalador — sem que a app passe a
//! depender disso. O `frontendDist` continua a ser `dist-app/`: **o mesmo
//! build**, embutido no binário em vez de servido.
//!
//! # Porque é que este casco fala com `arkhe-verify` em vez de com o wasm
//!
//! As duas rotas verificam com o **mesmo core**. A casca wasm
//! (`arkhe-verify-wasm`) e a casca nativa (`arkhe-verify`) chamam as mesmas
//! funções `*_inner`; é a arquitetura "um core, duas cascas". A diferença é o
//! que cada uma consegue fazer:
//!
//! - A casca wasm recebe os bytes que o **JavaScript** já tem em memória — o
//!   digest de um modelo de vários GB teria de passar inteiro pelo heap do
//!   webview antes de ser conferido.
//! - A casca nativa lê o arquivo no processo Rust (`inspect_gguf_model`),
//!   onde o descritor de arquivo existe. É a razão de o `arkhe-verify` ser
//!   deliberadamente `&[u8]`-only: sem `Path`, sem I/O, o core compila para
//!   wasm; quem lê o arquivo é quem chama — e quem chama aqui é este casco.
//!
//! Nada de verificação é reimplementado neste crate. `inspect_gguf_model`
//! chama `arkhe_verify::gguf::{model_digest, parse_header}` e devolve o
//! resultado; não há uma segunda leitura de cabeçalho GGUF nem uma segunda
//! computação de SHA-256 escritas aqui.
//!
//! # O que este casco **não** faz
//!
//! - Não expõe a atestação completa. O delegate natural seguinte é
//!   `arkhe_verify::gguf::verify_model_attestation`, que confere a ligação
//!   modelo ↔ atestação; esta fase expõe só a inspecção do modelo.
//! - Não abre diálogo de ficheiros. O caminho chega como `String` do
//!   frontend; não há permissão de filesystem nem plugin de diálogo
//!   configurados. Ver a nota de segurança em [`inspect_gguf_model`].
//! - Não traz arte. Existe um `icons/icon.ico` **provisório** — um losango
//!   geométrico gerado por aritmética, que existe porque o `tauri-build` o exige
//!   em tempo de build-script no Windows. Não é um recurso de marca e tem de ser
//!   substituído; ver `icons/PROVISORIO.md`.

use arkhe_verify::gguf::{model_digest, parse_header, GgufHeaderReport};
use serde::Serialize;

/// O que a app recebe ao inspeccionar um modelo.
///
/// O digest e o cabeçalho vêm em campos **separados** pelo mesmo motivo que
/// `GgufModelReport` os separa no núcleo: os dois factos não se implicam. Um
/// arquivo truncado tem um digest perfeitamente calculável e um cabeçalho que
/// não interpreta, e quem chama precisa de ver qual dos dois falhou em vez de
/// um `false` só.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelInspection {
    /// O caminho que foi lido, ecoado de volta.
    ///
    /// Ecoado de propósito: o frontend pode ter vários pedidos em voo, e a
    /// resposta sem o caminho obrigaria a correlacioná-los por ordem de
    /// chegada.
    pub path: String,
    /// Quantos bytes foram lidos. É o mesmo número que
    /// [`GgufHeaderReport::available_bytes`] — repetido no topo porque a
    /// aritmética do truncamento é a primeira coisa que se quer ver.
    pub bytes: usize,
    /// `SHA-256(model)`, em hex minúsculo — [`model_digest`].
    pub digest_hex: String,
    /// O cabeçalho lido dos **mesmos** bytes — [`parse_header`].
    pub header: GgufHeaderReport,
}

/// Lê um arquivo de modelo do disco e devolve `model_digest` e `parse_header`.
///
/// # Porque é que esta função **não** é `pub` — e não deve passar a ser
///
/// `#[tauri::command]` num `pub fn` **não compila** (E0255). O macro gera um
/// `macro_rules! __cmd__<nome>` no módulo e, quando a função é `pub`,
/// acrescenta `#[macro_export]` ao macro e depois emite
/// `pub use {__cmd__<nome>, __tauri_command_name_<nome>};` para o macro ser
/// resolvível pelo mesmo caminho da função (`tauri-macros/src/command/wrapper.rs`,
/// linhas 163 e 351). O `pub use` reimporta para o mesmo módulo um macro que
/// acabou de ser definido ali — o mesmo nome, duas vezes, no espaço de nomes
/// de macros.
///
/// A visibilidade herdada (privada) é, por isso, a forma correcta, e é o que o
/// template da Tauri usa. Funciona porque `generate_handler!` é invocado no
/// [`run`], que está neste mesmo módulo: um comando privado é invocável pelo
/// registo e invisível fora dele, que é exactamente o que se quer de uma
/// fronteira de IPC.
///
/// # O I/O vive aqui, e é o ponto
///
/// `arkhe-verify` é `&[u8]`-only por desenho, para poder compilar para wasm.
/// Este comando é o lado que *tem* um sistema de ficheiros: lê o arquivo uma
/// vez e entrega os mesmos bytes às duas funções do núcleo. Ler uma vez e
/// derivar os dois factos evita a janela em que o digest e o cabeçalho
/// descreveriam arquivos diferentes (um ficheiro substituído entre as duas
/// leituras).
///
/// # Nota de segurança: o caminho é entrada não confiável
///
/// O `path` vem do frontend, e o frontend corre código que veio da rede. Este
/// comando lê **qualquer** caminho que lhe seja dado, com os privilégios do
/// utilizador — não há sandbox de ficheiros. É aceitável enquanto o frontend é
/// o `dist-app/` embutido neste binário (não há conteúdo remoto: `frontendDist`
/// é local e não há navegação externa), mas deixa de ser se a app carregar
/// conteúdo remoto ou executar JS de terceiros. A correcção nessa altura é uma
/// allowlist de raízes, não uma validação do caminho no frontend.
///
/// # Erros
///
/// O único `Err` é a falha de leitura (ficheiro ausente, sem permissão,
/// directoria). Um arquivo que **existe** mas não é um GGUF — ou é um GGUF
/// truncado — **não** é um erro: é um resultado, e vem em
/// [`ModelInspection::header`] com `ok: false` e a causa em `error`. Nunca
/// devolve `Err` por conteúdo.
#[tauri::command]
fn inspect_gguf_model(path: String) -> Result<ModelInspection, String> {
    let bytes =
        std::fs::read(&path).map_err(|err| format!("não foi possível ler {path}: {err}"))?;

    Ok(ModelInspection {
        digest_hex: model_digest(&bytes),
        header: parse_header(&bytes),
        bytes: bytes.len(),
        path,
    })
}

/// A fronteira do IPC: a lista dos comandos que o frontend pode invocar.
///
/// Um comando `#[tauri::command]` que não esteja nesta lista existe como função
/// mas não é invocável — é esta lista que é a fronteira, e é por isso que ela
/// fica ao lado da definição das funções.
///
/// # Porque é que isto está numa função, e não dentro do [`run`]
///
/// Para que o **registo** seja um só. `tests/ipc.rs` passa esta mesma função ao
/// runtime falso (`MockRuntime`) e despacha por ela, portanto remover um comando
/// daqui quebra os testes de despacho — não só a app. Se o `invoke_handler`
/// estivesse escrito em dois sítios (um aqui, outro no teste), o teste
/// continuaria verde a exercitar um registo que a app já não tem, que é a forma
/// mais silenciosa de um teste de integração deixar de valer.
///
/// Genérica sobre `R: tauri::Runtime` porque é isso que permite servir as duas
/// pontas: `tauri::Wry` (o `Builder::default` do [`run`]) e o `MockRuntime` dos
/// testes.
///
/// # Porque é que é `pub`
///
/// `pub` e não privada só para o alvo de integração a poder chamar: um
/// `tests/*.rs` é um crate à parte, que vê a API pública e mais nada, e
/// duplicar-lhe o `invoke_handler` era precisamente o que esta função existe
/// para evitar. Não é um alargamento gratuito da superfície — é a superfície
/// mínima que deixa o teste exercitar o registo **de produção**. (O comando em
/// si continua privado; o comentário de [`inspect_gguf_model`] explica porquê.)
///
/// O `#[must_use]` acompanha o de `Builder::invoke_handler`: ignorar o
/// resultado seria devolver um builder **sem** os comandos registados.
#[must_use]
pub fn registar_comandos<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![inspect_gguf_model])
}

/// Arranca a janela da app.
///
/// Chamada pelo `src/main.rs`. O registo do que o frontend pode pedir vive em
/// [`registar_comandos`], partilhado com os testes.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    registar_comandos(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("falha ao arrancar a janela da app Arkhe");
}

#[cfg(test)]
mod tests {
    use super::*;

    // Este módulo guarda os testes **unitários**: chamam `inspect_gguf_model`
    // directamente, provando a função. O **despacho** — o mesmo comando pelo
    // `invoke_handler` registado — vive em `tests/ipc.rs`, porque só um alvo de
    // integração pode receber o manifesto do ComCtl32 v6 de que o
    // `tauri::test` precisa no Windows. O `build.rs` e o topo daquele ficheiro
    // explicam a cadeia toda.

    /// Um caminho único no directorio temporário, sem trazer uma dependência
    /// (`tempfile`) só para isto.
    fn caminho_temporario(nome: &str) -> std::path::PathBuf {
        let unico = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("relógio antes da epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("arkhe-ui-app-{}-{unico}-{nome}", std::process::id()))
    }

    /// O caminho feliz do delegate: os bytes que o comando lê no disco são os
    /// mesmos que o núcleo digere.
    ///
    /// O digest esperado é a constante conhecida de `SHA-256("")` e
    /// `SHA-256("GGUF")` — vinda de **fora** desta implementação, para que o
    /// teste não seja a implementação a concordar consigo mesma.
    #[test]
    fn inspeciona_bytes_reais_do_disco() {
        let caminho = caminho_temporario("vazio.bin");
        std::fs::write(&caminho, b"").expect("escrever fixture");

        let inspeccao = inspect_gguf_model(caminho.to_string_lossy().into_owned())
            .expect("ler a fixture");

        // `SHA-256("")`, do NIST.
        assert_eq!(
            inspeccao.digest_hex,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(inspeccao.bytes, 0);
        assert!(!inspeccao.header.ok, "zero bytes não é um cabeçalho GGUF");

        std::fs::remove_file(&caminho).expect("remover fixture");
    }

    /// Conteúdo inválido é um **resultado**, não um erro: o comando só devolve
    /// `Err` quando a leitura falha.
    #[test]
    fn conteudo_invalido_nao_e_erro_de_leitura() {
        let caminho = caminho_temporario("magic-errado.bin");
        std::fs::write(&caminho, b"nao sou um gguf").expect("escrever fixture");

        let inspeccao = inspect_gguf_model(caminho.to_string_lossy().into_owned())
            .expect("conteúdo inválido não é Err");

        // `!magic_ok` em vez de `magic_ok == false`: o `clippy::bool_comparison`
        // recusa a segunda forma, e com `-D warnings` isso é erro. Era o único
        // lint do `--all-targets` deste pacote que já vinha de trás desta
        // tarefa; a asserção é a mesma.
        assert!(!inspeccao.header.magic_ok);
        assert!(!inspeccao.header.ok);
        assert!(inspeccao.header.error.is_some(), "a causa tem de vir preenchida");
        // O digest é reportado mesmo com o cabeçalho recusado — o dado que
        // existe não é escondido pela falha.
        assert_eq!(inspeccao.digest_hex.len(), 64);

        std::fs::remove_file(&caminho).expect("remover fixture");
    }

    /// Um caminho que não existe é o único caso de `Err`.
    #[test]
    fn caminho_inexistente_e_erro() {
        let caminho = caminho_temporario("nao-existe.bin");
        let erro = inspect_gguf_model(caminho.to_string_lossy().into_owned())
            .expect_err("caminho ausente tem de ser Err");
        assert!(erro.contains("não foi possível ler"), "erro inesperado: {erro}");
    }
}
