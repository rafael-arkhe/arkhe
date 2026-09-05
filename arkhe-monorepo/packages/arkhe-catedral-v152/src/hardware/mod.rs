//! Hardware da Catedral OS v152.0 — Undulator (I194–I198), framer Z1T (G1) e
//! bridge Z1T (G8/G11).

pub mod framer;
pub mod undulator;
pub mod z1t;

pub use undulator::{DecayRate, RangingDelta, UndulatorHandover, UndulatorNode, UndulatorStats};