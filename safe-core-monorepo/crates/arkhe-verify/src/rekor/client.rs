//! O adaptador HTTP para uma instância Rekor.
//!
//! **Este é o único módulo da crate que faz rede.** Nada mais aqui abre socket:
//! as demais funções são puras sobre dados já obtidos, e é por isso que os
//! testes conseguem exercitar o modelo e as verificações inteiros sem tocar a
//! internet. Os dois métodos de [`RekorClient`] são as duas chamadas de rede
//! que existem nesta crate.
//!
//! ```
//! use arkhe_verify::RekorClient;
//!
//! // A instância pública do Sigstore é o padrão.
//! assert_eq!(RekorClient::new().base_url(), "https://rekor.sigstore.dev");
//!
//! // E é configurável: um espelho, um proxy, ou um servidor de teste.
//! let local = RekorClient::with_base_url("http://127.0.0.1:8080/");
//! assert_eq!(local.base_url(), "http://127.0.0.1:8080");
//! ```
//!
//! # Nenhuma chamada de rede nos testes
//!
//! O adaptador é exercitado contra um servidor **local** (`mockito`), como faz
//! o `arkhe-orcid`, e nunca contra a instância pública. Ver
//! `tests/rekor_mock.rs`. A instância padrão é, portanto, um valor de
//! configuração documentado — não um endereço que esta crate já tenha
//! consultado.

use std::collections::BTreeMap;

use crate::error::RekorError;
use crate::rekor::{ConsistencyProof, LogEntry};

/// A instância pública do Sigstore, que é o que código de produção quer.
///
/// Usada por [`RekorClient::new`]; quem precisa apontar para outro host (um
/// espelho, um proxy, um servidor de teste) usa [`RekorClient::with_base_url`].
pub const DEFAULT_BASE_URL: &str = "https://rekor.sigstore.dev";

/// Fala com a API de log de uma instância Rekor.
///
/// Barato de clonar (o `reqwest::Client` interno tem pool de conexões) e sem
/// estado por consulta: o mesmo cliente busca qualquer número de entradas.
#[derive(Clone, Debug)]
pub struct RekorClient {
    base_url: String,
    http: reqwest::Client,
}

