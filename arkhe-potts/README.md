# arkhe-potts

A small, honest Potts-model core in `std`-only Rust: **energy**, exact
**Boltzmann statistics**, and **MAP inference** over discrete configurations.

This is the piece the *EffieDes* section of the "Arkhe + Protein Design"
document gestured at but never actually built. That Lean sketch had a fatal
bug — its `potts_energy` never referenced the configuration `σ`, so the energy
of a sequence did not depend on the sequence, and the MAP/probability code on
top of it was meaningless. Here the energy genuinely depends on every position,
and correctness is **verified against a ground-truth oracle** rather than
asserted.

## Energy convention

For `σ = (σ_0, …, σ_{n-1})`, each `σ_i ∈ {0, …, q-1}`:

```
E(σ) = - Σ_i h_i(σ_i)  -  Σ_{i<j} J_{ij}(σ_i, σ_j)
```

`P(σ) = exp(-β E(σ)) / Z`. Lower energy ⇒ higher probability, so MAP inference
is `argmin_σ E(σ)`.

## What's real here

| Method | What it does | How it's trusted |
| --- | --- | --- |
| `energy` | Fields + pairwise couplings for the actual `σ` | Hand-computed on a 2×2 model |
| `partition_function` / `probability` | Exact `Z` and `P(σ)` by enumeration | Probabilities sum to 1; `Z` matches a manual sum |
| `map_exact` / `map_exact_constrained` | Exhaustive `argmin E`, the ground-truth oracle | Cross-checked against an independent enumeration |
| `map_icm` / `map_icm_restarts` | Iterated Conditional Modes — scalable heuristic | Monotone descent; recovers `map_exact` over 50 random instances |
| `Constraint::{Fixed,Equal,NotEqual}` | Hard constraints (e.g. symmetry `σ_0 == σ_3`) | Feasibility + infeasible-set detection tested |

Exhaustive methods refuse to run above `q^n = 50_000_000` (returning
`StateSpaceTooLarge`) instead of hanging; the heuristic still runs at any size.
The heuristic's randomness is a seeded SplitMix64 generator, so
`map_icm_restarts(seed, restarts)` is fully deterministic — no `rand`
dependency.

## What it is *not*

No amino-acid alphabet, no `toulbar2` binding, no BLOSUM62, no secretion
numbers. Those were placeholders in the source document (hardcoded constants
dressed as results). This crate is the honest mathematical core they all
assumed: a correct Potts energy and a MAP solver you can trust because a
brute-force oracle checks it. Wiring a real sequence alphabet or an external
weighted-CSP solver on top is a separate, later step.

## Run

```bash
cargo test
```
