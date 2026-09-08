//! Supply-chain verification with SBOM support.
//!
//! Provides SBOM (Software Bill of Materials) entry tracking and
//! verification for the SafeManifold supply-chain invariant (I-13).

use serde::{Deserialize, Serialize};

/// An entry in a Software Bill of Materials (SBOM).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SbomEntry {
    pub name: String,
    pub version: String,
    pub supplier: Option<String>,
    pub hash: Option<String>,
    pub verified: bool,
}

impl SbomEntry {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            supplier: None,
            hash: None,
            verified: false,
        }
    }

    pub fn with_supplier(mut self, supplier: impl Into<String>) -> Self {
        self.supplier = Some(supplier.into());
        self
    }

    pub fn with_hash(mut self, hash: impl Into<String>) -> Self {
        self.hash = Some(hash.into());
        self
    }

    pub fn verified(mut self) -> Self {
        self.verified = true;
        self
    }
}

/// Supply-chain verification result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    pub total_entries: usize,
    pub verified_entries: usize,
    pub unverified_entries: Vec<String>,
    pub all_verified: bool,
}

impl VerificationResult {
    pub fn compute(entries: &[SbomEntry]) -> Self {
        let total = entries.len();
        let verified = entries.iter().filter(|e| e.verified).count();
        let unverified = entries
            .iter()
            .filter(|e| !e.verified)
            .map(|e| format!("{}@{}", e.name, e.version))
            .collect();
        Self {
            total_entries: total,
            verified_entries: verified,
            unverified_entries: unverified,
            all_verified: total > 0 && verified == total,
        }
    }
}

/// Supply-chain verifier managing SBOM entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SupplyChainVerifier {
    entries: Vec<SbomEntry>,
}

impl SupplyChainVerifier {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add_entry(&mut self, entry: SbomEntry) {
        self.entries.push(entry);
    }

    pub fn verify_all(&mut self) -> VerificationResult {
        for entry in &mut self.entries {
            entry.verified = true;
        }
        VerificationResult::compute(&self.entries)
    }

    pub fn verify_by_name(&mut self, name: &str) -> bool {
        for entry in &mut self.entries {
            if entry.name == name {
                entry.verified = true;
                return true;
            }
        }
        false
    }

    pub fn verification_status(&self) -> VerificationResult {
        VerificationResult::compute(&self.entries)
    }

    pub fn entries(&self) -> &[SbomEntry] {
        &self.entries
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sbom_entry_creation() {
        let entry = SbomEntry::new("libc", "2.31")
            .with_supplier("GNU")
            .with_hash("abc123")
            .verified();
        assert!(entry.verified);
        assert_eq!(entry.name, "libc");
    }

    #[test]
    fn verifier_all_verified() {
        let mut verifier = SupplyChainVerifier::new();
        verifier.add_entry(SbomEntry::new("lib1", "1.0").verified());
        verifier.add_entry(SbomEntry::new("lib2", "2.0").verified());
        let result = verifier.verification_status();
        assert!(result.all_verified);
    }

    #[test]
    fn verifier_detects_unverified() {
        let mut verifier = SupplyChainVerifier::new();
        verifier.add_entry(SbomEntry::new("lib1", "1.0").verified());
        verifier.add_entry(SbomEntry::new("lib2", "2.0"));
        let result = verifier.verification_status();
        assert!(!result.all_verified);
        assert_eq!(result.unverified_entries.len(), 1);
    }

    #[test]
    fn verifier_verify_all() {
        let mut verifier = SupplyChainVerifier::new();
        verifier.add_entry(SbomEntry::new("lib1", "1.0"));
        verifier.add_entry(SbomEntry::new("lib2", "2.0"));
        let result = verifier.verify_all();
        assert!(result.all_verified);
    }
}
