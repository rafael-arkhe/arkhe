// crates/arkhe-governance/src/capability.rs
//! Capability tokens: what a subject may do, to what, and until when.

use serde::{Deserialize, Serialize};

/// A token granting a subject a set of actions over a set of resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    /// Identifier of the token.
    pub token_id: String,
    /// Subject the token was issued to.
    pub subject: String,
    /// Actions the token permits.
    pub actions: Vec<String>,
    /// Resources the token permits those actions on.
    pub resources: Vec<String>,
    /// Instant after which the token is no longer valid, if any.
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl CapabilityToken {
    /// Returns `true` when the token permits `action` on `resource`.
    ///
    /// Both the action and the resource must be listed. Expiry is not consulted
    /// here; the caller is responsible for checking [`Self::expires_at`].
    pub fn can(&self, action: &str, resource: &str) -> bool {
        self.actions.iter().any(|a| a == action)
            && self.resources.iter().any(|r| r == resource)
    }
}
