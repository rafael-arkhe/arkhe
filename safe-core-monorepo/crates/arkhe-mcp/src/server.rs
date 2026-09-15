//! O handler MCP — as quatro ferramentas registradas no roteador do `rmcp`.
//!
//! Este módulo é o único ponto do crate que conhece o protocolo. Cada método
//! `#[tool]` abaixo faz três coisas e nenhuma a mais: recebe os argumentos já
//! desserializados pelo `rmcp`, chama a função correspondente de [`crate::tools`]
//! e traduz o resultado na resposta MCP.
//!
//! # O que o `rmcp` gera daqui
//!
//! - **Os schemas.** O `#[tool]` deriva o JSON Schema de cada argumento de
//!   `Parameters<T>` a partir do `#[derive(JsonSchema)]` de `T` — os tipos de
//!   [`crate::tools`] —, então o schema que o agente vê e o tipo que o handler
//!   recebe são a mesma declaração. Não há um segundo schema escrito à mão para
//!   divergir.
//! - **O roteamento.** O `#[tool_router]` monta o `ToolRouter` a partir dos
//!   métodos, e o `#[tool_handler]` implementa `call_tool`/`list_tools`/
//!   `get_tool`/`get_info`. Um nome de ferramenta que não existe vira
//!   `method not found` do próprio SDK, sem passar por este código.
//!
//! # A tradução do resultado
//!
//! [`into_response`] é a fronteira descrita na documentação do [`crate`]: um relatório
//! (inclusive reprovado) vira resultado estruturado; uma entrada que não se
//! interpretou vira erro estruturado. As duas formas carregam o JSON em
//! `structured_content`, então o agente lê os mesmos campos nos dois casos.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};

use crate::tools::{self, AttestationArgs, InclusionArgs, Sha256Args, SignatureArgs, ToolOutcome};

/// O servidor MCP do Arkhe.
///
/// Um tipo sem estado, e isso é deliberado: as quatro ferramentas são funções
/// puras sobre os argumentos — não há sessão, cache nem acúmulo entre chamadas.
/// Verificar duas vezes a mesma prova dá a mesma resposta, e uma chamada não
/// pode influenciar a seguinte.
#[derive(Debug, Clone, Copy, Default)]
pub struct ArkheMcp;

impl ArkheMcp {
    /// Um servidor com as quatro ferramentas registradas.
    pub const fn new() -> Self {
        Self
    }
}

#[tool_router]
impl ArkheMcp {
    /// Confere o digest SHA-256 de um payload.
    ///
    /// Delega a [`arkhe_verify::verify_sha256`]. Um digest declarado que não é
    /// hex não é rejeitado aqui: o core o reporta como veredito reprovado e
    /// devolve o digest calculado de qualquer forma.
    #[tool(
        description = "Verify that SHA-256(payload) equals a declared digest. \
                       Runs Arkhe's core verification unchanged and returns its report: \
                       ok, computed_hex, expected_hex, and the cause when they differ. \
                       A negative verdict is a result, not an error."
    )]
    fn arkhe_verify_sha256(&self, Parameters(args): Parameters<Sha256Args>) -> CallToolResult {
        into_response(tools::verify_sha256(args))
    }

    /// Confere uma assinatura Ed25519 e que a chave está no trust root.
    ///
    /// Delega a [`arkhe_verify::verify_signature`]. Os dois fatos são reportados
    /// em campos separados (`ok` e `trusted`): uma assinatura válida por uma
    /// chave não confiável é uma rejeição, e o relatório diz qual dos dois
    /// falhou.
    #[tool(
        description = "Verify an Ed25519 signature over a message and that the signing key is \
                       in the Arkhe trust root. Reports `ok` and `trusted` separately: a \
                       cryptographically valid signature from an untrusted key is a rejection, \
                       and the report says which of the two failed."
    )]
    fn arkhe_verify_signature(
        &self,
        Parameters(args): Parameters<SignatureArgs>,
    ) -> CallToolResult {
        into_response(tools::verify_signature(args))
    }

    /// Confere uma prova de inclusão Merkle RFC 6962 contra uma raiz conhecida.
    ///
    /// Delega a [`arkhe_verify::verify_inclusion`]. Não confere *prova de
    /// consistência* entre tamanhos de árvore: o core não tem esse verificador,
    /// e escrever um aqui seria duplicar lógica de verificação fora do core.
    #[tool(
        description = "Verify a Merkle RFC 6962 inclusion proof for a leaf against a known tree \
                       root. Returns ok, leaf_index, tree_size, the declared root, and the cause \
                       when the proof does not reconstruct it."
    )]
    fn arkhe_verify_inclusion(
        &self,
        Parameters(args): Parameters<InclusionArgs>,
    ) -> CallToolResult {
        into_response(tools::verify_inclusion(args))
    }

    /// O pipeline completo de atestação — as quatro verificações compostas.
    ///
    /// Delega a [`arkhe_verify::verify_attestation`], que avalia os quatro
    /// estágios de forma independente e reporta cada um. É por aqui que o
    /// quórum de witnesses é verificado: a ferramenta recebe o documento com os
    /// witnesses e o `quorum_threshold`, e o pipeline chama o quórum do core.
    #[tool(
        description = "Run Arkhe's full attestation pipeline over an attestation document: \
                       SHA-256 of the payload, the signer's Ed25519 signature against the trust \
                       root, Merkle RFC 6962 inclusion, and the witness quorum. The four stages \
                       are evaluated independently and reported separately, so an attestation \
                       that fails only the quorum comes back with quorum=false and the other \
                       three true."
    )]
    fn arkhe_verify_attestation(
        &self,
        Parameters(args): Parameters<AttestationArgs>,
    ) -> CallToolResult {
        into_response(tools::verify_attestation(args))
    }
}

