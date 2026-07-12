//! `Evaluator` — avalia um `Artifact` de código executando o toolchain real.
//!
//! Nada de heurística de "qualidade": o `Evaluator` só responde duas
//! perguntas que dão para checar sem ambiguidade — compila? os testes
//! passam? — e devolve isso como `EvaluationResult`. Ranquear candidatos por
//! critérios mais subjetivos é trabalho de um `Selector`, que ainda não existe.

use arkhe_rsi_core::{Artifact, EvaluationResult, RsiError};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

pub trait Evaluator {
    fn evaluate(&self, artifact: &Artifact) -> Result<EvaluationResult, RsiError>;
}

/// Copia `manifest_dir` para um diretório descartável, escreve o conteúdo do
/// artefato em `target_file` (relativo à raiz copiada) e roda `cargo build` +
/// `cargo test` de verdade nessa cópia — nunca no `manifest_dir` original.
///
/// Isso importa porque `manifest_dir` normalmente é o repositório de verdade:
/// sem a cópia, cada `evaluate()` sobrescreveria o arquivo alvo no working
/// tree real, e um candidato que falha ficaria lá dentro (sujo) até a próxima
/// chamada sobrescrever de novo. A cópia garante que `manifest_dir` nunca é
/// tocado, e o diretório descartável (`tempfile::TempDir`) se apaga sozinho
/// ao sair de escopo, com sucesso ou falha.
///
/// `target/` não é copiado — cada avaliação recompila do zero. Isso troca
/// velocidade por isolamento; se compilação incremental entre avaliações for
/// necessária depois, é aqui que se otimizaria.
///
/// Score: `1.0` se compilar e todos os testes passarem; `0.5` se compilar mas
/// algum teste falhar; `0.0` se não compilar. `cargo test` só roda se o build
/// passou.
pub struct CargoTestEvaluator {
    manifest_dir: PathBuf,
    target_file: PathBuf,
}

impl CargoTestEvaluator {
    pub fn new(manifest_dir: impl Into<PathBuf>, target_file: impl Into<PathBuf>) -> Self {
        Self {
            manifest_dir: manifest_dir.into(),
            target_file: target_file.into(),
        }
    }

    fn run_cargo(&self, workspace: &Path, args: &[&str]) -> Result<(bool, Duration), RsiError> {
        let start = Instant::now();
        let status = Command::new("cargo")
            .args(args)
            .current_dir(workspace)
            .status()
            .map_err(|e| RsiError::Backend(format!("failed to spawn `cargo {}`: {e}", args.join(" "))))?;
        Ok((status.success(), start.elapsed()))
    }
}

impl Evaluator for CargoTestEvaluator {
    fn evaluate(&self, artifact: &Artifact) -> Result<EvaluationResult, RsiError> {
        let workspace = crate::workspace::isolate(&self.manifest_dir)?;

        let target_path = workspace.path().join(&self.target_file);
        std::fs::write(&target_path, &artifact.content)
            .map_err(|e| RsiError::Backend(format!("failed to write {}: {e}", target_path.display())))?;

        let (build_ok, build_time) = self.run_cargo(workspace.path(), &["build", "--quiet"])?;
        if !build_ok {
            return Ok(EvaluationResult::new(0.0)
                .with_metric("build_ok", 0.0)
                .with_metric("build_secs", build_time.as_secs_f64()));
        }

        let (tests_ok, test_time) = self.run_cargo(workspace.path(), &["test", "--quiet"])?;

        Ok(EvaluationResult::new(if tests_ok { 1.0 } else { 0.5 })
            .with_metric("build_ok", 1.0)
            .with_metric("build_secs", build_time.as_secs_f64())
            .with_metric("tests_ok", if tests_ok { 1.0 } else { 0.0 })
            .with_metric("test_secs", test_time.as_secs_f64()))
        // `workspace` sai de escopo aqui e se apaga, tenha o build passado ou não.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_rsi_core::ArtifactKind;

    const PLACEHOLDER: &str = "// placeholder original — nunca deveria sobreviver a um evaluate()\n";

    /// Cria um crate Cargo real e isolado (fora do workspace) para avaliar contra.
    fn fixture_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"eval-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), PLACEHOLDER).unwrap();
        dir
    }

    #[test]
    fn passing_code_scores_one() {
        let dir = fixture_dir();
        let evaluator = CargoTestEvaluator::new(dir.path(), "src/lib.rs");
        let artifact = Artifact::new(
            ArtifactKind::Code,
            r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adds() { assert_eq!(add(2, 2), 4); }
}
"#,
        );
        let result = evaluator.evaluate(&artifact).unwrap();
        assert_eq!(result.score, 1.0);
    }

    #[test]
    fn code_that_fails_to_compile_scores_zero() {
        let dir = fixture_dir();
        let evaluator = CargoTestEvaluator::new(dir.path(), "src/lib.rs");
        let artifact = Artifact::new(ArtifactKind::Code, "pub fn add(a: i32, b: i32) -> i32 { a + b");
        let result = evaluator.evaluate(&artifact).unwrap();
        assert_eq!(result.score, 0.0);
        assert_eq!(result.metrics.get("build_ok"), Some(&0.0));
    }

    #[test]
    fn code_that_compiles_but_fails_tests_scores_half() {
        let dir = fixture_dir();
        let evaluator = CargoTestEvaluator::new(dir.path(), "src/lib.rs");
        let artifact = Artifact::new(
            ArtifactKind::Code,
            r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adds() { assert_eq!(add(2, 2), 5); }
}
"#,
        );
        let result = evaluator.evaluate(&artifact).unwrap();
        assert_eq!(result.score, 0.5);
    }

    #[test]
    fn evaluate_never_mutates_the_original_manifest_dir() {
        let dir = fixture_dir();
        let evaluator = CargoTestEvaluator::new(dir.path(), "src/lib.rs");

        // Roda três avaliações seguidas, incluindo uma que falha ao compilar —
        // o pior caso para "sujar" o original caso a isolação não funcione.
        let good = Artifact::new(ArtifactKind::Code, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n");
        let broken = Artifact::new(ArtifactKind::Code, "pub fn add(a: i32, b: i32) -> i32 { a + b");
        evaluator.evaluate(&good).unwrap();
        evaluator.evaluate(&broken).unwrap();

        let untouched = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
        assert_eq!(untouched, PLACEHOLDER);
    }
}
