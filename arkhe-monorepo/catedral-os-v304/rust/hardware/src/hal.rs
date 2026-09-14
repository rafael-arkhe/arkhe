// Catedral OS v304.0 — Hardware Abstraction Layer (HAL). I358.
//
// A leitura de Φ é contínua e confiável se, e somente se, o sensor
// implementar esta interface e a validação Lean (ProductionHardware.lean,
// I358) verificar a sequência de leituras.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SensorError {
    #[error("falha de comunicação: {0}")]
    Communication(String),
    #[error("dados inválidos: {0}")]
    InvalidData(String),
    #[error("dispositivo não encontrado")]
    DeviceNotFound,
    #[error("calibração falhou: {0}")]
    CalibrationFailed(String),
    #[error("timeout: {0}")]
    Timeout(String),
}

pub type SensorResult<T> = Result<T, SensorError>;

#[async_trait]
pub trait PhiSensor: Send + Sync {
    async fn read_phi(&mut self) -> SensorResult<f64>;
    async fn calibrate(&mut self) -> SensorResult<()>;
    fn name(&self) -> &'static str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfig {
    pub device_path: Option<String>,
    pub api_endpoint: Option<String>,
    pub bluetooth_address: Option<String>,
    pub calibration_factor: f64,
    pub timeout_ms: u64,
}

impl Default for SensorConfig {
    fn default() -> Self {
        Self {
            device_path: None,
            api_endpoint: None,
            bluetooth_address: None,
            calibration_factor: 1.0,
            timeout_ms: 1000,
        }
    }
}

/// Normaliza uma leitura bruta para o intervalo constitucional de Φ.
/// Gap-1: 0.0 < Φ ≤ 0.999900 (limite superior constitucional).
pub fn clamp_phi(raw: f64) -> f64 {
    raw.clamp(0.0, 0.999900)
}