#[tool_handler(
    name = "arkhe-mcp",
    instructions = "Arkhe verification server. Every tool delegates to Arkhe's core \
                    verification (the native shell of arkhe-verify) and returns that \
                    function's report unchanged. A report with ok=false means the \
                    verification ran and rejected; a result with stage=\"input\" means the \
                    arguments could not be interpreted and no verification happened."
)]
impl ServerHandler for ArkheMcp {}

/// Traduz o resultado de uma ferramenta na resposta MCP.
///
/// A distinção é a documentada em [`crate`], e o `rmcp` a torna explícita:
/// `CallToolResult::structured` é um resultado que o chamador lê,
/// `CallToolResult::structured_error` é uma falha que o chamador vê. Nenhum dos
/// dois é `Err(McpError)` — um erro de protocolo não chega à interface do
/// usuário, e uma verificação que reprovou (ou uma entrada que não se
/// interpretou) precisa chegar.
fn into_response(outcome: ToolOutcome) -> CallToolResult {
    match outcome {
        ToolOutcome::Report(value) => CallToolResult::structured(value),
        ToolOutcome::Invalid(value) => CallToolResult::structured_error(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rmcp::handler::server::router::tool::ToolRouter;
    use serde_json::Value;

    /// Os nomes que o servidor publica — a lista inteira, e nada além dela.
    const TOOLS: [&str; 4] = [
        "arkhe_verify_attestation",
        "arkhe_verify_inclusion",
        "arkhe_verify_sha256",
        "arkhe_verify_signature",
    ];

    fn router() -> ToolRouter<ArkheMcp> {
        ArkheMcp::tool_router()
    }

    #[test]
    fn the_router_registers_exactly_the_documented_tools() {
        let mut names: Vec<String> = router()
            .list_all()
            .iter()
            .map(|tool| tool.name.to_string())
            .collect();
        names.sort();

        assert_eq!(
            names, TOOLS,
            "o roteador tem de publicar exatamente estas quatro ferramentas"
        );
    }

    #[test]
    fn every_tool_documents_itself_and_declares_its_arguments() {
        let router = router();

        for name in TOOLS {
            let tool = router
                .get(name)
                .unwrap_or_else(|| panic!("`{name}` não está no roteador"));

            let description = tool.description.as_deref().unwrap_or_default();
            assert!(
                !description.is_empty(),
                "`{name}` sem descrição: o agente não tem como saber o que ela faz"
            );

            // O schema é derivado de `Parameters<T>`, então cada campo do tipo de
            // argumentos tem de aparecer aqui. A lista é a declarada no corpo da
            // ferramenta, não o que o `rmcp` por acaso gerou.
            let expected: &[&str] = match name {
                "arkhe_verify_sha256" => &["payload_b64", "expected_hex"],
                "arkhe_verify_signature" => &[
                    "message_b64",
                    "signature_hex",
                    "public_key_hex",
                    "trust_root",
                ],
                "arkhe_verify_inclusion" => &[
                    "leaf_b64",
                    "leaf_index",
                    "tree_size",
                    "proof_hex",
                    "root_hex",
                ],
                "arkhe_verify_attestation" => &["attestation_json", "trust_root"],
                other => panic!("ferramenta sem schema declarado no teste: {other}"),
            };

            let properties = tool
                .input_schema
                .get("properties")
                .and_then(Value::as_object)
                .expect("o schema gerado tem `properties`");

            for field in expected {
                assert!(
                    properties.contains_key(*field),
                    "`{name}` não declara o argumento `{field}`"
                );
            }
            assert_eq!(
                properties.len(),
                expected.len(),
                "`{name}` declara argumentos além dos documentados"
            );
        }
    }

    #[test]
    fn the_handler_advertises_the_tools_capability() {
        let info = ArkheMcp.get_info();

        assert!(
            info.capabilities.tools.is_some(),
            "sem a capability de tools o cliente não chama nenhuma ferramenta"
        );
        assert_eq!(info.server_info.name, "arkhe-mcp");
        assert!(
            info.instructions.is_some(),
            "as instruções viajam no handshake e dizem como ler os dois tipos de resposta"
        );
    }
}
