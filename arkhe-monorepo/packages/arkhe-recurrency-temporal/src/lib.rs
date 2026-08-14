//! Cyber-physical safety layer for `491-AGI-CORTEX` (v0.4.3).
//!
//! Composes the pure, deterministic [`arkhe_recurrency`] core with an async
//! operational layer:
//!
//! * [`daemon::TemporalStateDaemon`] — an async arousal daemon with a temporal
//!   EWMA integrity history and a modulation event stream (`record_cycle`);
//! * [`safety::iguard`] — the [`safety::iguard::MissionSafetyKernel`], which
//!   records a failed perception cycle *before* forcing `Coma` (the v0.4.3
//!   regression fix: without `record_cycle`, the temporal EWMA stays
//!   inconsistent with reality);
//! * [`network::smartad`] and [`spatial::csi`] — network and RF-plane
//!   detectors that feed the anomaly-aware engine gate;
//! * [`engine::AnomalyAwareRecurrencyEngine`] — gates `tick_correct` behind
//!   the arousal regime, the network anomaly detector and the lateral
//!   geometry integrity.
//!
//! The three-phase cyber-physical kill chain (`tests/kill_chain.rs`) is the
//! God Test: it drives a full stack from a network intrusion, through an RF
//! perturbation, to an iGuard-forced `Coma`, and verifies the temporal
//! history records the failure and the engine rejects tickets in `Coma`.

pub mod daemon;
pub mod engine;
pub mod error;
pub mod geometry;
pub mod network;
pub mod perception;
pub mod safety;
pub mod spatial;

pub use arkhe_recurrency::{ArousalRegime, StimulusId};
pub use daemon::{TemporalEvent, TemporalIntegrity, TemporalRecord, TemporalStateDaemon};
pub use engine::{
    AnomalyAwareRecurrencyEngine, PhiCalculator, SMeasureCalculator, TemporalRecurrencyEngine,
    TemporalTicket,
};
pub use error::RecurrencyError;
pub use geometry::LateralGeometry;
pub use network::smartad::{
    HuaweiSmartADDetector, NetworkAnomalyDetector, SmartADEvent,
};
pub use perception::{ActionId, Perception};
pub use safety::iguard::{IguardAlert, IguardResponse, IguardWebhookListener, MissionSafetyKernel};
pub use spatial::csi::{CsiLateralField, CsiMatrix};
