# arkhe-observability

ARKHE Observatory — the read-only bridge between the Safe-Core anti-hallucination
policy and the Cathedral dashboard (Chladni Cloud), plus the **TPM hardware anchor**.

## What it does

- Subscribes to the recurrency daemon's `WatchTickets` stream.
- Runs a local `RecurrencyPolicy` to derive constitutional metrics (vibe, clamp
  state, R², multiplicity).
- Publishes them on the Telegraph ξM-field bus so the dashboard renders live.
- **Never** issues `SetRegime` — enforcement stays in `arkhe-safe-core`.

### Topics

| Topic | Signals |
|---|---|
| `/cathedral/stats` | `cycle_stats`, `phi_confidence` |
| `/cathedral/events` | `security_event`, `identity_attestation`, `execution_attestation`, `multiplicity_attestation` |
| `/cathedral/control` | `<metric>_response` (replies to dashboard commands) |
| `/signal/phi` | `phiInterop` |

## TPM hardware anchor

`TpmAnchor` (`src/tpm.rs`) seals an attestation signing key in the **real OS
TPM** using Windows NCrypt/CNG (SRK-bound, sealed), and signs identity /
execution attestations with it.

- The unsafe FFI for TPM calls is **confined to `tpm.rs`** and documented block
  by block.
- Crate-wide `[lints.rust] unsafe_code = "deny"` — any new unsafe outside
  `tpm.rs` fails the build.

```rust
use arkhe_observability::{TpmAnchor, Signal};

let anchor = arkhe_observability::load_tpm_anchor()?; // NCrypt-backed
let sig = anchor.sign(b"attestation")?;
```

## License

MIT OR Apache-2.0