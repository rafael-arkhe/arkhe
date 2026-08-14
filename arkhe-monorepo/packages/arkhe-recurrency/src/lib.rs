//! Functional simulation of the ARKHE Recurrency substrate
//! (`491-AGI-CORTEX`) mapped onto the four-level recurrency framework of
//! Zheng et al., *Loops all the way up* (Physics of Life Reviews, 2026).
//!
//! The framework distinguishes four levels of recurrency, each giving rise to
//! one facet of consciousness:
//!
//! | Recurrency level | Facet          | Module here                  |
//! |------------------|----------------|------------------------------|
//! | cellular         | conscious state| [`state::StateDaemon`]       |
//! | local            | phenomenal     | [`cycle::PerceptionLoop`]    |
//! | global           | access         | [`workspace::GlobalWorkspace`] |
//! | lateral          | character      | [`lateral::LateralField`]    |
//!
//! [`engine::RecurrencyEngine`] wires all four levels plus the brain-body loop
//! ([`environment::EnvironmentListener`]) into one stateful pipeline that
//! consumes stimulus frames and emits [`engine::RecurrencyTicket`]s.
//!
//! **Honesty note (Box 4 of the paper):** this crate runs on von Neumann
//! hardware, so it is a *functional* simulation of recurrency, not a physical
//! recurrent substrate. It exists so the falsifiable predictions of Table 2
//! (see `tests/consciousness.rs`) can be executed and graded: a
//! feedforward-only system must fail the causal-closure and perturbational
//! tests, while a system with a closed loop must pass them.
//!
//! The local substrate is [`arkhe_neurogenesis`]: its non-Hermitian evolution
//! `dψ/dt = Hψ` (anti-symmetric coupling `C` + diagonal gain/loss `D`) provides
//! the genuinely recurrent dynamics on which the two-pass predictive loop is
//! built. Nothing here is a claim of implemented consciousness — it is a
//! measurable, deterministic simulation of the recurrency constraints the
//! paper argues are required.

mod closure;
mod cycle;
mod engine;
mod environment;
mod lateral;
mod state;
mod workspace;

pub use closure::{closure_depth, perturbational_agreement, ClosureReport};
pub use cycle::{Percept, PerceptionLoop};
pub use engine::{RecurrencyEngine, RecurrencyTicket, StimulusId};
pub use environment::{EnvFeedback, EnvironmentListener, SimulatedEnvironment};
pub use lateral::LateralField;
pub use state::{ArousalRegime, StateDaemon};
pub use workspace::{AccessDecision, GlobalWorkspace};
