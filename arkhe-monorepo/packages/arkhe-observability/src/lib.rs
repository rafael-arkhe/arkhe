//! ARKHE Observatory — the observability bridge between the Safe-Core
//! anti-hallucination policy and the Cathedral v26.5 Chladni Cloud dashboard.
//!
//! The bridge is *read-only*: it subscribes to the recurrency daemon's
//! `WatchTickets` stream, runs a local [`RecurrencyPolicy`] to derive the
//! constitutional metrics (vibe, clamp state, R², multiplicity), and publishes
//! them onto the Telegraph ξM-field bus so the dashboard's cards render live.
//! It never issues `SetRegime` — enforcement stays in `safe-core`.
//!
//! # Topics
//!
//! | Topic | Signals |
//! |-------|---------|
//! | `/cathedral/stats` | `cycle_stats`, `phi_confidence` |
//! | `/cathedral/events` | `security_event`, `identity_attestation`, `execution_attestation`, `multiplicity_attestation` |
//! | `/cathedral/control` | `<metric>_response` (replies to dashboard commands) |
//! | `/signal/phi` | `phiInterop` |

pub mod observatory;
pub mod signal;
pub mod telegraph;
pub mod tpm;

pub use observatory::Observatory;
pub use signal::Signal;
pub use telegraph::{ControlCommand, TelegraphClient};
pub use tpm::{TpmAnchor, load as load_tpm_anchor};
