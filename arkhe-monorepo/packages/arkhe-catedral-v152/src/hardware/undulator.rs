//! Nó Undulator — decaimento físico com Zeno Veto (I194–I197) e adaptativo
//! (I198).

use crate::core::spectral::irreducible_coherence;
use serde::{Deserialize, Serialize};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// I195 — Ranging Delta (microssegundos de linha).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangingDelta(pub u64);

impl RangingDelta {
    pub fn from_micros(us: u64) -> Self {
        Self(us)
    }

    pub fn as_micros(&self) -> u64 {
        self.0
    }

    pub fn as_secs_f64(&self) -> f64 {
        self.0 as f64 * 1e-6
    }

    /// I194 — Limiar de Zeno = 2 × ranging.
    pub fn zeno_threshold(&self) -> u64 {
        self.0 * 2
    }
}

/// I196/I198 — Taxa de decaimento.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DecayRate(pub f64);

impl DecayRate {
    pub fn from_ranging(ranging: RangingDelta) -> Self {
        let tau = ranging.as_secs_f64();
        if tau <= 0.0 {
            Self(1.0)
        } else {
            Self(1.0 / tau)
        }
    }

    /// Decaimento exponencial: Φ(t) = Φ₀·exp(−λ·t).
    pub fn apply(&self, phi: f64, dt_secs: f64) -> f64 {
        phi * (-self.0 * dt_secs).exp()
    }

    pub fn value(&self) -> f64 {
        self.0
    }
}

/// Um handover aceito pelo Undulator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UndulatorHandover {
    pub phi: f64,
    pub source: String,
    pub timestamp: u64,
    pub ranging_delta_us: u64,
    pub zeno_passed: bool,
    pub decayed_phi: f64,
}

/// Estatísticas agregadas do nó Undulator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UndulatorStats {
    pub handover_count: u64,
    pub total_phi_received: f64,
    pub total_phi_accepted: f64,
    pub acceptance_rate: f64,
    pub ranging_us: u64,
    pub decay_rate: f64,
}

/// Nó Undulator com Zeno Veto (I194) e decaimento adaptativo (I198).
pub struct UndulatorNode {
    pub ranging_delta: RangingDelta,
    pub decay_rate: DecayRate,
    last_handover: Instant,
    handover_count: u64,
    total_phi_received: f64,
    total_phi_accepted: f64,
    history: Vec<UndulatorHandover>,
    /// I198 — parâmetros de adaptação.
    adaptive: AdaptiveParams,
}

/// I198 — λ(t) = λ₀ + β·(1 − f), com clamp em [min, max].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AdaptiveParams {
    pub initial_decay: f64,
    pub adaptation_rate: f64,
    pub min_decay: f64,
    pub max_decay: f64,
}

impl Default for AdaptiveParams {
    fn default() -> Self {
        Self {
            initial_decay: 0.001,
            adaptation_rate: 0.0005,
            min_decay: 0.0001,
            max_decay: 0.01,
        }
    }
}

impl UndulatorNode {
    pub fn new(ranging_delta: RangingDelta) -> Self {
        let decay_rate = DecayRate::from_ranging(ranging_delta);
        Self {
            ranging_delta,
            decay_rate,
            last_handover: Instant::now(),
            handover_count: 0,
            total_phi_received: 0.0,
            total_phi_accepted: 0.0,
            history: Vec::new(),
            adaptive: AdaptiveParams::default(),
        }
    }

    pub fn with_adaptive(mut self, ap: AdaptiveParams) -> Self {
        self.adaptive = ap;
        self.decay_rate = DecayRate(ap.initial_decay);
        self
    }

