use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SensorError {
    #[error("Communication failure: {0}")]
    Communication(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Device not found")]
    DeviceNotFound,
    #[error("Calibration failed: {0}")]
    CalibrationFailed(String),
    #[error("Timeout after {0}ms")]
    Timeout(u64),
    #[error("Device busy")]
    DeviceBusy,
}

pub type SensorResult<T> = Result<T, SensorError>;

#[async_trait]
pub trait PhiSensor: Send {
    async fn read_phi(&mut self) -> SensorResult<f64>;
    async fn calibrate(&mut self) -> SensorResult<()>;
    fn name(&self) -> &'static str;
    fn is_connected(&self) -> bool;
    async fn reconnect(&mut self) -> SensorResult<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfig {
    pub device_path: Option<String>,
    pub api_endpoint: Option<String>,
    pub bluetooth_address: Option<String>,
    pub calibration_factor: f64,
    pub timeout_ms: u64,
    pub retry_count: u32,
}

impl Default for SensorConfig {
    fn default() -> Self {
        Self {
            device_path: None,
            api_endpoint: None,
            bluetooth_address: None,
            calibration_factor: 1.0,
            timeout_ms: 1000,
            retry_count: 3,
        }
    }
}

pub struct SensorPool {
    sensors: Vec<Box<dyn PhiSensor>>,
    active_index: usize,
}

impl SensorPool {
    pub fn new() -> Self {
        Self {
            sensors: Vec::new(),
            active_index: 0,
        }
    }

    pub fn add_sensor(&mut self, sensor: Box<dyn PhiSensor>) {
        self.sensors.push(sensor);
    }

    pub async fn read_phi(&mut self) -> SensorResult<f64> {
        if self.sensors.is_empty() {
            return Err(SensorError::DeviceNotFound);
        }
        let len = self.sensors.len();
        for i in 0..len {
            let idx = (self.active_index + i) % len;
            if self.sensors[idx].is_connected() {
                match self.sensors[idx].read_phi().await {
                    Ok(phi) => {
                        self.active_index = idx;
                        return Ok(phi);
                    }
                    Err(e) => {
                        tracing::warn!("Sensor {} failed: {}", self.sensors[idx].name(), e);
                        continue;
                    }
                }
            }
        }
        Err(SensorError::DeviceNotFound)
    }

    pub async fn calibrate_all(&mut self) -> Vec<(&'static str, SensorResult<()>)> {
        let mut results = Vec::new();
        for sensor in &mut self.sensors {
            let name = sensor.name();
            let result = sensor.calibrate().await;
            results.push((name, result));
        }
        results
    }
}
