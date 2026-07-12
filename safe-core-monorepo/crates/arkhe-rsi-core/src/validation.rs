use serde::{Deserialize, Serialize};

/// Resultado de uma validação estática (`cargo check`, clippy, eslint, etc.)
/// — roda antes de gastar tempo com execução em sandbox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub passes: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub language: String,
    pub tool_name: String,
}

impl ValidationReport {
    pub fn new(tool_name: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            passes: true,
            warnings: Vec::new(),
            errors: Vec::new(),
            tool_name: tool_name.into(),
            language: language.into(),
        }
    }
}
