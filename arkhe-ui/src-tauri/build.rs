//! O script de build da app.
//!
//! `tauri_build::build()` é o que lê o `tauri.conf.json`, resolve os
//! `frontendDist`/`devUrl` e gera o código de contexto que
//! `tauri::generate_context!()` consome no `src/lib.rs` — inclusive os recursos
//! do Windows (manifest e versão do executável). Sem esta chamada o
//! `generate_context!` não compila.
//!
//! Nota: a ausência de `bundle.icon` no `tauri.conf.json` é deliberada e está
//! registada no README deste directorio. Este script não precisa dos ícones —
//! quem precisa é o empacotador (`tauri build`), e não o `cargo check`.

fn main() {
    tauri_build::build();
}
