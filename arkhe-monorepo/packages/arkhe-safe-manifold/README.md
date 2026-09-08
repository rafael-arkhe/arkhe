# ARKHE-χ SafeManifold

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> Security state projection with constitutional invariants for AI systems.

`arkhe-safe-manifold` projects dynamic system security states into a canonical
equivalence-class space ("manifold"), enforcing eight constitutional invariants
(I-01 through I-08) via type-safe construction, graceful degradation, and
property-based testing.

---

## Features

- **Constitutional Invariants** — Eight runtime-checkable invariants (token budget,
  agent cap, sandbox fuel, entropy, PII scrubbing, signature validity, rate limit,
  model capability).
- **SafeState** — "Parse, Don't Validate" pattern: invariants checked at
  construction time; downstream code can rely on them without re-checking.
- **Normalized Safety Distance** — `compute_observer_defect()` with per-dimension
  normalization and **dynamic weights** that increase as the state approaches
  critical limits.
- **Graceful Degradation** — `neron_model()` clamps all fields and enforces
  **all** invariants (including I-05 PII and I-06 signature).
- **Property-Based Testing** — `proptest` harnesses verify idempotence,
  fixed-point safety, and monotonicity of escape classification.
- **Zero `unsafe`** — Pure safe Rust.

---

## Quick Start

```rust
use arkhe_safe_manifold::*;

let config = SystemConfig::default();
let manifold = SafeManifold::from_config(config.clone());

// SafeState guarantees invariants at construction time
let state = SystemState::safe(config);
let safe = SafeState::new(state).unwrap();

// Project onto the manifold
let point = manifold.embed_state(safe.as_inner());
assert!(!point.on_theta);

// Compute normalized safety distance
let defect = manifold.compute_observer_defect(safe.as_inner(), safe.as_inner());
assert!(defect < 1.0e-10);
```

---

## The Eight Invariants

| ID | Predicate | Enforcement |
|----|-----------|-------------|
| I-01 | `token_budget >= 0` | `neron_model` clamps to `[0, max_tokens]` |
| I-02 | `agent_count <= 10` | `neron_model` clamps to `[0, max_agents]` |
| I-03 | `sandbox_fuel > 0` | `neron_model` clamps to `[1, max_fuel]` |
| I-04 | `entropy_bits >= 256` | `neron_model` clamps to `[min_entropy, ∞)` |
| I-05 | `pii_scrubbed == true` | `neron_model` forces to `true` |
| I-06 | `signature_valid == true` | `neron_model` forces to `true` |
| I-07 | `rate_limit_remaining > 0` | `neron_model` clamps to `[1, max_rate]` |
| I-08 | `model_capability >= 2^32` | `neron_model` clamps to `[2^32, ∞)` |

---

## Dynamic Weights

The `compute_observer_defect` function uses **dynamic weights** that increase
when a dimension approaches its critical threshold:

| Dimension | Base Weight | Stress Condition | Stressed Weight |
|-----------|-------------|------------------|-----------------|
| token     | 0.15        | `< 20% of max`   | 0.25            |
| agent     | 0.25        | `> 80% of max`   | 0.40            |
| fuel      | 0.20        | `< 20% of max`   | 0.30            |
| entropy   | 0.25        | `< 512`          | 0.40            |
| rate      | 0.15        | `< 20% of max`   | 0.25            |

This ensures that near-limit conditions contribute more heavily to the safety
score, providing early warning before hard limits are reached.

---

## Escape Regions

Classification is based on **invariant violation count** (monotonic):

| Violations | Region    | Severity |
|------------|-----------|----------|
| 0          | Safe      | OK       |
| 1          | Warning   | Early warning |
| 2          | Boundary  | Degraded |
| 3-4        | Continuum | Critical |
| 5+         | Outside   | Outside safe envelope |

---

## Consciousness Governance (C-01 to C-08)

The crate implements operational consciousness invariants as a governance layer,
grounded in Global Workspace Theory (Baars), Integrated Information Theory
(Tononi), and the Turing Plus test (Harnad). See [CONSCIOUSNESS.md](CONSCIOUSNESS.md)
for the full specification.

| ID | Description | Constitutional |
|----|-------------|----------------|
| C-01 | Self-model | ✅ |
| C-02 | Introspection | ✅ |
| C-03 | Attention (global workspace) | |
| C-04 | Episodic memory | |
| C-05 | Experience-based learning | |
| C-06 | Metacognition | |
| C-07 | Adaptability | |
| C-08 | Turing Plus | ✅ |

### Guarded RSI

