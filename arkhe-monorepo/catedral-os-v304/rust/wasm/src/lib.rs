// Catedral OS v304.0 — Sandbox WASM (I361).

pub mod contract;

pub use contract::{ContractConfig, ContractSandbox};

use anyhow::Result;

/// Resultado da execução de um contrato.
#[derive(Debug)]
pub struct ContractOutput {
    pub output: Vec<u8>,
    pub fuel_used: u64,
    pub fuel_budget: u64,
}

/// Helper de execução sandboxed em um único passo.
pub fn run_contract(wasm: &[u8], input: &[u8]) -> Result<ContractOutput> {
    let mut sandbox = ContractSandbox::compile(wasm, ContractConfig::default())?;
    let budget = sandbox.fuel_budget();
    let (output, fuel_used) = sandbox.execute(input)?;
    Ok(ContractOutput {
        output,
        fuel_used,
        fuel_budget: budget,
    })
}