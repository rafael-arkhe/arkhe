//! Verificador de invariantes do loop RSI.

#![deny(unsafe_code)]

pub mod approval;
pub mod checkpoint_store;
pub mod evaluator;
pub mod registry;
pub mod rollback;
pub mod sled_backend;
pub mod validator;
pub mod verifier;
pub mod wasm_sandbox;
mod workspace;

pub use approval::{ApprovalWorkflow, Vote};
pub use checkpoint_store::{Checkpoint, CheckpointStore};
pub use evaluator::{CargoTestEvaluator, Evaluator};
pub use registry::{InMemoryRegistryBackend, Registry, RegistryBackend};
pub use rollback::RollbackManager;
pub use sled_backend::SledRegistryBackend;
pub use validator::{RustClippyValidator, StaticValidator};
pub use verifier::Verifier;
pub use wasm_sandbox::WasmSandboxEvaluator;
