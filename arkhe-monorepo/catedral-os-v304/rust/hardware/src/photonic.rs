// Catedral OS v304.0 — Driver do chip fotônico 3D via UART/serial.
//
// Correções de validação vs. rascunho original:
//   1. leitura com `read` preenchendo o frame completo (loop com timeout);
//   2. `SerialPortBuilder` removido (import não usado);
//   3. clamp aplicado via hal::clamp_phi (Gap-1).

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use serialport::SerialPort;
use tracing::{debug, info, warn};

use crate::hal::{clamp_phi, PhiSensor, SensorError, SensorResult};

pub struct PhotonicChipDriver {
    port: Mutex<Box<dyn SerialPort>>,
    calibration: f64,
}

impl PhotonicChipDriver {
    pub fn new(device_path: &str, baud_rate: u32) -> Result<Self, SensorError> {
        let port = serialport::new(device_path, baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| SensorError::Communication(e.to_string()))?;
        info!("chip fotônico conectado em {} ({} baud)", device_path, baud_rate);
        Ok(Self {
            port: Mutex::new(port),
            calibration: 1.0,
        })
    }

    fn port<'a>(&'a mut self) -> SensorResult<&'a mut Box<dyn SerialPort>> {
        self.port
            .get_mut()
            .map_err(|_| SensorError::Communication("mutex serial empoisonado".into()))
    }

    fn read_frame(&mut self, n: usize) -> SensorResult<Vec<u8>> {
        let port = self.port()?;
        let mut buf = vec![0u8; n];
        let mut filled = 0usize;
        while filled < n {
            match port.read(&mut buf[filled..]) {
                Ok(0) => {
                    return Err(SensorError::Communication(
                        "leitura serial vazia (dispositivo em silêncio)".into(),
                    ))
                }
                Ok(len) => filled += len,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::TimedOut && filled == 0 {
                        return Err(SensorError::Timeout(format!("sem frame em {}ms", 100)));
                    }
                    return Err(SensorError::Communication(e.to_string()));
                }
            }
        }
        Ok(buf)
    }

    fn decode_interferometry(&mut self, buffer: &[u8]) -> SensorResult<f64> {
        if buffer.len() < 8 {
            return Err(SensorError::InvalidData(format!(
                "tamanho de frame insuficiente: {} bytes",
                buffer.len()
            )));
        }
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&buffer[..8]);
        let phi = clamp_phi(f64::from_le_bytes(raw) * self.calibration);
        debug!(phi, "leitura bruta decodificada");
        Ok(phi)
    }
}

#[async_trait]
impl PhiSensor for PhotonicChipDriver {
    async fn read_phi(&mut self) -> SensorResult<f64> {
        let frame = self.read_frame(64)?;
        self.decode_interferometry(&frame)
    }

    async fn calibrate(&mut self) -> SensorResult<()> {
        let port = self.port()?;
        port.write(b"CAL\n")
            .map_err(|e| SensorError::Communication(e.to_string()))?;
        let frame = self.read_frame(8)?;
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&frame[..8]);
        let ref_phi = f64::from_le_bytes(raw);
        if !(0.0 < ref_phi) || ref_phi > 1.0 {
            return Err(SensorError::CalibrationFailed(format!(
                "valor de referência inválido: {ref_phi}"
            )));
        }
        self.calibration = 1.0 / ref_phi;
        info!(calibration = self.calibration, "chip fotônico calibrado");
        warn!("calibração em hardware: execute a bateria de referência Lean (I358) antes de liberar");
        Ok(())
    }

    fn name(&self) -> &'static str {
        "PhotonicChip3D"
    }
}