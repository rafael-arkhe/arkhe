// Catedral OS v304.0 — Pool de provadores ZK exposto publicamente (I359).
//
// A feature `nexus` acopla o backend real Nexus (nexus-zkvm). ATENÇÃO: a
// superfície de API do repositório nexus-zkvm@main mudou múltiplas vezes;
// o trocadilho de crates deve ser revalidado na campanha de CI do target
// antes de habilitar a feature. A feature default usa SimProver.

pub mod prover_pool;

pub use prover_pool::{
    run_scaling_test, scaling_satisfies_linearity, Proof, Prover, ProverError, ProverPool,
    ProverTask, SimProver, ZkResult,
};

#[cfg(feature = "nexus")]
compile_error!(
    "feature `nexus`: o backend Nexus não pode ser ativado nesta revisão — \
     o repositório nexus-xyz/nexus-zkvm renomeou o crate (nexus-zkvm -> nexus-vm). \
     Faça o onboard no CI do alvo de validação e ative I359 real lá."
);