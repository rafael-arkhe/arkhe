// Catedral OS v304.0 — Driver de satélite LEO (API REST autenticada).
// I358: a leitura remota chega via bearer token e é validada no clamp Gap-1.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use tracing::info;

use crate::hal::{clamp_phi, PhiSensor, SensorError, SensorResult};

pub struct LeoApiDriver {
    client: Client,
    endpoint: String,
    token: String,
}

impl LeoApiDriver {
    pub fn new(endpoint: &str, token: &str) -> Self {
        Self {
            client: Client::new(),
            endpoint: endpoint.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }
}

#[async_trait]
impl PhiSensor for LeoApiDriver {
    async fn read_phi(&mut self) -> SensorResult<f64> {
        let response = self
            .client
            .get(&self.endpoint)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| SensorError::Communication(e.to_string()))?;
        if !response.status().is_success() {
            return Err(SensorError::Communication(format!(
                "LEO respondeu HTTP {}",
                response.status()
            )));
        }
        let data: Value = response
            .json()
            .await
            .map_err(|e| SensorError::InvalidData(e.to_string()))?;
        let phi = data
            .get("phi")
            .and_then(Value::as_f64)
            .or_else(|| data.get("data").and_then(|d| d.get("phi")).and_then(Value::as_f64))
            .ok_or_else(|| SensorError::InvalidData("campo 'phi' não encontrado na resposta LEO".into()))?;
        Ok(clamp_phi(phi))
    }

    async fn calibrate(&mut self) -> SensorResult<()> {
        let response = self
            .client
            .post(format!("{}/calibrate", self.endpoint))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| SensorError::Communication(e.to_string()))?;
        if !response.status().is_success() {
            return Err(SensorError::CalibrationFailed(format!(
                "calibração remota recusada: HTTP {}",
                response.status()
            )));
        }
        info!("satélite LEO calibrado remotamente");
        Ok(())
    }

    fn name(&self) -> &'static str {
        "LEOSatellite"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn clamps_phi_at_constitutional_bound() {
        assert_eq!(super::clamp_phi(1.5), 0.999900);
        assert_eq!(super::clamp_phi(-0.2), 0.0);
    }
}