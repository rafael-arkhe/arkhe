//! O **despacho do IPC** exercitado a sério, sem janela e sem sessão gráfica.
//!
//! # Porque é que isto vive em `tests/` e não no `lib.rs`
//!
//! Por uma razão de *linkagem*, não de gosto — e é uma razão que só se descobre
//! a correr. No Windows, qualquer teste que toque no `tauri::test` morre no
//! arranque do processo com `STATUS_ENTRYPOINT_NOT_FOUND` (`0xc0000139`) se
//! estiver no `mod tests` do `lib.rs`:
//!
//! 1. `tauri::test::mock_app`/`mock_builder` referenciam `InvokeMessage`, que
//!    traz `#[default_runtime(crate::Wry, wry)]` — o que arrasta o `tao` e o
//!    `muda` para dentro do binário de teste.
//! 2. O `tao` importa `TaskDialogIndirect`, que só existe no **ComCtl32 v6**, e
//!    o v6 só é ligado se o executável declarar a dependência no seu
//!    manifesto.
//! 3. O `tauri-build` compila esse manifesto, mas o liga **só aos binários**:
//!    por baixo do `tauri_winres` está o `embed-resource`, que emite
//!    `cargo:rustc-link-arg-bins=`
//!    (`embed-resource-3.0.11/src/lib.rs:443`).
//! 4. O `cargo:rustc-link-arg-tests=` que resolve isto **é rejeitado** se o
//!    pacote não tiver um alvo de teste — "The package arkhe-ui-app does not
//!    have a test target" — e o `mod tests` de uma `lib` **não** é um alvo de
//!    teste para efeitos deste directivo: é a compilação `--test` do alvo
//!    `lib`. Um ficheiro em `tests/` é que é.
//!
//! Medido, não deduzido: o `cargo test --lib` com este mesmo código falhava com
//! exit 127 e nenhum teste corrido; o erro só aparece no `mod.rs` do
//! `tauri::test` quando algo o referencia, e por isso não aparecia antes de
//! existir este teste. O `build.rs` traz a outra metade — compila o manifesto
//! para os alvos de teste.
//!
//! # O que os testes exercitam
//!
//! O `InvokeRequest` é entregue ao `Webview::on_message` — o mesmo método por
//! onde passa um pedido vindo do webview. Ficam cobertos: o registo
//! (`generate_handler!`, partilhado com o binário através de
//! [`arkhe_ui_app_lib::registar_comandos`]), a resolução do comando pelo nome,
//! o ACL, a desserialização do argumento e a serialização da resposta.
//!
//! # O que **não** exercitam
//!
//! O salto JavaScript→Rust. Não há `window.__TAURI_INTERNALS__`, não há o
//! `invoke` do `@tauri-apps/api`, não há motor JS de webview, não há
//! `tauri://localhost` a servir o `dist-app`, e o `invoke_key` é passado à mão em
//! vez de ter sido injectado no script de inicialização. Um erro de
//! serialização do lado do JS, um `postMessage` mal formado ou uma divergência
//! entre o nome do comando no bundle e no `generate_handler!` continuariam
//! invisíveis aqui. Nada disto é substituível sem uma sessão gráfica; o que
//! estes testes fazem é não deixar que a **metade Rust** da fronteira dependa de
//! a sessão gráfica existir.

use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;

