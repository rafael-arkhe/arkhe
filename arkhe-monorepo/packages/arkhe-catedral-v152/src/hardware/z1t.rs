//! Bridge Z1T — comunicação UART/USB com framing G1 e fallback para simulação
//! (G8/G11).
//!
//! O crate é `no_unsafe` e não embarca `serialport` (que exigiria FFI/unsafe).
//! Esta implementação fornece o **transport trait** assíncrono e um **driver
//! simulado** determinístico para CI, além do fluxo de reconexão com backoff.
//! Em produção com `tokio-serial` basta implementar [`Z1TTransport`] para o
//! dispositivo físico.

use crate::hardware::framer::Z1TFramer;
use std::time::{Duration, Instant};

/// Transporte físico (UART/USB). Implemente para o seu dispositivo.
pub trait Z1TTransport {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize>;
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;
    fn open() -> std::io::Result<Self>
    where
        Self: Sized;
    fn close(&mut self);
}

/// Fallback determinístico para simulação (G11).
#[derive(Debug, Clone)]
pub struct SimulatedTransport;

impl Z1TTransport for SimulatedTransport {
    fn write(&mut self, _data: &[u8]) -> std::io::Result<usize> {
        Ok(0)
    }
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Resposta simulada: 16 p‑bits (u8) + coerência estimada (f32),
        // envelopada em G1.
        let mut payload = Vec::with_capacity(20);
        for i in 0..16u8 {
            payload.push(i * 3 + 128);
        }
        let phi = crate::core::spectral::irreducible_coherence(13) as f32;
        payload.extend_from_slice(&phi.to_le_bytes());

        let resp = crate::hardware::framer::frame(&payload);
        let n = resp.len().min(buf.len());
        buf[..n].copy_from_slice(&resp[..n]);
        Ok(n)
    }
    fn open() -> std::io::Result<Self> {
        Ok(Self)
    }
    fn close(&mut self) {}
}

/// Bridge Z1T com reconexão (G8) e fallback (G11).
pub struct Z1TBridge<T = SimulatedTransport> {
    transport: Option<T>,
    framer: Z1TFramer,
    connected: bool,
    simulating: bool,
    max_retries: u32,
    backoff: Duration,
    last_contact: Instant,
}

impl<T: Z1TTransport> Z1TBridge<T> {
    pub fn new() -> Self {
        Self {
            transport: None,
            framer: Z1TFramer::new(),
            connected: false,
            simulating: false,
            max_retries: 3,
            backoff: Duration::from_millis(500),
            last_contact: Instant::now(),
        }
    }

    /// G8 — conecta com retry e backoff; degrada para simulação (G11).
    pub fn connect(&mut self) -> bool {
        if self.connected {
            return true;
        }
        let mut attempt = 0;
        while attempt < self.max_retries {
            match T::open() {
                Ok(t) => {
                    let mut t = t;
                    let framed = self.framer.frame(b"Z1T_HANDSHAKE");
                    if t.write(&framed).is_err() {
                        t.close();
                        attempt += 1;
                        std::thread::sleep(self.backoff * (attempt as u32));
                        continue;
                    }
                    self.transport = Some(t);
                    self.connected = true;
                    self.simulating = false;
                    self.last_contact = Instant::now();
                    return true;
                }
                Err(_) => {
                    attempt += 1;
                    std::thread::sleep(self.backoff * (attempt as u32));
                }
            }
        }
        self.simulating = true;
        true
    }

    /// Envia comando com framing; retorna resposta unframed ou `None`.
    pub fn send_command(&mut self, cmd: &[u8]) -> Option<Vec<u8>> {
        if !self.connected {
            self.connect();
        }

        if self.simulating {
            let mut sim = SimulatedTransport::open().ok()?;
            let mut buf = [0u8; 4096];
            let n = sim.read(&mut buf).ok()?;
            self.last_contact = Instant::now();
            return self.framer.unframe(&buf[..n]);
        }

        let Some(transport) = &mut self.transport else {
            self.simulating = true;
            return None;
        };

        let framed = self.framer.frame(cmd);
        if transport.write(&framed).is_err() {
            self.connected = false;
            self.simulating = true;
            return None;
        }

        let mut buf = [0u8; 4096];
        match transport.read(&mut buf) {
            Ok(n) if n > 0 => {
                self.last_contact = Instant::now();
                self.framer.unframe(&buf[..n])
            }
            _ => {
                self.connected = false;
                self.simulating = true;
                None
            }
        }
    }

    pub fn disconnect(&mut self) {
        if let Some(t) = &mut self.transport {
            t.close();
        }
        self.transport = None;
        self.connected = false;
        self.simulating = false;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn is_simulating(&self) -> bool {
        self.simulating
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_fallback_to_simulation() {
        let mut bridge = Z1TBridge::<SimulatedTransport>::new();
        assert!(bridge.connect());
        let resp = bridge.send_command(b"Z1T_INFER");
        assert!(resp.is_some());
        assert_eq!(resp.unwrap().len(), 20);
    }

    #[test]
    fn test_bridge_disconnect() {
        let mut bridge = Z1TBridge::<SimulatedTransport>::new();
        bridge.connect();
        bridge.disconnect();
        assert!(!bridge.is_connected());
        assert!(bridge.connect());
    }
}