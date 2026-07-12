//! Mapeamento para o SWC Registry (Smart Contract Weakness Classification).

use serde::{Deserialize, Serialize};

/// Entrada do SWC Registry relevante para o crate (subconjunto usado pelos
/// invariantes FI-W implementados; o registro completo tem 37 entradas).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwcEntry {
    pub id: &'static str,
    pub title: &'static str,
}

/// Retorna as entradas do SWC Registry cobertas pelos módulos deste crate.
pub fn covered_swc_entries() -> Vec<SwcEntry> {
    vec![
        SwcEntry { id: "SWC-107", title: "Reentrancy" },
        SwcEntry { id: "SWC-105", title: "Unprotected Ether Withdrawal" },
        SwcEntry { id: "SWC-101", title: "Integer Overflow and Underflow" },
        SwcEntry { id: "SWC-112", title: "Delegatecall to Untrusted Callee" },
        SwcEntry { id: "SWC-114", title: "Transaction Order Dependence" },
        SwcEntry { id: "SWC-128", title: "DoS With Block Gas Limit" },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covered_entries_are_non_empty_and_unique() {
        let entries = covered_swc_entries();
        assert!(!entries.is_empty());
        let mut ids: Vec<&str> = entries.iter().map(|e| e.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), entries.len());
    }
}
