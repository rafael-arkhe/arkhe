// crates/arkhe-governance/src/gdpr.rs
//! GDPR workflows: the right to erasure, data portability, and residency
//! policies.
//!
//! NOTE (first compilation): this module used to `use arkhe_identity::ArkheDid;`.
//! No such type exists anywhere in the workspace — `arkhe-identity` exports the
//! GDID types only, and the non-member crate `safe-core-crypto` names its DID
//! type `DidArkhe`. Subjects are therefore carried as their textual DID
//! representation, which is the minimum change that makes the module compile
//! without editing another crate.

use arkhe_core::ArkheHash;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A data subject's request to have their data erased.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightToErasure {
    /// DID of the data subject.
    pub subject: String,
    /// Identifier of the request.
    pub request_id: String,
    /// When the request was received.
    pub requested_at: DateTime<Utc>,
    /// Where the request stands.
    pub status: ErasureStatus,
    /// Hashes of the entries that were tombstoned rather than removed.
    pub tombstoned_entries: Vec<ArkheHash>,
    /// Why some data is being retained, when it is.
    pub retention_reason: Option<String>,
}

/// Lifecycle of an erasure request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErasureStatus {
    /// Received, not yet started.
    Pending,
    /// Erasure is under way.
    InProgress,
    /// Erasure finished.
    Completed,
    /// The request was refused.
    Rejected,
    /// The request lapsed before it was acted on.
    Expired,
}

/// A data subject's request to export their data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPortabilityRequest {
    /// DID of the data subject.
    pub subject: String,
    /// Identifier of the request.
    pub request_id: String,
    /// Format the export should be produced in.
    pub format: ExportFormat,
    /// Where the request stands.
    pub status: PortabilityStatus,
}

/// Serialisation format of a data export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    /// A single JSON document.
    Json,
    /// Newline-delimited JSON.
    JsonLd,
    /// Comma-separated values.
    Csv,
}

/// Lifecycle of a portability request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortabilityStatus {
    /// Received, not yet started.
    Pending,
    /// The export is being assembled.
    Processing,
    /// The export is available.
    Ready,
    /// The export failed.
    Failed,
    /// The request lapsed before it was acted on.
    Expired,
}

/// Where a category of data may be stored, and for how long.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataResidencyPolicy {
    /// The category of data this policy applies to.
    pub data_category: String,
    /// Jurisdictions in which the category may be stored.
    pub allowed_jurisdictions: Vec<String>,
    /// Whether storing the category requires consent.
    pub requires_consent: bool,
    /// How long the category may be retained, in days.
    pub retention_days: u32,
}

/// Checks data placements against a set of residency policies.
pub struct GdprEngine {
    policies: HashMap<String, DataResidencyPolicy>,
}

impl GdprEngine {
    /// Creates an engine with no policies.
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
        }
    }

    /// Adds or replaces the policy for a data category.
    pub fn add_policy(&mut self, p: DataResidencyPolicy) {
        self.policies.insert(p.data_category.clone(), p);
    }

    /// Returns `Ok(())` when `jurisdiction` is allowed for `category`.
    ///
    /// Returns `Err` when no policy exists for the category, or when the
    /// jurisdiction is not among the allowed ones.
    pub fn check_jurisdiction(&self, category: &str, jurisdiction: &str) -> Result<(), String> {
        self.policies
            .get(category)
            .ok_or_else(|| format!("No policy for '{}'", category))
            .and_then(|p| {
                if p.allowed_jurisdictions.iter().any(|j| j == jurisdiction) {
                    Ok(())
                } else {
                    Err(format!(
                        "Jurisdiction '{}' not allowed for '{}'",
                        jurisdiction, category
                    ))
                }
            })
    }
}

impl Default for GdprEngine {
    fn default() -> Self {
        Self::new()
    }
}