impl RekorClient {
    /// Um cliente apontado para a instância pública do Sigstore
    /// ([`DEFAULT_BASE_URL`]).
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    /// Um cliente apontado para `url`, com a barra final removida.
    ///
    /// É como os testes apontam o cliente para um servidor local, e como um
    /// operador o aponta para um espelho.
    pub fn with_base_url(url: impl Into<String>) -> Self {
        Self {
            base_url: url.into().trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    /// A URL base contra a qual este cliente emite requisições, sem barra
    /// final.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Busca a entrada de log em `log_index`.
    ///
    /// Emite `GET {base}/api/v1/log/entries?logIndex={log_index}`.
    ///
    /// A resposta do Rekor é **um objeto JSON com um único par chave-valor**,
    /// cuja chave é o UUID da entrada e cujo valor é a [`LogEntry`]; este
    /// método desembrulha o mapa e devolve a entrada. Uma resposta com zero ou
    /// mais de um elemento é [`RekorError::MalformedLogEntry`], não uma escolha
    /// arbitrária do primeiro.
    ///
    /// | Resposta | Erro |
    /// |---|---|
    /// | `2xx` | interpretada; corpo que não é o mapa esperado é [`RekorError::MalformedLogEntry`] |
    /// | `404` | [`RekorError::NotFound`] |
    /// | `429` | [`RekorError::RateLimited`] |
    /// | `5xx` | [`RekorError::Provider`] |
    /// | outro `não-2xx` | [`RekorError::UnexpectedResponse`] |
    /// | sem resposta | [`RekorError::Transport`] |
    pub async fn log_entry(&self, log_index: u64) -> Result<LogEntry, RekorError> {
        let url = entries_url(&self.base_url, log_index);
        let response = self.get(&url, RekorError::NotFound { log_index }).await?;

        let entries: BTreeMap<String, LogEntry> =
            response.json().await.map_err(|err| RekorError::MalformedLogEntry {
                reason: format!(
                    "GET {url} não devolveu um objeto JSON de UUID para entrada: {err}"
                ),
            })?;

        if entries.len() != 1 {
            return Err(RekorError::MalformedLogEntry {
                reason: format!(
                    "GET {url} devolveu {} entradas; esperava exatamente uma",
                    entries.len()
                ),
            });
        }

        entries
            .into_values()
            .next()
            .ok_or_else(|| RekorError::MalformedLogEntry {
                reason: format!("GET {url} devolveu um mapa que não pôde ser desembrulhado"),
            })
    }

    /// Busca a prova de consistência entre dois tamanhos de árvore.
    ///
    /// Emite
    /// `GET {base}/api/v1/log/proof?firstSize={first_size}&lastSize={last_size}&treeSize={tree_size}`.
    ///
    /// Devolve os bytes que o log publica. **Não verifica a consistência**: o
    /// core do `arkhe-verify-wasm` cobre hash, assinatura Ed25519, inclusão
    /// RFC 6962 e quórum — não cobre prova de consistência. Verificar exigiria
    /// um verificador novo no core, e escrevê-lo aqui seria exatamente a
    /// duplicação que a arquitetura de "um core, duas cascas" existe para
    /// impedir. Ver a nota em [`ConsistencyProof`].
    pub async fn consistency_proof(
        &self,
        first_size: u64,
        last_size: u64,
        tree_size: u64,
    ) -> Result<ConsistencyProof, RekorError> {
        let url = proof_url(&self.base_url, first_size, last_size, tree_size);
        let response = self
            .get(
                &url,
                RekorError::UnexpectedResponse {
                    reason: format!(
                        "GET {url} respondeu 404: o log não tem essa prova de consistência"
                    ),
                },
            )
            .await?;

        response.json().await.map_err(|err| {
            RekorError::UnexpectedResponse {
                reason: format!("GET {url} não devolveu uma prova de consistência: {err}"),
            }
        })
    }

    /// Emite o `GET` e mapeia o status, sem interpretar o corpo.
    ///
    /// O 404 é o único status cujo significado depende de quem chamou — "esse
    /// índice não existe" é [`RekorError::NotFound`] quando se sabe o índice
    /// pedido, e uma resposta inesperada quando não se sabe —, então ele é
    /// passado como valor em vez de ser adivinhado a partir do texto do erro.
    async fn get(
        &self,
        url: &str,
        on_not_found: RekorError,
    ) -> Result<reqwest::Response, RekorError> {
        let response = self
            .http
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| RekorError::Transport(format!("GET {url}: {err}")))?;

        let status = response.status().as_u16();
        match status {
            200..=299 => Ok(response),
            404 => Err(on_not_found),
            429 => Err(RekorError::RateLimited),
            500..=599 => Err(RekorError::Provider { status }),
            other => Err(RekorError::UnexpectedResponse {
                reason: format!("GET {url} respondeu HTTP {other}"),
            }),
        }
    }
}

impl Default for RekorClient {
    fn default() -> Self {
        Self::new()
    }
}

/// `GET {base}/api/v1/log/entries?logIndex={log_index}`.
fn entries_url(base_url: &str, log_index: u64) -> String {
    format!("{base_url}/api/v1/log/entries?logIndex={log_index}")
}

/// `GET {base}/api/v1/log/proof?firstSize=&lastSize=&treeSize=`.
fn proof_url(base_url: &str, first_size: u64, last_size: u64, tree_size: u64) -> String {
    format!(
        "{base_url}/api/v1/log/proof?firstSize={first_size}&lastSize={last_size}&treeSize={tree_size}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_base_url_is_the_sigstore_public_instance() {
        assert_eq!(RekorClient::new().base_url(), DEFAULT_BASE_URL);
        assert_eq!(RekorClient::default().base_url(), DEFAULT_BASE_URL);
    }

    #[test]
    fn a_trailing_slash_does_not_double_up_in_the_url() {
        let client = RekorClient::with_base_url("http://127.0.0.1:1/");
        assert_eq!(client.base_url(), "http://127.0.0.1:1");
        assert_eq!(
            entries_url(client.base_url(), 42),
            "http://127.0.0.1:1/api/v1/log/entries?logIndex=42"
        );
    }

    #[test]
    fn the_entries_url_uses_the_log_index_query_parameter() {
        assert_eq!(
            entries_url("https://rekor.sigstore.dev", 7),
            "https://rekor.sigstore.dev/api/v1/log/entries?logIndex=7"
        );
    }

    #[test]
    fn the_proof_url_carries_the_three_sizes() {
        assert_eq!(
            proof_url("https://rekor.sigstore.dev", 10, 20, 30),
            "https://rekor.sigstore.dev/api/v1/log/proof?firstSize=10&lastSize=20&treeSize=30"
        );
    }

    /// Não há servidor nesta porta. Este é o único teste que deixa a crate
    /// tentar uma conexão, e ele **falha de propósito** — nada é transmitido, e
    /// o que se verifica é o mapeamento do erro de transporte.
    #[tokio::test]
    async fn a_refused_connection_is_a_transport_error() {
        let client = RekorClient::with_base_url("http://127.0.0.1:1/");
        assert!(matches!(
            client.log_entry(0).await,
            Err(RekorError::Transport(_))
        ));
        assert!(matches!(
            client.consistency_proof(1, 2, 3).await,
            Err(RekorError::Transport(_))
        ));
    }
}
