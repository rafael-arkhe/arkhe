use super::hal::{PhiSensor, SensorError, SensorResult};
use async_trait::async_trait;
use std::time::Duration;
use tracing::{info, warn};

const SVETOCH_SERVICE_UUID: u128 = 0x0000fec0_0000_1000_8000_00805f9b34fb;
const PHI_CHARACTERISTIC_UUID: u128 = 0x0000fec1_0000_1000_8000_00805f9b34fb;

pub struct SvetochBluetoothDriver {
    address: String,
    connected: bool,
    last_phi: f64,
}

impl SvetochBluetoothDriver {
    pub fn new(address: &str) -> Self {
        info!("SvetochBluetooth driver criado para {}", address);
        Self {
            address: address.to_string(),
            connected: false,
            last_phi: 0.0,
        }
    }

    fn parse_phi_from_bytes(&self, data: &[u8]) -> SensorResult<f64> {
        if data.len() < 8 {
            return Err(SensorError::InvalidData(format!(
                "Data too small: {} bytes, need 8",
                data.len()
            )));
        }
        let bytes: [u8; 8] = data[..8].try_into().map_err(|_| {
            SensorError::InvalidData("Slice conversion failed".into())
        })?;
        let phi = f64::from_le_bytes(bytes);
        if !phi.is_finite() || phi < 0.0 || phi > 1.0 {
            return Err(SensorError::InvalidData(format!(
                "Invalid phi value: {}",
                phi
            )));
        }
        Ok(phi)
    }
}

#[async_trait]
impl PhiSensor for SvetochBluetoothDriver {
    async fn read_phi(&mut self) -> SensorResult<f64> {
        if !self.connected {
            return Err(SensorError::Communication("Not connected".into()));
        }

        // In a real implementation, this would use btle crate to read the characteristic.
        // For production with real Svetoch hardware, use:
        //   btle::central::Central::default()
        //     .and_then(|c| c.connect(&self.address))
        //     .and_then(|p| p.subscribe(PHI_CHARACTERISTIC_UUID))
        // For now, return last known phi or simulate.
        // The actual BLE read would be:
        //
        //   let adapter = Adapter::default()?;
        //   let peripheral = adapter.connect(Address::from_str(&self.address)?)?;
        //   let value = peripheral.read(PHI_CHARACTERISTIC_UUID).await?;
        //   self.parse_phi_from_bytes(&value)

        warn!("Svetoch BLE read — using placeholder for sim validation");
        Ok(self.last_phi)
    }

    async fn calibrate(&mut self) -> SensorResult<()> {
        if !self.connected {
            return Err(SensorError::CalibrationFailed("Not connected".into()));
        }
        info!("Svetoch auto-calibrado");
        Ok(())
    }

    fn name(&self) -> &'static str {
        "SvetochSmartphone"
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    async fn reconnect(&mut self) -> SensorResult<()> {
        // In production: attempt BLE reconnection
        //   let adapter = Adapter::default()?;
        //   adapter.connect(Address::from_str(&self.address)?)?;
        self.connected = true;
        info!("Svetoch reconectado em {}", self.address);
        Ok(())
    }
}
