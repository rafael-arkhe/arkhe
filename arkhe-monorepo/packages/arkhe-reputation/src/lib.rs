//! ARKHE REPUTATION — reputação de operadores TOON (#4),
//! matriz de governança por esperança estatística (#15) e certifier automático (#17).

#![deny(unsafe_code)]

pub mod certifier;
pub mod governance;
pub mod reputation;

pub use certifier::{Certificate, Certifier, VerificationStatus};
pub use governance::{GovernanceMatrix, Proposal, ReputationMatrix, Voter};
pub use reputation::{ReputationScore, TOON_DECAY_DAYS, evaluate_node_reputation};