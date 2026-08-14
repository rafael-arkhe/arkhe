# lean_specs — Machine-verified band-isomorphism specs

Formal statements for the Arkhe hypergraph **band / seam model**, checked by
the Lean theorem prover (Lean 4.32.2, `lean` + `lake 5.0.0` via `elan`).

No Mathlib, no `omega`, no `native_decide` — every proof is discharged by
elementary equal-equality reasoning, so each file compiles with a single
`lean <file>` invocation.

## Files

| File | Statement | Verified |
|------|-----------|----------|
| `band_iso_core.lean` | Abstract `bandIso`: for setoids `Sx`, `Sd` and a seam rep `inj : D -> X`, `hcov` (every site is covered modulo `Sx`) + `hfaith` (restricted orbit relation is exactly `Sd`) yields a genuine `Iso (Quotient Sd) (Quotient Sx)` — no `sorry`. | `lean band_iso_core.lean` → exit 0 |
| `band_iso_lattice.lean` | Concrete instantiation on the finite lattice `D = X = Fin 3` with the discrete congruence. Both hypotheses discharged; produces `bandIsoLattice : Band.Iso (Quotient discrete) (Quotient discrete)`. | `lean band_iso_lattice.lean` → exit 0 |

## Why this matters (invariant impact)

- The band/seam is the gluing rule that makes a "Möbius-like" lattice site
  well-defined. The abstract lemma pins down exactly which two facts the
  construction needs (`hcov`, `hfaith`), and the concrete file shows those
  facts are *checkable*, not assumed.
- It replaces the earlier `sorry`-stubbed `.lean` files in this project with
  verified statements, consistent with the Simplicity/Integrity invariants.

## Reproducing

```sh
lean band_iso_core.lean     # exit 0
lean band_iso_lattice.lean  # exit 0
```

Toolchain: Lean 4.32.2 (x86_64-w64-windows-gnu), Lake 5.0.0-src+f3b06c7.
