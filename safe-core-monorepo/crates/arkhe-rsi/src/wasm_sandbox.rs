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
//! ## Out-of-process isolation — why the actual wasmtime call never runs here
//!
//! An earlier version of this module ran wasmtime directly in the calling
//! process. That was a real, reproduced problem: on this development
//! machine (Windows, `wasmtime` 28.0.1), forcing execution to run out of
//! fuel mid-run crashes the *entire host process* with
//! `STATUS_STACK_BUFFER_OVERRUN` — inside wasmtime's own
//! `wasmtime_longjmp` trap-unwind helper, unwinding back out through
//! Cranelift-JIT'd call frames, before control ever returns to any Rust
//! error-handling code. Confirmed to persist with a dedicated 64MiB-stack
//! thread and in `--release`, ruling out "not enough stack" and
//! "debug-build-only" as the cause — this is a genuine wasmtime/Windows
//! interaction bug in this version, and *any* fuel budget is vulnerable to
//! it if a candidate runs long enough to exhaust it (deliberately or by
//! an accidental infinite loop — a realistic occurrence for an RSI
//! candidate, not just a hypothetical adversarial input).
//!
//! A crash that takes down the *caller* (e.g. the RSI evaluation loop
//! itself, or a whole `cargo test` run) is not an acceptable trade-off for
//! evaluating candidates that may not terminate. So the actual wasmtime
//! call now runs in a **dedicated child process**
//! (`arkhe-rsi-wasm-sandbox-runner`, a `[[bin]]` target of this crate —
//! see `src/bin/wasm_sandbox_runner.rs`), launched and wall-clock-bounded
//! by [`WasmSandboxEvaluator::with_timeout`]. If wasmtime crashes the
//! child, the parent observes an abnormal exit code (via
//! `crate::process_timeout::run_with_timeout`, the same polling/kill
//! primitive `validator.rs` already used, refactored into a shared
//! module) and returns a normal `Err(RsiError::Backend(..))` — the caller
//! never crashes, regardless of what the sandboxed candidate does.
//! `an_infinite_loop_cannot_crash_or_hang_the_caller` proves this
//! directly: a genuinely non-terminating candidate still returns from
//! `evaluate()` within the configured wall-clock timeout.
//!
//! What this does *not* claim even with this fix: this is instruction-
//! level isolation via a WASM runtime plus process-level fault isolation,
//! not OS-level sandboxing (no seccomp, no container, no VM boundary) —
//! the child is still a native process on the host, just no longer one
//! that can take the caller down with it.

use crate::evaluator::Evaluator;
use crate::process_timeout::run_with_timeout;
use arkhe_rsi_core::{Artifact, EvaluationResult, RsiError};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// A generous default fuel budget, passed through to the sandbox runner
/// child process — best-effort accounting/diagnostics
/// (`sandbox_fuel_consumed`) now that [`WasmSandboxEvaluator::with_timeout`]
/// is the actual hard guarantee against a candidate that never returns.
pub const DEFAULT_FUEL: u64 = 20_000_000_000;

/// The name of the sandbox runner binary this evaluator locates and
/// spawns — see `src/bin/wasm_sandbox_runner.rs`.
const RUNNER_BINARY_NAME: &str = "arkhe-rsi-wasm-sandbox-runner";

/// Runs a candidate artifact's `cargo test` suite inside a wasmtime WASI
/// sandbox (target `wasm32-wasip1`), in a dedicated child process. See
/// the module docs for what this does and does not protect against.
pub struct WasmSandboxEvaluator {
    manifest_dir: PathBuf,
    target_file: PathBuf,
    fuel: u64,
    timeout: Duration,
}

impl WasmSandboxEvaluator {
    pub fn new(manifest_dir: impl Into<PathBuf>, target_file: impl Into<PathBuf>) -> Self {
        Self {
            manifest_dir: manifest_dir.into(),
            target_file: target_file.into(),
            fuel: DEFAULT_FUEL,
            timeout: Duration::from_secs(30),
        }
    }

