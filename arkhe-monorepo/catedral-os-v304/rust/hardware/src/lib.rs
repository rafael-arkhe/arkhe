// Catedral OS v304.0 — HAL física (I358).
//
// Drivers: chip fotônico 3D (UART), satélite LEO (REST), Svetoch (BLE/simulado).
// A validação física das leituras é formalizada em lean/ProductionHardware.lean.

pub mod hal;
pub mod leo;
pub mod photonic;
pub mod svetoch;

pub use hal::{clamp_phi, PhiSensor, SensorConfig, SensorError, SensorResult};
pub use leo::LeoApiDriver;
pub use photonic::PhotonicChipDriver;
pub use svetoch::{SvetochChannel, SvetochDriver, SimChannel};

/// Seleciona o sensor a partir da configuração (I358).
pub async fn sensor_from_config(
    config: &SensorConfig,
) -> Result<Box<dyn PhiSensor>, SensorError> {
    if let Some(path) = &config.device_path {
        return Ok(Box::new(PhotonicChipDriver::new(path, 115_200)?));
    }
    if let Some(endpoint) = &config.api_endpoint {
        let token = config.bluetooth_address.clone().unwrap_or_default();
        return Ok(Box::new(LeoApiDriver::new(endpoint, &token)));
    }
    #[cfg(feature = "sim-svetoch")]
    {
        return Ok(Box::new(SvetochDriver::new(SimChannel::new(0.85))));
    }
    #[allow(unreachable_code)]
    {
        Err(SensorError::DeviceNotFound)
    }
}