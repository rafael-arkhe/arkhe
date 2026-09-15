//! Fase 4, verificação 5: a resolução passa mesmo a vir dos relays declarados?
//!
//! Os `htree install --check` das verificações 1–4 correm no CLI, e o CLI lê os
//! relays do `~/.hashtree/config.toml` do utilizador — **não** do
//! `tauri.conf.json` da app. Servem para provar que o release publicado resolve,
//! não para provar que resolve *por causa dos relays declarados*.
//!
//! Este teste fecha essa distância: constrói o `UpdaterContext` do plugin a
//! partir do `tauri.conf.json` **real**, pelo mesmo `Config` que o
//! `generate_context!` desserializa em tempo de compilação, e corre o mesmo
//! `check()`. Se a lista declarada fosse insuficiente, este teste falhava.
//!
//! Não é um teste de rede por fingir: bate mesmo nos relays. Corre com
//! `cargo test --test updater_relays -- --nocapture`.

use tauri_plugin_hashtree_updater::{Config, UpdaterContext};

fn real_config() -> Config {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json");
    let raw = std::fs::read_to_string(path).expect("ler tauri.conf.json");
    let doc: serde_json::Value = serde_json::from_str(&raw).expect("tauri.conf.json e JSON valido");
    let section = doc["plugins"]["hashtree-updater"].clone();
    serde_json::from_value(section).expect("desserializar plugins.hashtree-updater")
}

/// A lista declarada tem exactamente as três entradas medidas, e `blossomServers`
/// continua ausente. Este é o invariante que a configuração promete.
#[test]
fn a_lista_declarada_e_a_medida() {
    let c = real_config();

    assert_eq!(
        c.relays,
        vec![
            "wss://relay.snort.social".to_string(),
            "wss://temp.iris.to".to_string(),
            "wss://relay.primal.net".to_string(),
        ],
        "a lista de relays declarada mudou"
    );

    // Os dois que NAO existem / nao servem a resolucao nao podem reaparecer.
    assert!(
        !c.relays.iter().any(|r| r.contains("relay.iris.to")),
        "relay.iris.to nao resolve (NXDOMAIN) e nao pode ser declarado"
    );
    assert!(
        !c.relays.iter().any(|r| r.contains("damus")),
        "damus.io foi retirado: falha intermitente e serve uma raiz obsoleta"
    );

    // Substitui (nao acrescenta) os servidores por omissao — declarar seria piorar.
    assert!(
        c.blossom_servers.is_empty(),
        "blossomServers tem de continuar por declarar"
    );

    // A referencia nao pode ganhar o segmento `stable`.
    let reference = c.reference.expect("reference e obrigatoria");
    assert_eq!(
        reference,
        "htree://npub1ql0eyuuwsvju0g65cge03c762f4qm9yprl0aymru6lszrzdd0tqsvmcnnk/releases%2Farkhe-os/latest"
    );
    assert!(
        !reference.contains("/stable/"),
        "`stable` e o campo `channel` do manifesto, nao um segmento de caminho"
    );
}

/// O `check()` real, contra os relays declarados, a resolver o release publicado.
///
/// **`#[ignore]` por uma razão medida, não por ser lento.** Um binário de teste
/// não é a app: o `rustls` exige um `CryptoProvider` de processo, e quem o
/// instala é o runtime do Tauri, em `tauri-2.11.5/src/protocol/tauri.rs:45`
/// (`rustls::crypto::ring::default_provider().install_default()`). Num
/// `cargo test` essa linha nunca corre, e o primeiro handshake TLS do tokio
/// aborta com *"Could not automatically determine the process-level
/// CryptoProvider from Rustls crate features"* — antes de chegar a falar com
/// relay nenhum. Não é a lista de relays que falha; é não haver provider.
///
/// Fechar isto pedia uma aresta directa para o `rustls` com a feature `ring` em
/// `[dev-dependencies]`. Não foi feito de propósito: o binário 0.2.2 já estava
/// construído, varrido e publicado quando este teste foi escrito, e mexer na
/// unificação de features do `rustls` arriscava mudar o que foi publicado.
/// A verificação 5 foi feita por outra via — ver o `README.md`.
///
/// Como correr, se um dia a aresta existir:
/// `cargo test --test updater_relays -- --ignored --nocapture`
#[test]
#[ignore = "precisa do CryptoProvider de processo que o runtime do Tauri instala; ver o doc-comment"]
fn os_relays_declarados_resolvem_a_raiz_publicada() {
    let c = real_config();
    let relays = c.relays.clone();

    let ctx = UpdaterContext::new(c, "0.0.0".to_string());
    let checked = tauri::async_runtime::block_on(ctx.check())
        .expect("o check nao devia dar erro de rede com estes relays");

    let update = checked.expect("o release devia resolver a partir dos relays declarados");

    println!("relays declarados : {relays:?}");
    println!("versao resolvida  : {}", update.version);
    println!("asset             : {} ({})", update.asset_name, update.asset_kind);

    assert_eq!(update.version, "0.2.2", "resolver devia dar a versao publicada");
    assert_eq!(update.asset_kind, "binary");
    assert!(update.update_available, "0.0.0 -> 0.2.2 e mais recente");
}
