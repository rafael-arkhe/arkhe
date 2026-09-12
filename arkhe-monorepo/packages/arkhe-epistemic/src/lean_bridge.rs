//! Ponte para o Lean 4.

use std::process::Command;

/// Resultado de uma chamada ao verificador Lean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeanResult {
    /// Se a verificação terminou com sucesso.
    pub success: bool,
    /// Stdout do processo Lean.
    pub stdout: String,
    /// Stderr do processo Lean.
    pub stderr: String,
}

/// Verificador formal via Lean 4.
pub struct LeanVerifier {
    lean_path: String,
    lean_file: String,
}

impl LeanVerifier {
    /// Cria um novo verificador apontando para o ficheiro `.lean` dado.
    pub fn new(lean_file: &str) -> Self {
        Self {
            lean_path: "lean".to_string(),
            lean_file: lean_file.to_string(),
        }
    }

    /// Verifica um teorema invocando `lean <file> --run <theorem>`.
    pub fn verify(&self, theorem: &str) -> LeanResult {
        let output = Command::new(&self.lean_path)
            .arg(&self.lean_file)
            .arg("--run")
            .arg(theorem)
            .output();

        match output {
            Ok(out) => LeanResult {
                success: out.status.success(),
                stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            },
            Err(e) => LeanResult {
                success: false,
                stdout: String::new(),
                stderr: e.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lean_verifier_cria_instancia() {
        let v = LeanVerifier::new("formal/lean/ArkheEpistemic.lean");
        assert_eq!(v.lean_path, "lean");
        assert_eq!(v.lean_file, "formal/lean/ArkheEpistemic.lean");
    }

    #[test]
    fn lean_verifier_sem_lean_retorna_erro() {
        let v = LeanVerifier {
            lean_path: "lean_inexistente_12345".to_string(),
            lean_file: "dummy.lean".to_string(),
        };
        let r = v.verify("dummy_theorem");
        assert!(!r.success);
        assert!(r.stderr.contains("NotFoundException") || r.stderr.contains("not found"));
    }
}