    /// Overrides the fuel budget passed to the sandbox runner (see
    /// [`DEFAULT_FUEL`]). Diagnostic/best-effort only — [`with_timeout`]
    /// is what actually bounds how long `evaluate()` can take.
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    /// Overrides the wall-clock timeout for the sandbox runner child
    /// process (default: 30s). This is the real guarantee against a
    /// candidate that never returns — if the child hasn't exited by the
    /// deadline, it is killed and `evaluate()` returns `Err`, not a hang.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
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

    /// Spawns the sandbox runner child process against `wasm_path`,
    /// wall-clock-bounded by `self.timeout`. See the module docs for why
    /// this is a separate process rather than an in-process wasmtime call.
    fn run_in_sandbox(&self, wasm_path: &Path) -> Result<SandboxOutcome, RsiError> {
        let runner = find_runner_binary()?;
        let wasm_path_str =
            wasm_path.to_str().ok_or_else(|| RsiError::Backend("wasm path is not valid UTF-8".to_string()))?;
        let fuel_str = self.fuel.to_string();

        let (success, stdout, stderr) =
            run_with_timeout(&runner, &[wasm_path_str, &fuel_str], None, self.timeout)?;

        if !success {
            return Err(RsiError::Backend(format!(
                "sandbox runner exited abnormally — most likely wasmtime itself trapped/crashed inside \
                 the isolated child process (see this module's doc comment); stderr: {stderr}"
            )));
        }

        parse_runner_stdout(&stdout)
    }
}

/// Locates the sandbox runner binary next to whatever executable is
/// currently running. Handles the `cargo test` case specifically: the
/// running test binary lives in `target/<profile>/deps/`, while `[[bin]]`
/// targets build to `target/<profile>/` (one directory up) — both
/// locations are tried.
fn find_runner_binary() -> Result<PathBuf, RsiError> {
    let current = std::env::current_exe()
        .map_err(|e| RsiError::Backend(format!("failed to locate the current executable: {e}")))?;
    let binary_name = format!("{RUNNER_BINARY_NAME}{}", std::env::consts::EXE_SUFFIX);

    let mut candidates = Vec::new();
    if let Some(dir) = current.parent() {
        candidates.push(dir.join(&binary_name));
        if let Some(parent_dir) = dir.parent() {
            candidates.push(parent_dir.join(&binary_name));
        }
    }

    candidates.into_iter().find(|p| p.is_file()).ok_or_else(|| {
        RsiError::Backend(format!(
            "could not find the sandbox runner binary ({binary_name}) next to the current executable \
             ({}) or its parent directory — run `cargo build --bin {RUNNER_BINARY_NAME}` first",
            current.display()
        ))
    })
}

fn parse_runner_stdout(stdout: &str) -> Result<SandboxOutcome, RsiError> {
    let mut exit_code = None;
    let mut fuel_consumed = None;
    for line in stdout.lines() {
        if let Some(v) = line.strip_prefix("exit_code=") {
            exit_code = v.trim().parse::<i32>().ok();
        } else if let Some(v) = line.strip_prefix("fuel_consumed=") {
            fuel_consumed = v.trim().parse::<u64>().ok();
        }
    }
    match (exit_code, fuel_consumed) {
        (Some(exit_code), Some(fuel_consumed)) => Ok(SandboxOutcome { exit_code, fuel_consumed }),
        _ => Err(RsiError::Backend(format!("sandbox runner produced unparseable output: {stdout:?}"))),
    }
}

/// The outcome of one sandbox runner execution.
pub struct SandboxOutcome {
    pub exit_code: i32,
    pub fuel_consumed: u64,
}

