#![deny(unsafe_code)]

//! ARKHE Actuators — evidence-driven biological effectors.
//!
//! The first actuator is the RF cell modulator (US10064941B2): an external RF
//! command (Z1) drives a continuous cellular response (Z2, [Ca²⁺]ᵢ / gene
//! expression) that is bounded by the I18 invariant and certified as discrete
//! evidence (Z3) against the repository standard `arkhe-buzz-bridge` types.
//!
//! Added 2026-08-18 (DPDM integration, arXiv:2510.13956): the aerogel plasma
//! actuator and the Zakharov saturation model implement the non-linear
//! plasma-saturation benchmark that calibrates the retrocausal RF/ELF
//! pipeline — saturation is treated as the observable, not a failure mode.

pub mod plasma_actuator;
pub mod rf_cell_modulator;
pub mod zakharov_model;

pub use plasma_actuator::{AerogelPlasmaActuator, AerogelState, PlasmaState};
pub use rf_cell_modulator::{
    ActuatorError, I18Audit, I18CalciumGuard, I18Step, I18Verdict, NanoparticleType,
    RfCellModulator, RfStimulationConfig, StimulationResult, TargetGene,
    evidence_stays_in_firewall,
};
pub use zakharov_model::ZakharovSaturation;

pub const VERSION: &str = "0.1.0-ARKHE-ACTUATORS-2026-08-02";