/// Um caminho único no directorio temporário, sem trazer uma dependência
/// (`tempfile`) só para isto.
///
/// Duplicado do `mod tests` do `lib.rs` de propósito: um alvo de integração não
/// vê os `fn` privados do crate — só a API pública —, e exportar este helper
/// para poupar oito linhas seria alargar a superfície pública por causa de um
/// teste. A alternativa, escrever num caminho fixo, faria dois testes a
/// disputar o mesmo ficheiro quando correm em paralelo.
fn caminho_temporario(nome: &str) -> std::path::PathBuf {
    let unico = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("relógio antes da epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("arkhe-ui-app-ipc-{}-{unico}-{nome}", std::process::id()))
}

/// `SHA-256` dos bytes de [`bytes_gguf_valido`] (28 bytes), calculado **fora**
/// desta implementação — `hashlib` do Python sobre
/// `bytes.fromhex("474755460300000001000000000000000200000000000000aabbccdd")`.
///
/// É o mesmo critério das constantes do NIST usadas nos testes unitários do
/// `lib.rs`: o digest esperado não pode sair de `model_digest`, senão o teste
/// seria o core a concordar consigo mesmo.
const SHA256_GGUF_SINTETICO: &str =
    "a55ef674ce3d379bbfe58c512d000f5379234a7353895221b30028df5e1a02fb";

/// `SHA-256` dos primeiros 16 bytes de [`bytes_gguf_valido`] — o caso truncado.
/// Também calculado fora, e **diferente** do digest inteiro: é isso que prova
/// que o digest foi calculado sobre os bytes que existem no arquivo, e não
/// sobre os 28 que deviam existir.
const SHA256_GGUF_TRUNCADO: &str =
    "52bfb25ce7e1a146c29e0f026214a3064cea3df531d0a17f21c76329bc942d8b";

/// A fixture do caminho feliz: um GGUF **sintético** de 28 bytes.
///
/// 24 bytes de cabeçalho — magic `GGUF`, versão 3, 1 tensor, 2 pares
/// chave-valor, tudo little-endian como `read_u32`/`read_i64` do núcleo — mais 4
/// bytes de "dados", para que o digest tenha mais do que o cabeçalho para
/// digerir.
///
/// É o mínimo que faz `parse_header` devolver `ok`, e **não** é um modelo: o
/// leitor do núcleo confere 24 bytes de estrutura e não lê um byte do bloco de
/// dados (está dito no topo de `arkhe-verify/src/gguf.rs`). O teste afirma o que
/// o comando afirma — "este cabeçalho é interpretável e estes bytes têm este
/// digest" —, não "este é um modelo válido".
///
/// O magic vem da **constante do núcleo**, não de uma string escrita aqui: se o
/// core mudasse de magic, a fixture mudava com ele e as asserções de digest
/// falhavam alto, em vez de o teste ficar a testar outra coisa.
fn bytes_gguf_valido() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(28);
    bytes.extend_from_slice(&arkhe_verify::gguf::GGUF_MAGIC);
    bytes.extend_from_slice(&3u32.to_le_bytes());
    bytes.extend_from_slice(&1i64.to_le_bytes());
    bytes.extend_from_slice(&2i64.to_le_bytes());
    bytes.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
    bytes
}

/// A app de teste: runtime falso, o **mesmo** `invoke_handler` do binário e o
/// contexto **real**.
///
/// O `invoke_handler` vem de `arkhe_ui_app_lib::registar_comandos` — a função
/// que o `run()` do binário também usa. É isso que faz estes testes valerem
/// alguma coisa: desregistar `inspect_gguf_model` **lá** põe-nos vermelhos. Se
/// o registo fosse reescrito aqui, o teste continuaria verde a exercitar uma
/// lista que a app já não tem.
///
/// O contexto real (`tauri::generate_context!()`: o `tauri.conf.json` e o
/// `capabilities/default.json` desta app) é deliberado. A alternativa óbvia —
/// `tauri::test::mock_context(noop_assets())` — zera o ACL resolvido
/// (`Resolved::default()`, logo `has_app_acl: false`) e um teste sobre ela não
/// diria nada sobre a app que existe.
///
/// Medido, não presumido: com o contexto real o comando **também** passa. O
/// `tauri-build` só insere o manifesto de app (`__app-acl__`) quando há
/// permissões de app declaradas (`tauri-build-2.6.3/src/acl.rs:405-412`), e o
/// `build.rs` desta app chama `tauri_build::build()` sem atributos — por isso
/// `has_app_manifest()` é `false`, o ramo de ACL em
/// `tauri-2.11.5/src/webview/mod.rs:1823` não dispara, e o comando chega ao
/// handler mesmo não constando de `capabilities/default.json` (que só concede
/// `core:default`). É o que o comentário daquela capability afirma; estes
/// testes são a medição.
///
/// Usar o contexto real não acrescenta fragilidade ao build: o `run()` já
/// expande `generate_context!()`, portanto o crate já não compila sem o
/// `dist-app/`.
fn app_de_teste() -> tauri::App<MockRuntime> {
    arkhe_ui_app_lib::registar_comandos(mock_builder())
        .build(tauri::generate_context!())
        .expect("construir a app de teste")
}

