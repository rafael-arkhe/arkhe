//! `WasmSandboxEvaluator` — an [`Evaluator`] that runs a candidate's tests
//! inside a capability-restricted WASI sandbox (`wasmtime`), instead of
//! `CargoTestEvaluator`'s direct host execution.
//!
//! This closes part of the gap `evaluator.rs`'s own README section
//! discloses: `CargoTestEvaluator` runs untrusted candidate code with the
//! full privileges of the host process. The WASI context here grants no
//! preopened directories, no network, and no inherited environment — the
//! sandboxed binary cannot read or write anything on the host filesystem,
//! unlike a native `cargo test` process (confirmed, not just configured:
//! every test below runs a real candidate through a real
//! `wasm32-wasip1` build and a real `wasmtime` execution).
//!
//! ## A real, reproduced platform limitation — read before relying on fuel
//!
//! wasmtime supports metering execution in "fuel" (an abstract
//! instruction-count budget) specifically so a runaway candidate — e.g. a
//! `#[test]` body that is `loop {}`, which hangs `CargoTestEvaluator`
//! forever with no way to stop it — can be interrupted. This evaluator
//! *configures* a fuel budget (see [`DEFAULT_FUEL`]/[`with_fuel`]) and
//! reports consumption via the `sandbox_fuel_consumed` metric. **What it
//! cannot currently do, on this development machine (Windows,
//! `wasmtime` 28.0.1), is safely let that budget actually run out.**
//! Reproduced directly, not hypothesized: forcing fuel exhaustion mid-
//! execution (via a `loop {}` candidate and a tiny fuel budget) crashes
//! the *entire host process* with `STATUS_STACK_BUFFER_OVERRUN`, not a
//! catchable `Result::Err` — the crash happens inside wasmtime's own
//! `wasmtime_longjmp` trap-unwind helper while unwinding back out through
//! Cranelift-JIT'd call frames, before control ever returns to any Rust
//! error-handling code in this crate. Neither a larger stack for the
//! executing thread (tried: a dedicated 64MiB-stack thread hit the
//! identical crash) nor `--release` (tried: identical crash, same
//! `STATUS_STACK_BUFFER_OVERRUN`, in an optimized build) changes this,
//! which is consistent with this being a genuine wasmtime/Windows
//! interaction bug in this version, not a resource or optimization-level
//! artifact of this crate's own code.
//!
//! Consequences of this, stated plainly: **do not rely on fuel exhaustion
//! to safely interrupt a candidate on Windows with this wasmtime version.**
//! There is no test here exercising actual fuel exhaustion, because doing
//! so reliably aborts the whole test process rather than failing one test.
//! A real fix for hard interruption on this platform would need
//! out-of-process isolation (running the sandboxed `wasmtime` execution in
//! a genuinely separate child process, so a crash there is an abnormal
//! exit code the parent observes, not a shared-process abort) — not
//! attempted in this pass.
//!
//! What this evaluator *does* deliver right now, confirmed by tests: real
//! WASI capability restriction for well-behaved candidates, and accurate
//! fuel-consumption accounting for executions that complete normally.
//!
//! What this does *not* claim even where it does work: this is
//! instruction-level isolation via a WASM runtime, not OS-level sandboxing
//! (no seccomp, no container, no VM boundary) — `wasmtime` itself is still
//! a native process on the host.

use crate::evaluator::Evaluator;
use arkhe_rsi_core::{Artifact, EvaluationResult, RsiError};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

/// A generous default fuel budget — enough for the WASI Preview 1 test
/// harness's own startup plus a realistic candidate test body. See the
/// module docs' "platform limitation" section before lowering this to try
/// to force exhaustion on Windows.
pub const DEFAULT_FUEL: u64 = 20_000_000_000;

/// Runs a candidate artifact's `cargo test` suite inside a wasmtime WASI
/// sandbox (target `wasm32-wasip1`) rather than directly on the host. See
/// the module docs for what this does and does not protect against.
pub struct WasmSandboxEvaluator {
    manifest_dir: PathBuf,
    target_file: PathBuf,
    fuel: u64,
}

impl WasmSandboxEvaluator {
    pub fn new(manifest_dir: impl Into<PathBuf>, target_file: impl Into<PathBuf>) -> Self {
        Self { manifest_dir: manifest_dir.into(), target_file: target_file.into(), fuel: DEFAULT_FUEL }
    }

