//! Gap-filling stage: reports which of the crate's 20 formal invariants
//! (FI-W01..FI-W20) were *not* exercised by a pipeline run, so gaps are
//! disclosed explicitly instead of silently omitted — the audit's
//! recurring finding was omission-without-disclosure, not incompleteness
//! itself.

use crate::InvariantId;

/// Every invariant ID this crate defines. Kept as a literal list (rather
/// than derived via reflection, which Rust doesn't support) — update this
/// alongside any new `FI-W##` invariant added elsewhere in the crate.
pub const ALL_INVARIANT_IDS: &[InvariantId] = &[
    "FI-W01", "FI-W02", "FI-W03", "FI-W04", "FI-W05", "FI-W06", "FI-W07", "FI-W08", "FI-W09", "FI-W10",
    "FI-W11", "FI-W12", "FI-W13", "FI-W14", "FI-W15", "FI-W16", "FI-W17", "FI-W18", "FI-W19", "FI-W20",
];

pub struct GapFillingAgent;

impl GapFillingAgent {
    /// Returns every invariant ID in [`ALL_INVARIANT_IDS`] that does not
    /// appear in `checked`.
    pub fn find_gaps(checked: &[InvariantId]) -> Vec<InvariantId> {
        ALL_INVARIANT_IDS.iter().copied().filter(|id| !checked.contains(id)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_every_id_as_a_gap_when_nothing_was_checked() {
        let gaps = GapFillingAgent::find_gaps(&[]);
        assert_eq!(gaps.len(), ALL_INVARIANT_IDS.len());
    }

    #[test]
    fn excludes_checked_ids_from_the_gap_list() {
        let gaps = GapFillingAgent::find_gaps(&["FI-W03", "FI-W04", "FI-W07"]);
        assert!(!gaps.contains(&"FI-W03"));
        assert!(!gaps.contains(&"FI-W04"));
        assert!(!gaps.contains(&"FI-W07"));
        assert!(gaps.contains(&"FI-W01"));
        assert_eq!(gaps.len(), ALL_INVARIANT_IDS.len() - 3);
    }
}
