//! FI-031 — acyclic execution plans. See [`validator`] for the full
//! explanation, including a real compile bug fixed relative to an earlier
//! draft.

#![deny(unsafe_code)]

pub mod plan_kind;
pub mod validator;

pub use plan_kind::PlanKind;
pub use validator::{Action, ActionId, Plan, PlanError, PlanValidator};
