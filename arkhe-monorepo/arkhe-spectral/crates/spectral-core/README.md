# spectral-core

Spectral analysis core for the ARKHE runtime — incremental SVD, DAG
Laplacian/Fiedler analysis, orthogonal (Procrustes) key alignment and
ML-DSA-65 wire-format guarding, built on `nalgebra 0.33`.

> **Design honesty:** this crate does **not** claim a spectral method is
> trustworthy just because an eigenvalue is separated by a tolerance. The
> Laplacian API exposes `is_fiedler_unique` (mathematical uniqueness) **and**
> `is_fiedler_reliable` (numerical stability against `ε_mach·λ_max`) and only
> returns a Fiedler vector when **both** hold — otherwise `None`, so callers
> never silently use a corrupted partition.

## Modules

| Module | What it provides |
|---|---|
| `svd` | `IncrementalSVD` — Brand (2006) + Zhang (2022) periodic re-orthogonalization; entropy (base 2), effective rank, `spectral_state()`, `reconstruct()` |
| `laplacian` | `build_symmetrized_laplacian`, `analyze_laplacian`, `fiedler_partition`, `check_dag_health`, `FIEDLER_RELIABILITY_KAPPA` |
| `procrustes` | `align_keys` (Kabsch, reflection handling), `key_compatibility_error` |
| `ml_dsa_guard` | `ml_dsa_65_offsets` — canonical FIPS 204 ML-DSA-65 wire offsets (c̃=48, z=3200, h=61, total 3309) |
| `error` | `SpectralError` |

## Laplacian analysis

```rust
use nalgebra::DMatrix;
use spectral_core::laplacian::{build_symmetrized_laplacian, analyze_laplacian, check_dag_health};

let lap = build_symmetrized_laplacian(5, &[(0,1),(1,2),(2,3),(3,4)]);
let a = analyze_laplacian(&lap).unwrap();

println!("λ₂ (algebraic connectivity): {}", a.algebraic_connectivity);
println!("unique: {}, reliable: {}", a.is_fiedler_unique, a.is_fiedler_reliable);

// fiedler_vector is Some ONLY when unique AND reliable
if let Some(v) = &a.fiedler_vector {
    println!("fiedler vector: {v}");
}
```

### Why `is_fiedler_reliable`?

`λ₂` being mathematically simple is necessary but not sufficient. On a
dumbbell `K₃-ε-K₃` with `ε=1e-9`, the solver returns a vector that is
numerically indistinguishable from the constant vector (`λ₂ ≈ ε_mach·λ_max`).
The criterion `λ₂ > FIEDLER_RELIABILITY_KAPPA · ε_mach · λ_max` (κ = 3e6)
separates the regimes:

| ε | λ₂ | λ₂/(ε_mach·λ_max) | Reliable |
|---|---|---|---|
| 1e-8 | ~6.7e-9 | ~1e7 | ✅ yes |
| 1e-9 | ~6.7e-10 | ~1e6 (boundary) | ❌ no |

Calibrated empirically; see `test_severe_bottleneck_api_reflects_numerical_reality`.

## Guarantees

- Depends only on `nalgebra`, `thiserror`, `serde` (+ `approx` in dev).
- No `unsafe` blocks.
- Where a result is numerically questionable, the API returns `None`/an
  explicit warning instead of fabricating a number.

## Example: DAG health

```rust
use spectral_core::laplacian::*;

let lap = build_symmetrized_laplacian(4, &[(0, 1), (2, 3)]); // disconnected
let a = analyze_laplacian(&lap).unwrap();
assert!(check_dag_health(&a, 0.1).is_some()); // low connectivity alert
```

## License

MIT OR Apache-2.0