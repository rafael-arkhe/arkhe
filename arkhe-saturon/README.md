# arkhe-saturon

Event-driven orchestrator for physics-hypothesis verification.

**Design rule:** Rust orchestrates; it never does the math. Equations pulled
from arXiv papers are carried as opaque text and delegated to an external
symbolic solver over a fenced bridge. The solver's verdict is folded back into
an immutable-reducer state.

```
  SaturonEvent ──▶ SaturonOrchestrator ──▶ ScriptRunner (external solver)
       ▲                   │                        │
       │                   ▼                        ▼
  (arXiv miner)      SaturonState.reduce      VERDICT:PASS / FAIL
                           │
                           ▼
                     SaturonCommand (to UI)
```

## Modules

| Module          | Responsibility |
|-----------------|----------------|
| `types`         | Plain domain data: `Hypothesis`, `PhysicalVariable`, `VerificationResult`, statuses. |
| `event`         | `SaturonEvent` (in) and `SaturonCommand` (out). |
| `state`         | `SaturonState` + pure, deterministic `reduce(Action)`. |
| `verify`        | `build_check_script` / `parse_verdict` — the solver protocol shape. |
| `bridge`        | `ScriptRunner` trait; `SubprocessRunner` (real, fenced) + `MockRunner` (tests). |
| `orchestrator`  | Single-consumer async loop tying it all together. |

## Fences (`SubprocessRunner`)

1. **Input cap** — scripts over `max_input_chars` (default 8 000) are rejected
   before spawning.
2. **Kill timer** — a run exceeding `timeout` (default 180 s) is aborted and the
   child is killed via `kill_on_drop`, leaving no orphan.
3. **No stderr leakage** — only stdout is returned on success.

Mutual exclusion ("one run at a time") is structural: the orchestrator is a
single-consumer loop, so it never dispatches two scripts concurrently.

## Build & test

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

24 tests (state reducer, verdict parsing, subprocess bridge incl. kill-timer and
spawn-failure, full async pipeline). `tokio` + `async-trait` only; no external
solver needed for the suite (`MockRunner`). Standalone workspace.