/// A webview de onde se despacha, com o label do `tauri.conf.json` (`main`).
///
/// O label não é decorativo: é o que o resolver de ACL consulta por janela
/// (`resolve_access(cmd, window_label, webview_label, origin)`) quando há
/// manifesto de app.
///
/// Construí-la à mão é preciso porque o `build()` **não** cria as janelas do
/// config: quem as cria é o `setup`, que só corre dentro do event loop
/// (`tauri-2.11.5/src/app.rs:2521-2530`), e nós não corremos event loop nenhum.
fn webview_de_teste(app: &tauri::App<MockRuntime>) -> tauri::WebviewWindow<MockRuntime> {
    tauri::WebviewWindowBuilder::new(app, "main", Default::default())
        .build()
        .expect("criar a webview de teste")
}

/// O `url` a partir do qual o webview fala — tem de ser **local**.
///
/// `Webview::on_message` classifica a origem (`is_local_url`,
/// `tauri-2.11.5/src/webview/mod.rs:1698`) e um `url` remoto entra no ramo que
/// rejeita qualquer comando sem `remote` capability (`webview/mod.rs:1823-1829`):
/// seria recusado por uma razão que não a que se quer medir.
/// `http://tauri.localhost` no Windows/Android e `tauri://localhost` no resto é
/// o que o próprio `mod.rs` do `tauri::test` usa no exemplo público.
fn url_local() -> tauri::Url {
    let url = if cfg!(any(windows, target_os = "android")) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    url.parse().expect("url local válida")
}

/// Despacha `cmd` pelo IPC com `corpo` no payload JSON — exactamente como o
/// frontend o envia — e devolve os **dois canais** da resposta como
/// `serde_json::Value`: `Ok(corpo)` e `Err(erro)`.
///
/// A distinção `Ok`/`Err` não é um detalhe do mock: `get_ipc_response` mapeia
/// `InvokeResponse::Ok` → `Ok` e `InvokeResponse::Err` → `Err`
/// (`tauri-2.11.5/src/test/mod.rs:310-311`), e é essa a divisão que o frontend
/// vê — `Ok` resolve a promessa, `Err` rejeita-a (o "isError" do lado do JS). Ou
/// seja: **o caso truncado deste conjunto é `Ok` com `header.ok == false`, e não
/// `Err`** — truncamento é resultado, não falha de chamada.
///
/// O `invoke_key` tem de ser o que o `mock_builder` instalou: o `on_message`
/// compara-o com o do manager e **retorna sem responder** se não bater certo
/// (`webview/mod.rs:1748-1761`), o que faria este helper rebentar no `recv` do
/// canal em vez de numa asserção — falha barulhenta, mas noutro sítio.
fn invocar(
    webview: &tauri::WebviewWindow<MockRuntime>,
    cmd: &str,
    corpo: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: url_local(),
            body: InvokeBody::Json(corpo),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|resposta| {
        resposta
            .deserialize::<serde_json::Value>()
            .expect("a resposta de um comando é JSON")
    })
}

/// 1. O caminho feliz, **pelo despacho**: o corpo vai como o frontend o envia
///    (`{ "path": ... }`) e a resposta traz o digest e o cabeçalho `ok`.
///
/// As asserções são sobre o JSON cru, não sobre um `ModelInspection`
/// desserializado, de propósito: é o JSON que o `gguf-adapter.ts` lê, e as
/// chaves (`digest_hex`, `header.ok`, …) são o contrato. Um round-trip pelo tipo
/// Rust passaria mesmo que o nome de um campo mudasse — este não. E o
/// `ModelInspection` só deriva `Serialize`; pôr-lhe `Deserialize` para agradar
/// ao teste seria mudar o tipo por causa do teste.
#[test]
fn despacho_do_caminho_feliz_traz_digest_e_cabecalho_ok() {
    let app = app_de_teste();
    let webview = webview_de_teste(&app);

    let caminho = caminho_temporario("gguf-sintetico.bin");
    std::fs::write(&caminho, bytes_gguf_valido()).expect("escrever fixture");
    let caminho = caminho.to_string_lossy().into_owned();

    let resposta = invocar(&webview, "inspect_gguf_model", serde_json::json!({"path": caminho}))
        .expect("um ficheiro legível é sucesso no IPC");

    assert_eq!(resposta["path"].as_str(), Some(caminho.as_str()));
    assert_eq!(resposta["bytes"].as_u64(), Some(28));
    assert_eq!(resposta["digest_hex"].as_str(), Some(SHA256_GGUF_SINTETICO));

    let cabecalho = &resposta["header"];
    assert_eq!(cabecalho["ok"].as_bool(), Some(true));
    assert_eq!(cabecalho["magic_ok"].as_bool(), Some(true));
    assert_eq!(cabecalho["available_bytes"].as_u64(), Some(28));
    assert_eq!(cabecalho["version"].as_u64(), Some(3));
    assert_eq!(cabecalho["tensor_count"].as_u64(), Some(1));
    assert_eq!(cabecalho["metadata_kv_count"].as_u64(), Some(2));
    assert!(
        cabecalho["error"].is_null(),
        "cabeçalho válido não tem causa de recusa: {}",
        cabecalho["error"]
    );

    std::fs::remove_file(&caminho).expect("remover fixture");
}

