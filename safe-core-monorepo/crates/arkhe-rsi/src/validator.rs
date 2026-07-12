//! `StaticValidator` — validação estática (compila? lints?) antes do sandbox.
//!
//! Roda antes do `Evaluator`: se o candidato nem compila, não faz sentido
//! gastar tempo rodando testes/benchmarks em sandbox por trás dele.

use crate::process_timeout::run_with_timeout;
use crate::workspace::isolate;
use arkhe_rsi_core::{Artifact, RsiError, ValidationReport};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub trait StaticValidator: Send + Sync {
    fn validate(&self, artifact: &Artifact) -> Result<ValidationReport, RsiError>;
}

/// Roda `cargo check` e, se passar, `cargo clippy` — ambos com
/// `--message-format=json`, para extrair erros/warnings estruturados em vez
/// de recortar texto de saída pensada para humano.
///
/// Reaproveita o isolamento de `crate::workspace`: nunca escreve em
/// `manifest_dir`, só numa cópia descartável.
///
/// `passes` reflete o código de saída dos dois comandos — clippy por padrão
/// só falha (`exit != 0`) em lints `deny`; warnings comuns (`clippy::all` é
/// `warn` por padrão) aparecem em `warnings` mas não derrubam `passes`. Se
/// isso for indesejado depois, é questão de passar `-D warnings` para clippy,
/// não de mudar este validador.
pub struct RustClippyValidator {
    manifest_dir: PathBuf,
    target_file: PathBuf,
    timeout: Duration,
}

impl RustClippyValidator {
    pub fn new(manifest_dir: impl Into<PathBuf>, target_file: impl Into<PathBuf>) -> Self {
        Self {
            manifest_dir: manifest_dir.into(),
            target_file: target_file.into(),
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn run_diagnostics(
        &self,
        workspace: &Path,
        args: &[&str],
    ) -> Result<(bool, Vec<String>, Vec<String>), RsiError> {
        let mut full_args = args.to_vec();
        full_args.push("--message-format=json");
        let (success, stdout, _stderr) = run_with_timeout("cargo", &full_args, Some(workspace), self.timeout)?;

        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        for line in stdout.lines() {
            let Ok(value) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if value.get("reason").and_then(Value::as_str) != Some("compiler-message") {
                continue;
            }
            let Some(message) = value.get("message") else {
                continue;
            };
            let rendered = message
                .get("rendered")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            match message.get("level").and_then(Value::as_str) {
                Some("error") => errors.push(rendered),
                Some("warning") => warnings.push(rendered),
                _ => {}
            }
        }

        Ok((success, errors, warnings))
    }
}

impl StaticValidator for RustClippyValidator {
    fn validate(&self, artifact: &Artifact) -> Result<ValidationReport, RsiError> {
        let workspace = isolate(&self.manifest_dir)?;

        let target_path = workspace.path().join(&self.target_file);
        std::fs::write(&target_path, &artifact.content)
            .map_err(|e| RsiError::Backend(format!("failed to write {}: {e}", target_path.display())))?;

        let mut report = ValidationReport::new("cargo-check+clippy", "rust");

        let (check_ok, check_errors, check_warnings) =
            self.run_diagnostics(workspace.path(), &["check"])?;
        report.errors.extend(check_errors);
        report.warnings.extend(check_warnings);

        if !check_ok {
            report.passes = false;
            return Ok(report);
        }

        let (clippy_ok, clippy_errors, clippy_warnings) =
            self.run_diagnostics(workspace.path(), &["clippy"])?;
        report.errors.extend(clippy_errors);
        report.warnings.extend(clippy_warnings);
        report.passes = clippy_ok;

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_rsi_core::ArtifactKind;

    const PLACEHOLDER: &str = "// placeholder original — nunca deveria sobreviver a um validate()\n";

    fn fixture_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"validator-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), PLACEHOLDER).unwrap();
        dir
    }

    #[test]
    fn clean_code_passes() {
        let dir = fixture_dir();
        let validator = RustClippyValidator::new(dir.path(), "src/lib.rs");
        let artifact = Artifact::new(ArtifactKind::Code, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n");

        let report = validator.validate(&artifact).unwrap();
        assert!(report.passes, "errors: {:?}", report.errors);
        assert_eq!(report.language, "rust");
    }

    #[test]
    fn syntactically_broken_code_fails() {
        let dir = fixture_dir();
        let validator = RustClippyValidator::new(dir.path(), "src/lib.rs");
        let artifact = Artifact::new(ArtifactKind::Code, "pub fn add(a: i32, b: i32) -> i32 { a + b");

        let report = validator.validate(&artifact).unwrap();
        assert!(!report.passes);
        assert!(!report.errors.is_empty());
    }

    #[test]
    fn clippy_lint_is_a_warning_not_a_failure() {
        let dir = fixture_dir();
        let validator = RustClippyValidator::new(dir.path(), "src/lib.rs");
        // clippy::needless_return é `warn` por padrão, não `deny`.
        let artifact = Artifact::new(
            ArtifactKind::Code,
            "pub fn add(a: i32, b: i32) -> i32 {\n    return a + b;\n}\n",
        );

        let report = validator.validate(&artifact).unwrap();
        assert!(report.passes, "errors: {:?}", report.errors);
        assert!(!report.warnings.is_empty());
    }

    #[test]
    fn validate_never_mutates_the_original_manifest_dir() {
        let dir = fixture_dir();
        let validator = RustClippyValidator::new(dir.path(), "src/lib.rs");

        validator
            .validate(&Artifact::new(ArtifactKind::Code, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n"))
            .unwrap();
        validator
            .validate(&Artifact::new(ArtifactKind::Code, "pub fn add(a: i32, b: i32) -> i32 { a + b"))
            .unwrap();

        let untouched = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
        assert_eq!(untouched, PLACEHOLDER);
    }
}
