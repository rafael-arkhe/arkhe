//! `arkhe-rsi-wasm-sandbox-runner` — runs exactly one wasmtime WASI sandbox
//! execution, then exits. Exists as a dedicated process (not a function
//! called in-process) so that a platform-level crash inside wasmtime's
//! trap-unwind machinery is an abnormal exit code the *parent* observes,
//! not a shared-process abort that takes the caller down with it — see
//! `arkhe_rsi::wasm_sandbox`'s module doc comment for the full account of
//! why this exists (a real, reproduced Windows/wasmtime-28.0.1 bug).
//!
//! Usage: `arkhe-rsi-wasm-sandbox-runner <wasm_path> <fuel>`
//!
//! On a normal completion (including a *failing* candidate test — that's
//! still a normal completion), prints to stdout:
//! ```text
//! exit_code=<i32>
//! fuel_consumed=<u64>
//! ```
//! and exits 0. On a real setup/infrastructure error (bad args, wasmtime
//! failed to even load the module), prints to stderr and exits 1. If
//! wasmtime itself crashes, this process's exit is whatever the OS reports
//! for that (e.g. `STATUS_STACK_BUFFER_OVERRUN` on Windows) — the caller
//! (`arkhe_rsi::wasm_sandbox::WasmSandboxEvaluator`) checks for exactly
//! that via `Command`'s exit status, not by expecting this binary to
//! report it gracefully.

use arkhe_rsi::wasm_sandbox::run_in_sandbox_process;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(wasm_path), Some(fuel_str)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: arkhe-rsi-wasm-sandbox-runner <wasm_path> <fuel>");
        std::process::exit(1);
    };

    let Ok(fuel) = fuel_str.parse::<u64>() else {
        eprintln!("invalid fuel value: {fuel_str}");
        std::process::exit(1);
    };

    match run_in_sandbox_process(&PathBuf::from(wasm_path), fuel) {
        Ok(outcome) => {
            println!("exit_code={}", outcome.exit_code);
            println!("fuel_consumed={}", outcome.fuel_consumed);
        }
        Err(e) => {
            eprintln!("sandbox error: {e}");
            std::process::exit(1);
        }
    }
}
