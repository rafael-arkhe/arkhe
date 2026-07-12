//! Recon stage: normalizes the caller-supplied contract spec into
//! [`FunctionSignature`]s for the later stages to reason about.
//!
//! Deliberately does **not** parse Solidity source or EVM bytecode — this
//! crate has no disassembler/AST parser, and pretending to have one without
//! actually implementing it is exactly the kind of gap the audit flagged.
//! Callers that have a real parser upstream (e.g. `solc --ast-json`, or an
//! EVM bytecode disassembler) feed its output in as a [`FunctionSignature`]
//! list; `ReconAgent` just collects/normalizes it for the pipeline.

/// A single function's shape, as relevant to the invariants this crate
/// checks: whether it mutates state and/or makes an external call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    pub name: String,
    pub mutates_state: bool,
    pub makes_external_call: bool,
}

pub struct ReconAgent;

impl ReconAgent {
    /// Collects the function signatures out of a caller-supplied list,
    /// dropping duplicates by name (first occurrence wins).
    pub fn process(functions: &[FunctionSignature]) -> Vec<FunctionSignature> {
        let mut seen = std::collections::HashSet::new();
        functions
            .iter()
            .filter(|f| seen.insert(f.name.clone()))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_by_name() {
        let functions = vec![
            FunctionSignature { name: "withdraw".into(), mutates_state: true, makes_external_call: true },
            FunctionSignature { name: "withdraw".into(), mutates_state: true, makes_external_call: true },
            FunctionSignature { name: "deposit".into(), mutates_state: true, makes_external_call: false },
        ];
        let result = ReconAgent::process(&functions);
        assert_eq!(result.len(), 2);
    }
}
