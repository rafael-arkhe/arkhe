//! Dimensionally-consistent retrocausal echo channel.
//!
//! An echo emitted at phase-time `t` travels across the phase field with finite
//! speed `v` and arrives — into the past-reaching read path — after a travel
//! **time** (`t_latency = distance / v_eco`). The bug in the original code was
//! comparing `travel_time <= max_travel` where `max_travel = self.v_eco * 1.0`
//! (i.e. comparing a *time* to a *velocity*). Here we place echoes on a FIFO
//! with an explicit `arrival_at` timestamp computed from `distance / v_eco`, so
//! `receive` simply pops any echo whose `arrival_at <= now`.

use std::collections::VecDeque;

use ndarray::Array2;

use crate::error::MhdError;

/// A single echo queued for propagation.
#[derive(Debug, Clone)]
pub struct RetroSignal {
    /// Phase-time the echo was emitted.
    pub emitted_at: f64,
    /// Phase-time at which the echo is due (arrival): `distance / v_eco`.
    pub arrival_at: f64,
    /// The echo pattern (a field-shaped matrix) being returned.
    pub pattern: Array2<f64>,
}

/// Circulation that returns dropped harmonic tail-modes to the current state.
///
/// The total latency is set from a fixed logical field diameter `field_diameter`
/// (e.g. `max(nx, ny) * h`), so one physical `distance` fixes the units and the
/// `emit`/`receive` pair is dimensionally consistent.
#[derive(Debug, Clone)]
pub struct RetroChannel {
    /// Echo propagation speed in grid-length / phase-time.
    pub v_eco: f64,
    /// Effective field diameter (grid-length units) an echo must cross.
    pub field_diameter: f64,
    /// FIFO of not-yet-arrived echoes.
    pending: VecDeque<RetroSignal>,
}

impl RetroChannel {
    /// Create a channel crossing `field_diameter` rows of the field.
    pub fn new(v_eco: f64, field_diameter: f64) -> Self {
        Self {
            v_eco,
            field_diameter,
            pending: VecDeque::new(),
        }
    }

    /// One-way travel time across the whole field.
    pub fn parallel_latency(&self) -> f64 {
        if self.v_eco <= 0.0 {
            f64::INFINITY
        } else {
            self.field_diameter / self.v_eco
        }
    }

    /// Inject an echo now (`at phase-time `now`). It stays latent until
    /// `now + parallel_latency()`.
    pub fn emit(&mut self, now: f64, pattern: Array2<f64>) {
        let arrival_at = now + self.parallel_latency();
        self.pending.push_back(RetroSignal {
            emitted_at: now,
            arrival_at,
            pattern,
        });
    }

    /// Return the next echo whose `arrival_at <= now`, or [`MhdError::NoEchoDue`].
    pub fn receive(&mut self, now: f64) -> Result<RetroSignal, MhdError> {
        let due = self
            .pending
            .iter()
            .position(|s| s.arrival_at <= now)
            .ok_or(MhdError::NoEchoDue { t: now })?;
        // Pop the oldest due echo (FIFO within due-window).
        Ok(self.pending.remove(due).expect("position is valid"))
    }

    /// Number of echoes still in flight.
    pub fn in_flight(&self) -> usize {
        self.pending.len()
    }

    /// Whether any echo has landed by `now`.
    pub fn has_landed(&self, now: f64) -> bool {
        self.pending.iter().any(|s| s.arrival_at <= now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_arrives_after_one_diameter() {
        let mut ch = RetroChannel::new(2.0, 10.0); // latency = 5 time units
        ch.emit(0.0, Array2::zeros((3, 3)));
        assert!(!ch.has_landed(3.0));
        assert!(!ch.has_landed(4.99));
        assert!(ch.has_landed(5.0), "latency should be exactly diameter/v");
        let sig = ch.receive(5.0).expect("due echo");
        assert_eq!(sig.emitted_at, 0.0);
        assert_eq!(sig.arrival_at, 5.0);
    }

    #[test]
    fn retro_reads_before_arrival_is_an_error() {
        let mut ch = RetroChannel::new(1.0, 100.0); // latency 100
        ch.emit(0.0, Array2::zeros((1, 1)));
        let r = ch.receive(1.0);
        assert!(matches!(r, Err(MhdError::NoEchoDue { t }) if t == 1.0));
    }

    #[test]
    fn retro_emission_records_timestamp() {
        let mut ch = RetroChannel::new(3.0, 9.0); // latency 3
        ch.emit(2.0, Array2::zeros((2, 2)));
        let due = ch.parallel_latency();
        assert!((due - 3.0).abs() < 1e-12);
        let sig = ch.receive(5.0).expect("due");
        assert_eq!(sig.emitted_at, 2.0);
        assert_eq!(sig.arrival_at, 5.0);
    }
}