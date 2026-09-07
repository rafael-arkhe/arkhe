//! Stub temporário da integração MCP (Model Context Protocol).
//!
//! Substitui o `arkhe-mcp-client` externo (sem evidência pública de existência
//! ou manutenção) por um cliente HTTP `reqwest` enxuto. Documentado em
//! `docs/ADR-001-substituicao-mcp.md`.
//!
//! Esta implementação é um placeholder testável; pode ser trocada por um
//! cliente MCP completo quando o protocolo estabilizar.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Erros do stub MCP.
#[derive(Error, Debug)]
pub enum McpError {
    /// Falha na requisição HTTP.
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// Resposta HTTP com status não-2xx.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

/// Tipo de resultado do módulo.
pub type Result<T> = std::result::Result<T, McpError>;

/// Payload de handover entre componentes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoverPayload {
    /// Componente de origem.
    pub from: String,
    /// Componente de destino.
    pub to: String,
    /// Corpo do handover.
    pub payload: String,
    /// Latência observada (ms).
    pub latency_ms: u64,
}

/// Relatório de qualidade retornado pelo endpoint `/quality`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    /// Score ponderado geral em [0,1].
    pub overall: f64,
    /// Estabilidade em [0,1].
    pub stability: f64,
    /// Taxa de sucesso em [0,1].
    pub success_rate: f64,
}

/// Cliente HTTP minimalista para os endpoints MCP usados pelo crate.
pub struct McpClient {
    client: Client,
    base_url: String,
}

impl McpClient {
    /// Cria o cliente apontando para `base_url`.
    #[must_use]
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }

    /// Envia um handover via `POST /handover`.
    pub async fn submit_handover(&self, handover: &HandoverPayload) -> Result<()> {
        let url = format!("{}/handover", self.base_url);
        let response = self.client.post(&url).json(handover).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(McpError::InvalidResponse(format!("Status: {status}, Body: {body}")));
        }
        Ok(())
    }

    /// Consulta a qualidade via `GET /quality`.
    pub async fn get_quality(&self) -> Result<QualityReport> {
        let url = format!("{}/quality", self.base_url);
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(McpError::InvalidResponse(format!("Status: {status}, Body: {body}")));
        }
        Ok(response.json::<QualityReport>().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_submit_handover_success() {
        let mock_server = MockServer::start().await;
        let client = McpClient::new(&mock_server.uri());

        Mock::given(method("POST"))
            .and(path("/handover"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let handover = HandoverPayload {
            from: "test".to_string(),
            to: "target".to_string(),
            payload: "test".to_string(),
            latency_ms: 10,
        };

        assert!(client.submit_handover(&handover).await.is_ok());
    }

    #[tokio::test]
    async fn test_submit_handover_error() {
        let mock_server = MockServer::start().await;
        let client = McpClient::new(&mock_server.uri());

        Mock::given(method("POST"))
            .and(path("/handover"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let handover = HandoverPayload {
            from: "test".to_string(),
            to: "target".to_string(),
            payload: "test".to_string(),
            latency_ms: 10,
        };

        assert!(client.submit_handover(&handover).await.is_err());
    }

    #[tokio::test]
    async fn test_get_quality_success() {
        let mock_server = MockServer::start().await;
        let client = McpClient::new(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/quality"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(QualityReport {
                    overall: 0.9,
                    stability: 0.85,
                    success_rate: 0.95,
                }),
            )
            .mount(&mock_server)
            .await;

        let report = client.get_quality().await.unwrap();
        assert_eq!(report.overall, 0.9);
        assert_eq!(report.stability, 0.85);
        assert_eq!(report.success_rate, 0.95);
    }
}