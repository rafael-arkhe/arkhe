//! Shared "run a command with a wall-clock timeout" helper. Extracted
//! rather than duplicated: both `validator.rs` (`cargo check`/`clippy`)
//! and `wasm_sandbox.rs` (the out-of-process sandbox runner) need the
//! identical drain-stdout/stderr-concurrently-while-polling pattern — a
//! command that fills the OS pipe buffer would otherwise hang forever
//! waiting for someone to read it, even with a timeout on the wait itself.

use arkhe_rsi_core::RsiError;
use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Runs `program args...` (optionally in `cwd`), killing it if it hasn't
/// exited within `timeout`. Returns `(success, stdout, stderr)`.
pub(crate) fn run_with_timeout<S: AsRef<OsStr>>(
    program: S,
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Result<(bool, String, String), RsiError> {
    let mut command = Command::new(program);
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let mut child =
        command.spawn().map_err(|e| RsiError::Backend(format!("failed to spawn `{}`: {e}", args.join(" "))))?;

    let (stdout, stderr) = drain_output(&mut child);

    let start = Instant::now();
    let status = loop {
        if let Some(status) =
            child.try_wait().map_err(|e| RsiError::Backend(format!("failed to poll process: {e}")))?
        {
            break status;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RsiError::Backend(format!("process timed out after {timeout:?}")));
        }
        thread::sleep(Duration::from_millis(25));
    };

    Ok((status.success(), stdout.collect(), stderr.collect()))
}

struct PipeReader(mpsc::Receiver<String>);

impl PipeReader {
    fn collect(self) -> String {
        self.0.recv().unwrap_or_default()
    }
}

fn drain_output(child: &mut Child) -> (PipeReader, PipeReader) {
    let mut stdout_pipe = child.stdout.take().expect("stdout piped");
    let mut stderr_pipe = child.stderr.take().expect("stderr piped");

    let (out_tx, out_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout_pipe.read_to_string(&mut buf);
        let _ = out_tx.send(buf);
    });

    let (err_tx, err_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_pipe.read_to_string(&mut buf);
        let _ = err_tx.send(buf);
    });

    (PipeReader(out_rx), PipeReader(err_rx))
}