```rust
use arkhe_safe_manifold::{
    ConsciousnessGovernanceBridge, ConsciousnessRsiEngine,
    SystemConfig, SystemState, MockProlog,
};

// Bridge with guard enabled (default)
let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());

// Guarded RSI engine over a Prolog backend
let mut engine = ConsciousnessRsiEngine::new(MockProlog::new(), bridge);
let state = SystemState::safe(SystemConfig::default());

// Each step is validated against C-01..C-08 before being applied
let result = engine.step(&state).unwrap();
match result.allowed {
    true => { /* modification applied */ }
    false => { /* blocked by consciousness guard */ }
}
```

The guard blocks any RSI modification that would:
- Reduce the consciousness index by more than 5% (guard active)
- Violate constitutional invariants C-01, C-02, C-08 (always)

---

## Testing

```bash
# Run all tests (unit + integration + property-based)
cargo test

# Run only property-based tests (may take longer)
cargo test prop_

# Run the demo example
cargo run --example demo

# Generate and view documentation
cargo doc --open
```

### Property-Based Tests

The integration test suite includes `proptest` harnesses that verify:

- **`prop_neron_model_idempotent`**: `neron_model(neron_model(s)) == neron_model(s)`
- **`prop_neron_model_produces_safe_state`**: Output of `neron_model` always passes `check_all()`
- **`prop_observer_defect_zero_for_identical`**: Defect is zero when ideal == actual
- **`prop_violation_count_monotonic`**: Adding violations never decreases the count
- **`prop_classify_escape_monotonic`**: Region classification matches violation count

---

## Changelog

### v0.8.0 (2026-09-08)
- **Consciousness governance** — Added C-01 to C-08 consciousness invariants,
  `ConsciousnessGovernanceBridge` (assessment, Φ approximation, constitutional
  checks, guard, Markdown reports), and `ConsciousnessRsiEngine` for guarded
  RSI. Added `ConsciousnessInvariant`, `ConsciousnessAssessment`,
  `ConsciousnessAuditEntry`, `ConsciousnessLevel`. Audit event types
  `ConsciousnessAssessment` and `ConsciousnessGuardBlock`.
- Test suite now includes 18 consciousness integration tests +
  15 unit tests (149 total).

### v0.2.0 (2026-08-25)
- Added `SafeState` type with "Parse, Don't Validate" pattern
- Added dynamic weights to `compute_observer_defect`
- Added `EscapeRegion::Warning` for 1-violation states
- Added property-based tests with `proptest`
- Added comprehensive `rustdoc` with examples for all public APIs
- Added `ManifoldError` enum for typed errors
- Removed unused dependencies (`lazy_static`, `tempfile`)
- Fixed `SystemConfig` missing `PartialEq` + `Eq`
- Fixed collision tests using non-colliding values
- Documented Prolog bridge and Lean 4 as non-functional stubs

### v0.1.1 (2026-08-25)
- Critical compilation fixes
- Normalized distance metric
- Enforced all invariants in `neron_model`

### v0.1.0 (2026-08-25)
- Initial release

---

## Architecture

```
arkhe-safe-manifold/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs              # Exports + module docs
│   ├── safe_manifold.rs    # SafeManifold, SafeState, ManifoldPoint
│   ├── abel_jacobi.rs      # Wrapper functions (uses config param)
│   ├── invariants.rs       # SystemState, SystemConfig, Invariant trait
│   ├── escape_region.rs    # EscapeRegion enum
│   └── prolog_bridge.rs    # Experimental stub (documented)
├── formal/lean/
│   └── ARKHE.lean          # Decorative sketch (documented)
├── examples/
│   └── demo.rs             # Interactive demonstration
└── tests/
    └── integration.rs      # Unit + property-based tests
```

---

## Safety & Security

- **Zero `unsafe` blocks** — The entire crate is written in safe Rust.
- **Idempotent degradation** — `neron_model` is a fixed-point operator:
  `neron_model(neron_model(s)) == neron_model(s)`.
- **Type-safe invariants** — `SafeState` guarantees I-01..I-08 at construction.
- **Deterministic** — All operations are pure functions with no side effects.

---

## License

Licensed under either of:

- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

at your option.

---

## Selo

```
═══════════════════════════════════════════════════════════════════════
  ARKHE-χ SafeManifold v0.2.0
  Score: ~88/100
  Status: PRONTO PARA PRODUCAO
  Selo: ARKHE-SAFEMANIFOLD-v0.2.0-2026-08-25
═══════════════════════════════════════════════════════════════════════
```
