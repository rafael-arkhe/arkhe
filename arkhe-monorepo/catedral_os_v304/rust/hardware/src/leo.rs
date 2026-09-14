use super::hal::{PhiSensor, SensorError, SensorResult};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::info;

pub struct LEOApiDriver {
    client: Client,
    endpoint: String,
    token: String,
    connected: bool,
}

impl LEOApiDriver {
    pub fn new(endpoint: &str, token: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
        info!("LEOSatellite driver criado para {}", endpoint);
        Self {
            client,
            endpoint: endpoint.to_string(),
            token: token.to_string(),
            connected: true,
        }
    }
}

#[async_trait]
impl PhiSensor for LEOApiDriver {
    async fn read_phi(&mut self) -> SensorResult<f64> {
        let response = self
            .client
            .get(&self.endpoint)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| SensorError::Communication(e.to_string()))?;

        if !response.status().is_success() {
            self.connected = false;
            return Err(SensorError::Communication(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let data: Value = response
            .json()
            .await
            .map_err(|e| SensorError::InvalidData(e.to_string()))?;

        let phi = data["phi"]
            .as_f64()
            .or_else(|| data["data"]["phi"].as_f64())
            .or_else(|| data["value"].as_f64())
            .ok_or_else(|| SensorError::InvalidData("phi field not found".into()))?;

        if !phi.is_finite() || phi < 0.0 || phi > 1.0 {
            return Err(SensorError::InvalidData(format!(
                "Phi out of range: {}",
                phi
            )));
        }

        Ok(phi)
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
                "Remote calibration failed: HTTP {}",
                response.status()
            )));
        }

        info!("LEOSatellite calibrado remotamente");
        Ok(())
    }

    fn name(&self) -> &'static str {
        "LEOSatellite"
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    async fn reconnect(&mut self) -> SensorResult<()> {
        match self.client.get(&self.endpoint).send().await {
            Ok(resp) if resp.status().is_success() => {
                self.connected = true;
                info!("LEOSatellite reconectado");
                Ok(())
            }
            Ok(resp) => {
                self.connected = false;
                Err(SensorError::Communication(format!(
                    "Reconnect failed: HTTP {}",
                    resp.status()
                )))
            }
            Err(e) => {
                self.connected = false;
                Err(SensorError::Communication(e.to_string()))
            }
        }
    }
}
