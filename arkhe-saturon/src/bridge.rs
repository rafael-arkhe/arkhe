//! The bridge to an external solver process.
//!
//! ## Fences (enforced by [`SubprocessRunner`])
//! 1. **Input cap** — scripts longer than `max_input_chars` are rejected
//!    *before* spawning anything (anti prompt-stuffing).
//! 2. **Kill timer** — a run exceeding `timeout` is aborted and the child is
//!    killed (`kill_on_drop`), leaving no orphan.
//! 3. **No stderr leakage into the verdict** — only stdout is returned on
//!    success; stderr is surfaced only inside a `ProcessFailed` error.
//!
//! Mutual exclusion ("one run at a time") is provided structurally by the
//! orchestrator: it is a single-consumer loop, so it never dispatches two
//! scripts concurrently. The bridge therefore stays stateless.

use std::fmt;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Errors a [`ScriptRunner`] can return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// Script exceeded the input cap; never dispatched.
    TooLong { len: usize, cap: usize },
    /// The child process could not be spawned.
    Spawn(String),
    /// An I/O error while writing stdin or reading output.
    Io(String),
    /// The run exceeded the kill timer.
    Timeout { secs: u64 },
    /// The process exited non-zero.
    ProcessFailed { code: Option<i32>, stderr: String },
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BridgeError::TooLong { len, cap } => {
                write!(f, "script too long: {len} chars > cap {cap}")
            }
            BridgeError::Spawn(e) => write!(f, "failed to spawn solver: {e}"),
            BridgeError::Io(e) => write!(f, "solver i/o error: {e}"),
            BridgeError::Timeout { secs } => write!(f, "solver timed out after {secs}s"),
            BridgeError::ProcessFailed { code, stderr } => {
                write!(f, "solver exited {code:?}: {stderr}")
            }
        }
    }
}

impl std::error::Error for BridgeError {}

/// Abstraction over "hand this script to a solver, get its output back".
#[async_trait]
pub trait ScriptRunner: Send + Sync {
    async fn run(&self, script: &str) -> Result<String, BridgeError>;
}

/// Runs a script by piping it to an external program's stdin and capturing
/// stdout. The program is caller-supplied so the crate is not coupled to any
/// particular solver binary.
pub struct SubprocessRunner {
    program: String,
    args: Vec<String>,
    max_input_chars: usize,
    timeout: Duration,
}

impl SubprocessRunner {
    /// Defaults: 8 000-char cap, 180 s kill timer (matching the design fences).
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
            max_input_chars: 8_000,
            timeout: Duration::from_secs(180),
        }
    }

    pub fn with_cap(mut self, cap: usize) -> Self {
        self.max_input_chars = cap;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl ScriptRunner for SubprocessRunner {
    async fn run(&self, script: &str) -> Result<String, BridgeError> {
        let len = script.chars().count();
        if len > self.max_input_chars {
            return Err(BridgeError::TooLong {
                len,
                cap: self.max_input_chars,
            });
        }

        let mut child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| BridgeError::Spawn(e.to_string()))?;

        // Feed the script, then close stdin so a reader sees EOF.
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| BridgeError::Io("child stdin unavailable".to_string()))?;
            stdin
                .write_all(script.as_bytes())
                .await
                .map_err(|e| BridgeError::Io(e.to_string()))?;
            // `stdin` dropped here -> pipe closed.
        }

        // On timeout the future (owning `child`) is dropped; `kill_on_drop`
        // then reaps the process, so no orphan is left behind.
        let output = match tokio::time::timeout(self.timeout, child.wait_with_output()).await {
            Ok(res) => res.map_err(|e| BridgeError::Io(e.to_string()))?,
            Err(_) => {
                return Err(BridgeError::Timeout {
                    secs: self.timeout.as_secs(),
                })
            }
        };

        if !output.status.success() {
            return Err(BridgeError::ProcessFailed {
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

/// In-memory runner for tests: returns a canned response and counts calls.
/// No process, no solver — exercises the orchestrator's async plumbing.
pub struct MockRunner {
    canned: Result<String, BridgeError>,
    delay: Option<Duration>,
    calls: Arc<AtomicUsize>,
}

impl MockRunner {
    /// Always answers with a passing verdict.
    pub fn passing() -> Self {
        Self::with_output(Ok("VERDICT:PASS residual=0".to_string()))
    }

    /// Always answers with a failing verdict carrying `reason`.
    pub fn failing(reason: &str) -> Self {
        Self::with_output(Ok(format!("VERDICT:FAIL:{reason}")))
    }

    /// Always answers with a bridge error.
    pub fn erroring(err: BridgeError) -> Self {
        Self::with_output(Err(err))
    }

    /// Returns arbitrary raw output (e.g. to test verdict parsing failures).
    pub fn with_output(canned: Result<String, BridgeError>) -> Self {
        Self {
            canned,
            delay: None,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// How many times `run` has been invoked.
    pub fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ScriptRunner for MockRunner {
    async fn run(&self, _script: &str) -> Result<String, BridgeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(d) = self.delay {
            tokio::time::sleep(d).await;
        }
        self.canned.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Portable "print this then exit 0" command.
    fn echo(text: &str) -> (String, Vec<String>) {
        if cfg!(windows) {
            ("cmd".to_string(), vec!["/C".to_string(), format!("echo {text}")])
        } else {
            (
                "sh".to_string(),
                vec!["-c".to_string(), format!("printf '%s\\n' '{text}'")],
            )
        }
    }

    // Portable "sleep for a few seconds" command (used to trip the kill timer).
    fn sleeper() -> (String, Vec<String>) {
        if cfg!(windows) {
            (
                "ping".to_string(),
                vec![
                    "-n".to_string(),
                    "5".to_string(),
                    "127.0.0.1".to_string(),
                ],
            )
        } else {
            ("sh".to_string(), vec!["-c".to_string(), "sleep 5".to_string()])
        }
    }

    #[tokio::test]
    async fn subprocess_happy_path_returns_stdout() {
        let (p, a) = echo("VERDICT:PASS");
        let runner = SubprocessRunner::new(p, a);
        let out = runner.run("check_identity: a = a\n").await.unwrap();
        assert!(out.contains("VERDICT:PASS"));
    }

    #[tokio::test]
    async fn input_cap_rejects_before_spawn() {
        // Program is irrelevant: the cap check happens first.
        let runner = SubprocessRunner::new("definitely-not-a-real-binary", vec![]).with_cap(4);
        let out = runner.run("way too long").await;
        assert!(matches!(out, Err(BridgeError::TooLong { cap: 4, .. })));
    }

    #[tokio::test]
    async fn kill_timer_trips() {
        let (p, a) = sleeper();
        let runner = SubprocessRunner::new(p, a).with_timeout(Duration::from_millis(200));
        let out = runner.run("noop").await;
        assert!(matches!(out, Err(BridgeError::Timeout { .. })));
    }

    #[tokio::test]
    async fn spawn_failure_is_reported() {
        let runner = SubprocessRunner::new("definitely-not-a-real-binary-xyz", vec![]);
        let out = runner.run("noop").await;
        assert!(matches!(out, Err(BridgeError::Spawn(_))));
    }

    #[tokio::test]
    async fn mock_counts_calls() {
        let runner = MockRunner::passing();
        let _ = runner.run("x").await;
        let _ = runner.run("y").await;
        assert_eq!(runner.call_count(), 2);
    }
}