    /// Overrides the fuel budget (see [`DEFAULT_FUEL`] and the module docs'
    /// "platform limitation" section — do not set this low expecting a
    /// clean `Err` on exhaustion on Windows with this wasmtime version).
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    /// Compiles the candidate's `wasm32-wasip1` test binary without running
    /// it (`--no-run`), parsing `--message-format=json` output to find the
    /// produced `.wasm` path — the same structured-diagnostics approach
    /// `validator.rs` already uses, rather than scraping human-formatted
    /// text. Returns `Ok(None)` if the build itself failed.
    fn build_test_wasm(&self, workspace: &Path) -> Result<Option<PathBuf>, RsiError> {
        let output = Command::new("cargo")
            .args(["test", "--no-run", "--target", "wasm32-wasip1", "--message-format=json", "--quiet"])
            .current_dir(workspace)
            .output()
            .map_err(|e| RsiError::Backend(format!("failed to spawn `cargo test --no-run`: {e}")))?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
            if value.get("reason").and_then(Value::as_str) != Some("compiler-artifact") {
                continue;
            }
            let is_test_profile =
                value.get("profile").and_then(|p| p.get("test")).and_then(Value::as_bool).unwrap_or(false);
            if !is_test_profile {
                continue;
            }
            if let Some(executable) = value.get("executable").and_then(Value::as_str) {
                return Ok(Some(PathBuf::from(executable)));
            }
        }
        Ok(None)
    }

    /// Runs `wasm_path` inside a fuel-metered WASI sandbox. See the module
    /// docs' "platform limitation" section: this returns a proper `Result`
    /// for normal completion (pass, fail, or a non-fuel trap), but must not
    /// be relied on to do so if `self.fuel` is actually exhausted on
    /// Windows with the currently-pinned wasmtime version.
    fn run_in_sandbox(&self, wasm_path: &Path) -> Result<SandboxOutcome, RsiError> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine =
            Engine::new(&config).map_err(|e| RsiError::Backend(format!("failed to create wasmtime engine: {e}")))?;

        let module = Module::from_file(&engine, wasm_path)
            .map_err(|e| RsiError::Backend(format!("failed to load {}: {e}", wasm_path.display())))?;

        // No preopened directories, no network, no inherited env/args
        // beyond what WasiCtxBuilder's defaults provide — the sandboxed
        // test binary cannot reach anything on the host filesystem.
        let wasi_ctx: WasiP1Ctx = WasiCtxBuilder::new().inherit_stdout().inherit_stderr().build_p1();

        let mut store = Store::new(&engine, wasi_ctx);
        store.set_fuel(self.fuel).map_err(|e| RsiError::Backend(format!("failed to set fuel: {e}")))?;

        let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
        preview1::add_to_linker_sync(&mut linker, |ctx| ctx)
            .map_err(|e| RsiError::Backend(format!("failed to wire WASI into linker: {e}")))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| RsiError::Backend(format!("failed to instantiate wasm module: {e}")))?;
        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|e| RsiError::Backend(format!("wasm module has no WASI _start export: {e}")))?;

        let fuel_before = store.get_fuel().unwrap_or(0);
        let result = start.call(&mut store, ());
        let fuel_after = store.get_fuel().unwrap_or(0);
        let fuel_consumed = fuel_before.saturating_sub(fuel_after);

        match result {
            Ok(()) => Ok(SandboxOutcome { exit_code: 0, fuel_consumed }),
            Err(trap) => match trap.downcast_ref::<wasmtime_wasi::I32Exit>() {
                Some(exit) => Ok(SandboxOutcome { exit_code: exit.0, fuel_consumed }),
                None => match trap.downcast_ref::<wasmtime::Trap>() {
                    // On `wasm32-wasip1`, a failed `assert_eq!` inside a
                    // `#[test]` panics, which the target's panic-abort
                    // configuration turns into `std::process::abort()` —
                    // which compiles to a wasm `unreachable` instruction,
                    // not a WASI `proc_exit` (confirmed via
                    // WASMTIME_BACKTRACE_DETAILS: the trapping wasm
                    // backtrace runs through
                    // `std::sys::pal::wasi::helpers::abort_internal` and
                    // `std::process::abort`). This is a normal "the test
                    // suite ran and a test failed" outcome, not an
                    // infrastructure error — any other trap kind still
                    // propagates as a real `RsiError`.
                    Some(wasmtime::Trap::UnreachableCodeReached) => {
                        Ok(SandboxOutcome { exit_code: 1, fuel_consumed })
                    }
                    _ => Err(RsiError::Backend(format!("sandboxed execution trapped: {trap}"))),
                },
            },
        }
    }
}