/// 2. O caso truncado: 16 dos 24 bytes do cabeçalho.
///
/// Duas coisas são afirmadas ao mesmo tempo, e é essa a razão de o teste
/// existir: o **digest ainda vem** (dos bytes que existem — o valor é o do
/// truncado, não o do inteiro) e o **cabeçalho diz `ok: false` com causa**. A
/// resposta é `Ok`, não `Err`: conteúdo inválido é resultado. O `expect` do
/// helper é a tradução fiel do "não vira isError" — o `Err` de
/// `get_ipc_response` é o canal que rejeita a promessa do `invoke` no frontend.
#[test]
fn despacho_do_caso_truncado_e_resultado_e_nao_erro() {
    let app = app_de_teste();
    let webview = webview_de_teste(&app);

    let mut truncado = bytes_gguf_valido();
    truncado.truncate(16);
    assert_eq!(truncado.len(), 16);

    let caminho = caminho_temporario("gguf-truncado.bin");
    std::fs::write(&caminho, &truncado).expect("escrever fixture");
    let caminho = caminho.to_string_lossy().into_owned();

    let resposta = invocar(&webview, "inspect_gguf_model", serde_json::json!({"path": caminho}))
        .expect("um cabeçalho truncado não é erro de chamada — é resultado");

    assert_eq!(resposta["bytes"].as_u64(), Some(16));
    assert_eq!(resposta["digest_hex"].as_str(), Some(SHA256_GGUF_TRUNCADO));

    let cabecalho = &resposta["header"];
    assert_eq!(cabecalho["ok"].as_bool(), Some(false));
    assert_eq!(cabecalho["magic_ok"].as_bool(), Some(true), "o magic está lá");
    assert_eq!(cabecalho["available_bytes"].as_u64(), Some(16));
    assert_eq!(cabecalho["version"].as_u64(), Some(3));
    assert_eq!(cabecalho["tensor_count"].as_u64(), Some(1));
    assert!(
        cabecalho["metadata_kv_count"].is_null(),
        "os 8 bytes dos pares chave-valor não estavam no arquivo"
    );

    let causa = cabecalho["error"]
        .as_str()
        .expect("recusa sem causa é uma recusa inútil");
    assert!(
        causa.contains("truncado") && causa.contains("16"),
        "causa inesperada: {causa}"
    );

    std::fs::remove_file(&caminho).expect("remover fixture");
}

/// 3. Um caminho que não existe: o comando devolve `Err`, e o IPC empacota-o no
///    canal de erro.
///
/// O empacotamento, medido: `Err(String)` vira
/// `InvokeError(serde_json::Value::String(msg))` — via
/// `impl<T: Serialize> From<T> for InvokeError`
/// (`tauri-2.11.5/src/ipc/mod.rs:240-245`) — e `get_ipc_response` devolve-o como
/// o `Err` de um `Result<_, serde_json::Value>`. Daí a asserção `as_str`: o erro
/// deste comando chega ao frontend como uma **string JSON**, não como um objeto.
#[test]
fn despacho_de_caminho_inexistente_chega_no_canal_de_erro() {
    let app = app_de_teste();
    let webview = webview_de_teste(&app);

    let caminho = caminho_temporario("nao-existe-mesmo.bin")
        .to_string_lossy()
        .into_owned();

    let erro = invocar(&webview, "inspect_gguf_model", serde_json::json!({"path": caminho}))
        .expect_err("caminho ausente tem de ir pelo canal de erro do IPC");

    let texto = erro
        .as_str()
        .expect("o Err de um comando Result<_, String> é uma string JSON");
    assert!(
        texto.contains("não foi possível ler"),
        "erro inesperado: {texto}"
    );
}

