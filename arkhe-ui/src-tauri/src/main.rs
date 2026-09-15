// Não abre uma segunda consola no Windows em release.
//
// O binário é um `windows_subsystem = "windows"`: sem isto, um build de release
// traria uma janela de consola preta ao lado da janela da app. Em debug a
// consola fica — é onde os `println!` do webview e os pânicos aparecem.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Todo o trabalho está na biblioteca: o `tauri::generate_context!` do
    // `build.rs` gera código que precisa de um crate com nome estável, e este
    // binário é só o ponto de entrada.
    arkhe_ui_app_lib::run()
}