    /// I194 — Recebe handover com verificação do limiar de Zeno.
    pub fn receive_handover(&mut self, phi: f64, source: &str) -> Option<UndulatorHandover> {
        let now = Instant::now();
        let dt = now - self.last_handover;
        let dt_us = dt.as_micros() as u64;
        let dt_secs = dt.as_secs_f64();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.handover_count += 1;
        self.total_phi_received += phi;

        let zeno_threshold = self.ranging_delta.zeno_threshold();
        let zeno_passed = dt_us <= zeno_threshold;

        if !zeno_passed {
            return None;
        }

        // I198 — taxa adaptativa baseada na frequência de handovers.
        if self.adaptive.adaptation_rate > 0.0 {
            self.decay_rate = DecayRate(self.adaptive_decay(now));
        }

        let decayed_phi = self.decay_rate.apply(phi, dt_secs);

        let handover = UndulatorHandover {
            phi,
            source: source.to_string(),
            timestamp,
            ranging_delta_us: self.ranging_delta.as_micros(),
            zeno_passed: true,
            decayed_phi,
        };

        self.history.push(handover.clone());
        self.last_handover = now;
        self.total_phi_accepted += decayed_phi;

        Some(handover)
    }

    /// I198 — λ(t) = λ₀ + β·(1 − f_handover), clampada.
    fn adaptive_decay(&self, now: Instant) -> f64 {
        let ap = self.adaptive;
        let freq = self.frequency(now);
        let raw = ap.initial_decay + ap.adaptation_rate * (1.0 - freq.min(1.0));
        raw.clamp(ap.min_decay, ap.max_decay)
    }

    /// Frequência estimada de handovers por segundo (janela fixa de 10s).
    fn frequency(&self, now: Instant) -> f64 {
        let window = std::time::Duration::from_secs_f64(10.0);
        let cutoff = now - window;
        // A janela de timestamps reais exige armazenamento; aproximação
        // determinística: todos os handovers aceitos caem na janela se o
        // nó recebeu `count` desde o início.
        let _ = cutoff;
        self.history.len() as f64 / 10.0
    }

    pub fn stats(&self) -> UndulatorStats {
        UndulatorStats {
            handover_count: self.handover_count,
            total_phi_received: self.total_phi_received,
            total_phi_accepted: self.total_phi_accepted,
            acceptance_rate: if self.total_phi_received > 0.0 {
                self.total_phi_accepted / self.total_phi_received
            } else {
                0.0
            },
            ranging_us: self.ranging_delta.as_micros(),
            decay_rate: self.decay_rate.value(),
        }
    }

    pub fn accepted_phi_reference(&self) -> f64 {
        // I157 aplicada ao nó: referência quasi-universal.
        irreducible_coherence(13)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zeno_bound() {
        let ranging = RangingDelta(1000);
        let mut undulator = UndulatorNode::new(ranging);

        std::thread::sleep(std::time::Duration::from_micros(500));
        let result = undulator.receive_handover(0.95, "test");
        assert!(result.is_some());

        std::thread::sleep(std::time::Duration::from_micros(2500));
        let result = undulator.receive_handover(0.95, "test");
        assert!(result.is_none());
    }

    #[test]
    fn test_decay_application() {
        let ranging = RangingDelta(1000);
        let decay = DecayRate::from_ranging(ranging);
        let result = decay.apply(1.0, 0.001);
        assert!((result - 0.367).abs() < 0.01);
    }

    #[test]
    fn test_zeno_threshold() {
        assert_eq!(RangingDelta(1000).zeno_threshold(), 2000);
    }

    #[test]
    fn test_adaptive_decay_bounded() {
        let ranging = RangingDelta(1000);
        let mut u = UndulatorNode::new(ranging).with_adaptive(AdaptiveParams::default());
        let Some(h) = u.receive_handover(0.9, "src") else {
            panic!("should accept when fresh");
        };
        assert!(h.decayed_phi > 0.0);
        assert!(u.stats().decay_rate < u.adaptive.max_decay + 1e-12);
        assert!(u.stats().decay_rate > u.adaptive.min_decay - 1e-12);
    }

    #[test]
    fn test_stats() {
        let ranging = RangingDelta(1000);
        let mut u = UndulatorNode::new(ranging);
        std::thread::sleep(std::time::Duration::from_micros(200));
        u.receive_handover(0.8, "a");
        let s = u.stats();
        assert_eq!(s.handover_count, 1);
        assert!(s.acceptance_rate <= 1.0 + 1e-9);
    }
}