/// 4. Um comando que não existe é rejeitado pelo **despachante**.
///
/// Este é o teste que fixa que os outros falam com o handler registado, e não
/// com uma função chamada à mão. A mensagem exacta (`Command X not found`) é a
/// de `manager.run_invoke_handler` devolver `false`
/// (`webview/mod.rs:1910-1911`); uma recusa por ACL diria outra coisa
/// (`Command X not allowed by ACL`), e uma recusa por `invoke_key` errado **não
/// responderia nada** — o `recv` do helper rebentava.
///
/// Vale a pena notar o que ele **não** é: o nome ausente aqui é
/// `inspect_gguf_model_ausente`, um comando que nunca existiu. Provar que o
/// caminho de rejeição é o do despachante é uma coisa; provar que o registo
/// **real** está coberto é outra, e essa é a do teste 1 (que passa pelo
/// `registar_comandos`). A prova de que estes testes **pegam** um comando
/// desregistado está no relatório da tarefa: foi medida removendo
/// `inspect_gguf_model` do `registar_comandos` e correndo a suite.
#[test]
fn despacho_rejeita_comando_desconhecido() {
    let app = app_de_teste();
    let webview = webview_de_teste(&app);

    let erro = invocar(
        &webview,
        "inspect_gguf_model_ausente",
        serde_json::json!({"path": "irrelevante"}),
    )
    .expect_err("um comando não registado não pode ser despachado");

    assert_eq!(
        erro.as_str(),
        Some("Command inspect_gguf_model_ausente not found"),
        "a recusa tem de vir do despachante, não do ACL nem do payload"
    );
}

/// 5. A **forma do argumento**: o corpo vai exactamente como o frontend o envia
///    e a chave é lida; uma chave que não existe falha.
///
/// O `gguf-adapter.ts` chama `invoke('inspect_gguf_model', { path: CAMINHO })`, e
/// o teste JS daquele lado (`gguf-adapter.test.ts:79`) fixa essa chamada **do
/// lado do mock**. Isto fixa o outro lado da mesma fronteira, a sério:
/// `{ "path": ... }` chega ao `path: String` do comando.
///
/// A segunda metade — `{ "caminho": ... }` tem de falhar — é o que impede o
/// teste de ser complacente: sem ela, um corpo mal formado poderia passar se a
/// desserialização fosse ignorada. A mensagem mede também o **nome do comando no
/// fio**, que é o `stringify!` do identificador: `inspect_gguf_model`, em
/// snake_case.
///
/// # O que esta parte **não** cobre, e não vale a pena fingir que cobre
///
/// O `#[tauri::command]` aplica `heck::ToLowerCamelCase` ao nome dos argumentos
/// por omissão (`tauri-macros-2.6.3/src/command/wrapper.rs:506-511`), portanto
/// um parâmetro Rust `model_path` seria `modelPath` no fio. **Este teste não
/// exercita esse mapeamento**: `path` é uma palavra só e
/// `to_lower_camel_case("path") == "path"` — não há nada para mapear, e o que
/// passa passaria com qualquer política de renomeação. Cobri-lo exigiria um
/// comando com um argumento multi-palavra, que esta app não tem; forçar um aqui
/// seria testar um comando que não existe. Fica **por cobrir**, e está dito.
#[test]
fn despacho_le_a_chave_do_argumento_que_o_frontend_envia() {
    let app = app_de_teste();
    let webview = webview_de_teste(&app);

    let caminho = caminho_temporario("gguf-chave.bin");
    std::fs::write(&caminho, bytes_gguf_valido()).expect("escrever fixture");
    let caminho = caminho.to_string_lossy().into_owned();

    // A chave certa: aceite, e o caminho vem ecoado.
    let resposta = invocar(
        &webview,
        "inspect_gguf_model",
        serde_json::json!({"path": caminho}),
    )
    .expect("`{ path }` é a forma que o comando declara");
    assert_eq!(resposta["path"].as_str(), Some(caminho.as_str()));

    // A chave errada: recusada na desserialização, com o nome do argumento e o
    // nome do comando na causa.
    let erro = invocar(
        &webview,
        "inspect_gguf_model",
        serde_json::json!({"caminho": caminho}),
    )
    .expect_err("uma chave que o comando não declara tem de falhar");
    assert_eq!(
        erro.as_str(),
        Some(
            "invalid args `path` for command `inspect_gguf_model`: \
             command inspect_gguf_model missing required key path"
        ),
        "a causa tem de nomear o argumento e o comando"
    );

    std::fs::remove_file(&caminho).expect("remover fixture");
}
