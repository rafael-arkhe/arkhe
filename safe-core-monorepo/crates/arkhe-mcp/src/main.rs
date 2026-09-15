//! O binário: fala MCP por **stdio** e serve [`ArkheMcp`].
//!
//! É a camada mais fina do crate, e de propósito. O que ele faz é o que o
//! transporte exige e nada mais: entrega os descritores de entrada e saída
//! padrão ao `rmcp` e espera o serviço terminar. A escolha do stdio não é
//! incidental — é o transporte que um cliente MCP (um agente, um editor, uma
//! IDE) usa para lançar um servidor local como processo filho, sem porta, sem
//! socket e sem autenticação de rede.
//!
//! Nada é escrito em `stdout` além do protocolo: o `stdout` **é** o canal do
//! MCP, e um `println!` de depuração aqui corromperia o fluxo. Se precisar
//! depurar, use `stderr`.
//!
//! # Executar
//!
//! ```text
//! cargo run -p arkhe-mcp
//! ```
//!
//! O processo fica lendo o `stdin` até o cliente fechar o canal; a saída é o
//! `QuitReason` de [`rmcp::service::RunningService::waiting`], tratado como fim
//! normal do serviço.

use arkhe_mcp::ArkheMcp;
use rmcp::transport::stdio;
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = ArkheMcp::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
