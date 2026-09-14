# concept-pca

Conceptual coverage of an embedding space via PCA for the ARKHE runtime:
how many principal components explain a target fraction of variance, plus
mean pairwise cosine similarities between concepts.

## What it provides

`pca_concept_coverage(embeddings, concepts, variance_threshold)` returns a
`ConceptCoverage`:

| Field | Meaning |
|---|---|
| `explained_variance_ratio` | cumulative variance of the selected components (`= coverage`) |
| `n_components` | number of components needed to reach the threshold |
| `coverage` | same ratio — variance captured by the selected components |
| `concept_similarities` | per-concept mean cosine similarity vs. all others |

### Behaviour

- **Centering:** row-wise demeaning (per-feature sample mean) before SVD —
  `row_mean()` from `nalgebra 0.33` returns the 1×D row directly.
- **Degenerate case:** if total variance < 1e-12, returns coverage `0.0`
  with zero components.
- **Similarities:** cosine on the **original** embeddings (not the centered
  ones), clamped to `≥ 0`, skipping zero-norm vectors.

## Example

```rust
use nalgebra::DMatrix;
use concept_pca::pca_concept_coverage;

// 4 concepts in 3D, rows = embeddings
let embeddings = DMatrix::from_row_slice(4, 3, &[
    1.0, 0.0, 0.0,
    0.0, 1.0, 0.0,
    0.0, 0.0, 1.0,
    0.5, 0.5, 0.5,
]);
let concepts = vec!["x".into(), "y".into(), "z".into(), "mix".into()];

let cov = pca_concept_coverage(&embeddings, &concepts, 0.9).unwrap();
println!("coverage: {:.4}, components: {}", cov.coverage, cov.n_components);
for (name, sim) in &cov.concept_similarities {
    println!("{name}: {sim:.4}");
}
```

## Guarantees

- Depends only on `nalgebra` and `serde` (+ `approx` in dev).
- No `unsafe` blocks.
- Returns `Err` on empty input or on a mismatch between the number of
  embeddings and the number of concept names (no silent truncation).

## License

MIT OR Apache-2.0