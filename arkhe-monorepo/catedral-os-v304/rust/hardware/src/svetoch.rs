// Catedral OS v304.0 — Driver de smartphone Svetoch (canal BLE ou simulado).
//
// Correção de validação: o rascunho original instanciava `Adapter::default()`
// e `Address::from_str` com assinaturas não verificáveis do crate `btle`.
// A API exata do `btle` não é estável entre plataformas/versões, então o
// transporte é isolado atrás de `SvetochChannel`:
//   - default: canal simulado (testável, sem hardware);
//   - feature `svetoch-ble`: driver real sobre btle (requer on-board/CI com
//     adaptador BLE para verificação final — checklist em ProductionHardware.md).

use async_trait::async_trait;
use tracing::{debug, info};

use crate::hal::{clamp_phi, PhiSensor, SensorError, SensorResult};

// ============================================================================ #
// Canal: contrato mínimo de transporte
// ============================================================================ #

pub trait SvetochChannel: Send {
    fn read_phi_bytes(&mut self) -> SensorResult<Vec<u8>>;
    fn is_connected(&self) -> bool;
}

// ============================================================================ #
// Driver
// ============================================================================ #

pub struct SvetochDriver<C: SvetochChannel> {
    channel: C,
}

impl<C: SvetochChannel> SvetochDriver<C> {
    pub fn new(channel: C) -> Self {
        Self { channel }
    }
}

#[async_trait]
impl<C: SvetochChannel + Send + Sync + 'static> PhiSensor for SvetochDriver<C> {
    async fn read_phi(&mut self) -> SensorResult<f64> {
        if !self.channel.is_connected() {
            return Err(SensorError::Communication("desconectado".into()));
        }
        let bytes = self.channel.read_phi_bytes()?;
        if bytes.len() < 8 {
            return Err(SensorError::InvalidData(format!(
                "frame BLE curto: {} bytes",
                bytes.len()
            )));
        }
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&bytes[..8]);
        let phi = clamp_phi(f64::from_le_bytes(raw));
        debug!(phi, "leitura Svetoch decodificada");
        Ok(phi)
    }

    async fn calibrate(&mut self) -> SensorResult<()> {
        if !self.channel.is_connected() {
            return Err(SensorError::CalibrationFailed("desconectado".into()));
        }
        info!("Svetoch: verificação auto-calibrante OK (canal conectado)");
        Ok(())
    }

    fn name(&self) -> &'static str {
        "SvetochSmartphone"
    }
}

// ============================================================================ #
// Canal simulado (feature default: sim-svetoch)
// ============================================================================ #

#[cfg(feature = "sim-svetoch")]
pub struct SimChannel {
    connected: bool,
    next_phi: f64,
}

#[cfg(feature = "sim-svetoch")]
impl SimChannel {
    pub fn new(next_phi: f64) -> Self {
        Self {
            connected: true,
            next_phi: next_phi.clamp(0.577350, 0.999900),
        }
    }
}

#[cfg(feature = "sim-svetoch")]
impl SvetochChannel for SimChannel {
    fn read_phi_bytes(&mut self) -> SensorResult<Vec<u8>> {
        Ok(self.next_phi.to_le_bytes().to_vec())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

// ============================================================================ //
// Canal BLE real (feature: svetoch-ble) — REQUER verificação em hardware.
// ============================================================================//

#[cfg(feature = "svetoch-ble")]
pub struct BleChannel {
    address: btle::Address,
    connected: bool,
}

#[cfg(feature = "svetoch-ble")]
impl BleChannel {
    pub fn new(address_hex: &str) -> Result<Self, SensorError> {
        let address: btle::Address = address_hex
            .parse()
            .map_err(|e| SensorError::InvalidData(format!("endereço BLE: {e:?}")))?;
        Ok(Self {
            address,
            connected: false,
        })
    }
}

#[cfg(feature = "svetoch-ble")]
impl SvetochChannel for BleChannel {
    fn read_phi_bytes(&mut self) -> SensorResult<Vec<u8>> {
        // NOTA DE VALIDAÇÃO: a API do crate `btle` (Adapter::default(),
        // Adapter::connect, Peripheral::characteristic, Characteristic::read)
        // deve ser confirmada contra a versão fixada e o firmware do Svetoch
        // durante a campanha de validação física (I358). Este canal é o ponto
        // único de acoplamento ao crate.
        Err(SensorError::Communication(
            "transportador BLE ainda não onboarded: verificar API btle em hardware".into(),
        ))
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sim_channel_reads_clamped_phi() {
        let mut driver = SvetochDriver::new(SimChannel::new(0.95));
        let phi = driver.read_phi().await.unwrap();
        assert!(phi > 0.5 && phi <= 0.999900);
    }
}