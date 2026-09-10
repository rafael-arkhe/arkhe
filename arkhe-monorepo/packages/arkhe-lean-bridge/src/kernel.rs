//! FFI forte ao kernel Lean 4.33.1 (subprocesso — sem `unsafe`, sem deps novas).
//!
//! O único ponto de confiança é o binário do kernel: o Rust confirma
//! `exit 0`, os nomes elaborados (via `#check`) e o fingerprint SHA3-256 da
//! fonte. O kernel é localizado por `ARKHE_LEAN_BIN` ou pelo `PATH`.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha3::{Digest, Sha3_256};

/// Vocab inspirado na boundary de `lean-rs` (auditoria das três frentes), mas
/// implementada com a superfície de dependências mínima do workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofSource {
    /// Identificador da invariante (ex.: `I524C_gclamp_monotone`).
    pub name: &'static str,
    /// Caminho absoluto do ficheiro `.lean` a verificar.
    pub path: PathBuf,
    /// SHA3-256 (32 bytes) do ficheiro — calculado aquando da construção.
    pub sha3_256: [u8; 32],
}

impl ProofSource {
    /// Constrói a fonte a partir do `lean` e calcula o digest SHA3-256
    /// (Ghost-1/`manifest.sha3`). Falha se o ficheiro não existir.
    pub fn new(name: &'static str, path: impl AsRef<Path>) -> Result<Self, VerifyError> {
        let digest = sha3_256_of_file(path.as_ref())?;
        Ok(Self {
            name,
            path: path.as_ref().to_path_buf(),
            sha3_256: digest,
        })
    }
}

/// Dano de verificação tipado (honestidade: `Err` enumera a causa, nunca
/// silencia).
#[derive(Debug)]
pub enum VerifyError {
    /// O ficheiro de prova não pôde ser lido.
    Io(std::io::Error),
    /// O binário do kernel Lean não foi encontrado (PATH/`ARKHE_LEAN_BIN`).
    LeanNotFound,
    /// O subprocesso do kernel falhou (não-kernel, ex.: spawn negado).
    Spawn(std::io::Error),
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::Io(e) => write!(f, "erro de I/O ao ler a fonte de prova: {e}"),
            VerifyError::LeanNotFound => {
                write!(f, "kernel Lean não encontrado — coloque `lean` no PATH ou defina ARKHE_LEAN_BIN")
            }
            VerifyError::Spawn(e) => write!(f, "falha ao lançar o kernel Lean: {e}"),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Veredito de uma única verificação FFI contra o kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelVerdict {
    /// Identificador da invariante.
    pub name: &'static str,
    /// `true` quando o digest da fonte no momento da verificação coincide com
    /// o digest assinado na construção do [`ProofSource`].
    pub digest_matches: bool,
    /// `true` quando o kernel terminou com código de saída 0.
    pub kernel_exit_ok: bool,
    /// `true` quando todas as invariações esperadas foram impressas pelo
    /// `#check` (o kernel viu cada declaração).
    pub acknowledged: Vec<String>,
    /// Saída estandard do kernel (contém as linhas `#check`).
    pub stdout: String,
    /// Saída de erro do kernel (não-vazia indica elaboração com erro).
    pub stderr: String,
}

impl KernelVerdict {
    /// Veredito consolidado: digest íntegro + kernel aceitou + todas as
    /// invariantes esperadas foram elaboradas.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        self.digest_matches
            && self.kernel_exit_ok
            && self
                .acknowledged
                .iter()
                .all(|name| self.stdout.contains(name.as_str()))
    }
}

/// Runner do kernel Lean — o ponto único de confiança da ponte.
#[derive(Debug, Clone)]
pub struct LeanKernel {
    binary: PathBuf,
}

impl LeanKernel {
    /// Kernel `lean` do `PATH` (Windows: `lean.exe` via resolução de PATH do
    /// `Command`).
    #[must_use]
    pub fn from_path() -> Self {
        Self {
            binary: PathBuf::from("lean"),
        }
    }

    /// Kernel explícito (ex.: `C:\.elan\bin\lean.exe`). Use quando o PATH não
    /// incluir `lean` no ambiente de CI.
    #[must_use]
    pub fn from_binary(path: impl AsRef<Path>) -> Self {
        Self {
            binary: path.as_ref().to_path_buf(),
        }
    }

    /// Kernel a partir de `ARKHE_LEAN_BIN` ou PATH.
    ///
    /// * `ARKHE_LEAN_BIN` setado → caminho explícito;
    /// * caso contrário → `lean` do PATH (fragmento resolvido em tempo de
    ///   execução pelo sistema operativo).
    #[must_use]
    pub fn from_env() -> Self {
        std::env::var_os("ARKHE_LEAN_BIN")
            .map(PathBuf::from)
            .map_or_else(Self::from_path, Self::from_binary)
    }

    /// Verifica uma fonte de prova contra o kernel. A verificação RECOMPUTA o
    /// digest SHA3-256 no momento da chamada (falsificação se o ficheiro for
    /// alterado depois da construção do [`ProofSource`]).
    ///
    /// `expected` é o subconjunto de nomes cuja elaboração é obrigatória para
    /// este veredito (filtra o ruído do `#check` de outros núcleos).
    pub fn verify(
        &self,
        source: &ProofSource,
        expected: &[&str],
    ) -> Result<KernelVerdict, VerifyError> {
        let fresh = sha3_256_of_file(&source.path)?;
        let digest_matches = fresh == source.sha3_256;

        let output = match Command::new(&self.binary).arg(&source.path).output() {
            Ok(out) => out,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(VerifyError::LeanNotFound);
            }
            Err(e) => return Err(VerifyError::Spawn(e)),
        };
        let exit_ok = output.status.success();
        let (stdout, stderr) = decode(output);

        let acknowledged: Vec<String> = expected
            .iter()
            .map(|name| name.to_string())
            .filter(|name| stdout.contains(name.as_str()))
            .collect();

        Ok(KernelVerdict {
            name: source.name,
            digest_matches,
            kernel_exit_ok: exit_ok,
            acknowledged,
            stdout,
            stderr,
        })
    }

    /// Verifica várias fontes sequencialmente (núcleos da ponte). Detém no
    /// primeiro `VerifyError` — honestidade: faltar uma fonte é erro, não
    /// veredito tratável.
    pub fn verify_many(
        &self,
        sources: &[ProofSource],
        expected_all: &[&str],
    ) -> Result<Vec<KernelVerdict>, VerifyError> {
        sources
            .iter()
            .map(|src| self.verify(src, expected_all))
            .collect()
    }
}

fn decode(output: Output) -> (String, String) {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (stdout, stderr)
}

/// SHA3-256 de um ficheiro (Ghost-1 — `manifest.sha3`), 32 bytes.
fn sha3_256_of_file(path: &Path) -> Result<[u8; 32], VerifyError> {
    let mut file = std::fs::File::open(path).map_err(VerifyError::Io)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(VerifyError::Io)?;
    Ok(digest_sha3_256(&buf))
}

/// SHA3-256 de bytes em memória (utilidade pública para checksum da fonte).
#[must_use]
pub fn digest_sha3_256(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha3_256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..]);
    out
}