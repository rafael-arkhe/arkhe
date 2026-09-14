# arkhe-safe-core

The enforceable safety collar for the ARKHE StateDaemon — a **gRPC watchdog**
that converts the recurrency daemon's `WatchTickets` stream into
`SetRegime` policy decisions.

## Policy

Each ticket is run through `RecurrencyPolicy`, which computes the constitutional
metric **vibe = broadcast × weak_closure** (Zheng et al., 2026, Table 2). When
vibe falls below threshold, the watchdog raises a `PolicyAction` and issues
`SetRegime` (including the `DeepSleep` collateral) back to the daemon.

## Runtime

The `safe_core` binary wires it all together:

- `run_watchdog_with_recorder` — the enforcement loop.
- `Fanout` → `TelemetrySink` (append-only JSONL, `safe-core-telemetry.jsonl`).
- `MetricsServer` (HTTP): `/api/v1/policy/metrics`, `/api/v1/policy/tickets`,
  `/healthz`.

### Configuration (environment)

| Variable | Default | Purpose |
|----------|---------|---------|
| `RECURRENCY_GRPC_ADDR` | `http://127.0.0.1:50051` | recurrency daemon endpoint |
| `SAFE_CORE_TELEMETRY_PATH` | `safe-core-telemetry.jsonl` | JSONL append-only log |
| `SAFE_CORE_METRICS_ADDR` | `127.0.0.1:8080` | metrics HTTP listener |

Runtime constants: `VIBE_THRESHOLD = 0.8`, `VIBE_WINDOW = 8`,
`CLAMP_COOLDOWN = 12`.

> **Scope honesty:** this is the *enforcement watchdog*, not a "provably
> unfireable safety kernel". Its decision rule is small and read-only-testable;
> formalization of the policy decision rule is a tracked Lean-nucleus work item.

## Guarantees

- `[lints.rust] unsafe_code = "deny"` — zero unsafe.
- Read-only observability legs: the watchdog only writes policy decisions;
  the metrics/telemetry paths never re-inject state.

## License

MIT OR Apache-2.0