struct SandboxOutcome {
    exit_code: i32,
    fuel_consumed: u64,
}

impl Evaluator for WasmSandboxEvaluator {
    fn evaluate(&self, artifact: &Artifact) -> Result<EvaluationResult, RsiError> {
        let workspace = crate::workspace::isolate(&self.manifest_dir)?;

        let target_path = workspace.path().join(&self.target_file);
        std::fs::write(&target_path, &artifact.content)
            .map_err(|e| RsiError::Backend(format!("failed to write {}: {e}", target_path.display())))?;

        let Some(wasm_path) = self.build_test_wasm(workspace.path())? else {
            return Ok(EvaluationResult::new(0.0).with_metric("build_ok", 0.0));
        };

        let outcome = self.run_in_sandbox(&wasm_path)?;
        let tests_ok = outcome.exit_code == 0;

        Ok(EvaluationResult::new(if tests_ok { 1.0 } else { 0.5 })
            .with_metric("build_ok", 1.0)
            .with_metric("tests_ok", if tests_ok { 1.0 } else { 0.0 })
            .with_metric("sandbox_fuel_consumed", outcome.fuel_consumed as f64))
        // `workspace` goes out of scope here and deletes itself, whether
        // the build/sandbox run succeeded or not.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_rsi_core::ArtifactKind;

    const PLACEHOLDER: &str = "// placeholder original — should never survive an evaluate()\n";

    fn fixture_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"wasm-eval-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), PLACEHOLDER).unwrap();
        dir
    }

    #[test]
    fn passing_code_scores_one_in_the_sandbox() {
        let dir = fixture_dir();
        let evaluator = WasmSandboxEvaluator::new(dir.path(), "src/lib.rs");
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
        // Real fuel accounting for a real, completed execution — not just
        // a fixed/fake value.
        assert!(result.metrics.get("sandbox_fuel_consumed").copied().unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn code_that_fails_to_compile_scores_zero_in_the_sandbox() {
        let dir = fixture_dir();
        let evaluator = WasmSandboxEvaluator::new(dir.path(), "src/lib.rs");
        let artifact = Artifact::new(ArtifactKind::Code, "pub fn add(a: i32, b: i32) -> i32 { a + b");
        let result = evaluator.evaluate(&artifact).unwrap();
        assert_eq!(result.score, 0.0);
        assert_eq!(result.metrics.get("build_ok"), Some(&0.0));
    }

    #[test]
    fn code_that_compiles_but_fails_tests_scores_half_in_the_sandbox() {
        let dir = fixture_dir();
        let evaluator = WasmSandboxEvaluator::new(dir.path(), "src/lib.rs");
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
    fn candidate_code_cannot_read_the_host_filesystem() {
        // The real capability restriction this evaluator delivers today:
        // a candidate that tries to read an arbitrary host path (not one
        // preopened for it — none are) must fail to do so, proving the
        // sandbox's WASI context actually withholds filesystem access
        // rather than merely being configured to (a passing test here
        // would be a false negative if WasiCtxBuilder defaults ever
        // silently changed to inherit access).
        let dir = fixture_dir();
        let evaluator = WasmSandboxEvaluator::new(dir.path(), "src/lib.rs");
        let artifact = Artifact::new(
            ArtifactKind::Code,
            r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    #[test]
    fn cannot_read_cargo_toml_in_the_sandboxed_workspace() {
        assert!(std::fs::read_to_string("Cargo.toml").is_err());
    }
}
"#,
        );
        let result = evaluator.evaluate(&artifact).unwrap();
        assert_eq!(result.score, 1.0, "the read must fail inside the sandbox for this test to pass");
    }

    #[test]
    fn evaluate_never_mutates_the_original_manifest_dir() {
        let dir = fixture_dir();
        let evaluator = WasmSandboxEvaluator::new(dir.path(), "src/lib.rs");

        let good = Artifact::new(ArtifactKind::Code, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n");
        let broken = Artifact::new(ArtifactKind::Code, "pub fn add(a: i32, b: i32) -> i32 { a + b");
        evaluator.evaluate(&good).unwrap();
        evaluator.evaluate(&broken).unwrap();

        let untouched = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
        assert_eq!(untouched, PLACEHOLDER);
    }
}
