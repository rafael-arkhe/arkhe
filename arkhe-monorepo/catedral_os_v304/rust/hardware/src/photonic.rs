use super::hal::{PhiSensor, SensorError, SensorResult};
use async_trait::async_trait;
use serialport::SerialPort;
use std::time::Duration;
use tracing::{info, warn};

pub struct PhotonicChipDriver {
    port: Box<dyn SerialPort>,
    calibration: f64,
    connected: bool,
}

impl PhotonicChipDriver {
    pub fn new(device_path: &str, baud_rate: u32) -> Result<Self, SensorError> {
        let port = serialport::new(device_path, baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| SensorError::Communication(e.to_string()))?;
        info!("PhotonicChip aberto em {} @ {} baud", device_path, baud_rate);
        Ok(Self {
            port,
            calibration: 1.0,
            connected: true,
        })
    }

    fn decode_interferometry(&self, buffer: &[u8]) -> SensorResult<f64> {
        if buffer.len() < 8 {
            return Err(SensorError::InvalidData(
                "Buffer too small for f64".into(),
            ));
        }
        let bytes: [u8; 8] = buffer[..8].try_into().map_err(|_| {
            SensorError::InvalidData("Failed to convert slice to array".into())
        })?;
        let phi = f64::from_le_bytes(bytes);
        if !phi.is_finite() || phi < 0.0 || phi > 1.0 {
            return Err(SensorError::InvalidData(format!(
                "Phi out of range: {}",
                phi
            )));
        }
        Ok(phi * self.calibration)
    }

    fn send_command(&mut self, cmd: &[u8]) -> SensorResult<()> {
        self.port
            .write_all(cmd)
            .map_err(|e| SensorError::Communication(e.to_string()))?;
        self.port
            .flush()
            .map_err(|e| SensorError::Communication(e.to_string()))?;
        Ok(())
    }

    fn read_response(&mut self, buffer: &mut [u8]) -> SensorResult<usize> {
        self.port
            .read(buffer)
            .map_err(|e| SensorError::Communication(e.to_string()))
    }
}

#[async_trait]
impl PhiSensor for PhotonicChipDriver {
    async fn read_phi(&mut self) -> SensorResult<f64> {
        self.send_command(b"READ\n")?;
        let mut buffer = [0u8; 64];
        let n = self.read_response(&mut buffer)?;
        if n < 8 {
            return Err(SensorError::InvalidData(format!(
                "Read only {} bytes, expected >= 8",
                n
            )));
        }
        self.decode_interferometry(&buffer[..n])
    }

    async fn calibrate(&mut self) -> SensorResult<()> {
        self.send_command(b"CAL\n")?;
        let mut buffer = [0u8; 8];
        let n = self.read_response(&mut buffer)?;
        if n < 8 {
            return Err(SensorError::CalibrationFailed(
                "Insufficient calibration data".into(),
            ));
        }
        let ref_phi = f64::from_le_bytes(buffer);
        if ref_phi <= 0.0 || ref_phi > 1.0 || !ref_phi.is_finite() {
            return Err(SensorError::CalibrationFailed(format!(
                "Invalid reference phi: {}",
                ref_phi
            )));
        }
        self.calibration = 1.0 / ref_phi;
        info!(
            "PhotonicChip calibrado — factor: {:.6}",
            self.calibration
        );
        Ok(())
    }

    fn name(&self) -> &'static str {
        "PhotonicChip3D"
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    async fn reconnect(&mut self) -> SensorResult<()> {
        let port_name = self.port.name().unwrap_or_default();
        let baud = self.port.baud_rate().unwrap_or(115200);
        match serialport::new(&port_name, baud)
            .timeout(Duration::from_millis(100))
            .open()
        {
            Ok(port) => {
                self.port = port;
                self.connected = true;
                info!("PhotonicChip reconectado em {}", port_name);
                Ok(())
            }
            Err(e) => {
                self.connected = false;
                Err(SensorError::Communication(e.to_string()))
            }
        }
    }
}
