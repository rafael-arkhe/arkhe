# signature-audit

Signature-consistency auditing via pseudo-inverse for the ARKHE runtime:
given a matrix of signatures `sigs` and a message vector `msgs`, decide
whether the system is **consistent** (the equations `sigs·x = msgs` admit a
solution) within a residual threshold.

## What it provides

`audit_signature_consistency(sigs, msgs, threshold)` returns an `AuditResult`:

| Field | Meaning |
|---|---|
| `residual` | `‖sigs·x − msgs‖` for the least-squares solution `x` (or `∞` if the SVD solve fails) |
| `condition_number` | `s_max / s_min` of `sigs` (`∞` if `s_min ≤ 1e-12`) |
| `consistent` | `residual < threshold` |

The pseudo-inverse solve is done through `nalgebra`'s `SVD::solve` with a
`1e-10` tolerance.

## Example

```rust
use nalgebra::{DMatrix, DVector};
use signature_audit::audit_signature_consistency;

// Two consistent equations: x1 + x2 = 3, 2·x1 - x2 = 0  →  x = (1, 2)
let sigs = DMatrix::from_row_slice(2, 2, &[1.0, 1.0, 2.0, -1.0]);
let msgs = DVector::from_row_slice(&[3.0, 0.0]);

let r = audit_signature_consistency(&sigs, &msgs, 1e-6);
assert!(r.consistent);
assert!(r.residual < 1e-12);
println!("condition number: {}", r.condition_number);
```

The consistency test drives the ARKHE audit trail check: if a claimed
signature/state relationship is not reproducible within the threshold, the
entry is flagged rather than silently trusted.

## Guarantees

- Depends only on `nalgebra` (+ `approx` in dev).
- No `unsafe` blocks.
- Degenerate (`singular`) systems are reported as **inconsistent** with
  `condition_number = ∞` — never silently "consistent".

## License

MIT OR Apache-2.0