/// The actual wasmtime work — runs `wasm_path` inside a fuel-metered WASI
/// sandbox. **Only ever called from within the dedicated
/// `arkhe-rsi-wasm-sandbox-runner` child process** (see
/// `src/bin/wasm_sandbox_runner.rs`) — never from
/// [`WasmSandboxEvaluator`] directly. See this module's doc comment for
/// why: a wasmtime-level crash here must take down only this dedicated
/// child, not whatever process is evaluating a candidate.
pub fn run_in_sandbox_process(wasm_path: &Path, fuel: u64) -> Result<SandboxOutcome, RsiError> {
    use wasmtime::{Config, Engine, Linker, Module, Store};
    use wasmtime_wasi::preview1::{self, WasiP1Ctx};
    use wasmtime_wasi::WasiCtxBuilder;

    let mut config = Config::new();
    config.consume_fuel(true);
    let engine =
        Engine::new(&config).map_err(|e| RsiError::Backend(format!("failed to create wasmtime engine: {e}")))?;

    let module = Module::from_file(&engine, wasm_path)
        .map_err(|e| RsiError::Backend(format!("failed to load {}: {e}", wasm_path.display())))?;

    // No preopened directories, no network, no inherited env/args beyond
    // what WasiCtxBuilder's defaults provide — the sandboxed test binary
    // cannot reach anything on the host filesystem. Deliberately does NOT
    // inherit stdout/stderr either: this function itself runs inside the
    // dedicated sandbox-runner process, whose own real stdout is this
    // process's control protocol (`exit_code=`/`fuel_consumed=`, parsed
    // by the caller). A real bug, found by actually running this against
    // a candidate that printed test-harness output: inheriting the
    // sandboxed binary's stdout let its own text land on the *same*
    // stream as the runner's protocol lines, sometimes concatenated onto
    // the same line with no separating newline, corrupting the parse on
    // the caller's side. WasiCtxBuilder's default (no explicit
    // `.stdout()`/`.stderr()` call) discards the sandboxed binary's
    // output instead — this evaluator scores by exit code, not printed
    // text, so nothing of value is lost.
    let wasi_ctx: WasiP1Ctx = WasiCtxBuilder::new().build_p1();

    let mut store = Store::new(&engine, wasi_ctx);
    store.set_fuel(fuel).map_err(|e| RsiError::Backend(format!("failed to set fuel: {e}")))?;

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
                // which compiles to a wasm `unreachable` instruction, not
                // a WASI `proc_exit` (confirmed via
                // WASMTIME_BACKTRACE_DETAILS: the trapping wasm backtrace
                // runs through `std::sys::pal::wasi::helpers::abort_internal`
                // and `std::process::abort`). This is a normal "the test
                // suite ran and a test failed" outcome, not an
                // infrastructure error — any other trap kind still
                // propagates as a real `RsiError`.
                Some(wasmtime::Trap::UnreachableCodeReached) => Ok(SandboxOutcome { exit_code: 1, fuel_consumed }),
                _ => Err(RsiError::Backend(format!("sandboxed execution trapped: {trap}"))),
            },
        },
    }
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
    fn an_infinite_loop_cannot_crash_or_hang_the_caller() {
        // The whole point of the out-of-process redesign: CargoTestEvaluator
        // has no answer to `loop {}` in a #[test] at all (hangs forever);
        // an earlier version of this evaluator could be made to crash the
        // *entire test process* by exhausting fuel mid-loop (a real,
        // reproduced Windows/wasmtime bug — see the module doc comment).
        // With the sandbox runner in its own child process and a short
        // wall-clock timeout here, this test itself completing at all
        // (within the outer test harness's own default timeout) is the
        // proof that neither failure mode reaches the caller anymore.
        let dir = fixture_dir();
        let evaluator = WasmSandboxEvaluator::new(dir.path(), "src/lib.rs").with_timeout(Duration::from_secs(5));
        let artifact = Artifact::new(
            ArtifactKind::Code,
            r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    #[test]
    fn loops_forever() { loop {} }
}
"#,
        );
        // Whatever the outcome — a timeout Err, or a crash-of-the-child
        // Err, or (if wasmtime's own fuel trap happens to work cleanly
        // this time) a completed Err/Ok — evaluate() must return, not
        // hang or abort this test process.
        let _ = evaluator.evaluate(&artifact);
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
