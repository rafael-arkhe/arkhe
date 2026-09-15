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

fn main() {
    tauri_build::build();
}
