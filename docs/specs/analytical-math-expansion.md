# Analytical math expansion evidence

Status: integrated evidence for [#59](https://github.com/moritzbrantner/moenarch-foundation/issues/59) and [#64](https://github.com/moritzbrantner/moenarch-foundation/issues/64).

This wave extends the established numerical ownership map without changing its layering or introducing a universal scalar abstraction:

`numeric contracts -> vectors/tensors/matrices/geometry -> statistics/algorithms -> domain capabilities`

## Added capability seams

| Capability | Canonical owner | Deterministic contract | Cost shape | Independent evidence |
| --- | --- | --- | --- | --- |
| Probability evaluation | `moenarch-math-probability` | validated Bernoulli, uniform, normal, binomial, and Poisson PMF/PDF/CDF/log-probability plus moments; no RNG or sampling policy | constant-time elementary evaluations; discrete CDFs iterate over the required finite support/tail terms | exact identities and published high-precision scalar fixtures; mass/CDF property tests |
| Lagged statistics | `moenarch-math-statistics` | positive lag pairs `left[t]` with `right[t + lag]`; overlap-local centering; explicit population/sample overlap rules | O(n) per requested lag; O(n * max_lag) for the simple series helpers | lag-zero equivalence to existing covariance/correlation plus direct deterministic fixtures |
| Symmetric spectral decomposition | `moenarch-math-linear` | pure-Rust cyclic Jacobi decomposition; explicit/scale-derived symmetry and convergence tolerances; descending eigenvalues and deterministic vector-sign normalization | O(n^3) work for dense n×n matrices, bounded by the configured sweep count | reconstruction/orthogonality/eigenpair laws plus feature-gated `nalgebra` eigenvalue parity |
| Sparse matrix composition | `moenarch-math-sparse-data` | CSR×CSR with row-local sparse accumulation, sorted canonical output, duplicate accumulation, exact-zero elision | proportional to compatible stored-entry products plus row-local output accumulation; no dense rows×cols buffer | differential comparison with sparse→dense `math-linear` multiplication |

## Cross-crate canaries

`crates/math/math-probability/tests/expanded_math_interop.rs` exercises the seams together rather than merely repeating crate-local unit tests. The canary lives in the new, still-unpublished probability package so already-publishable math packages do not acquire a dev-dependency on an unavailable registry version.

1. An explicit Bernoulli fixture is summarized by `math-statistics` and its population mean/variance must equal the distribution's analytical moments. No sampler or hidden RNG is involved.
2. A shifted paired fixture verifies the positive-lag direction used by cross-correlation.
3. A covariance matrix produced by `RunningCovariance` is promoted to f64, decomposed by the symmetric eigensolver, reconstructed, and checked as positive semidefinite within numerical tolerance.
4. A sparse CSR product is compared with the dense `math-linear` oracle, then summarized through `math-statistics` to prove finite-value and shape interoperability.

## Observable work evidence

Correctness gates intentionally avoid wall-clock thresholds. Two new APIs expose structural work that deterministic tests can reason about:

- `SparseProductStats::candidate_products` reports scalar products actually considered; `output_nnz` reports canonical stored output entries. The integration canary compares this work with the equivalent dense candidate count on a fixed sparse fixture.
- `SymmetricEigenDecomposition::sweeps` reports Jacobi sweeps. The integration canary verifies convergence within the documented default bound while reconstruction remains the correctness criterion.

These are diagnostic/work counters, not promises that one machine must finish within a fixed duration.

## Numerical boundaries

- `math-probability` rejects invalid/non-finite parameters and finite continuous evaluation-point violations with typed foundation errors. Log probability may be negative infinity only for mathematically zero probability/density.
- `math-statistics` retains `f64` scalar-series semantics. Lagged operations reuse the existing finite-series/covariance/correlation contracts instead of introducing a second validation policy.
- `math-linear` remains f64-first for spectral decomposition. The f32 entry point promotes to the canonical f64 implementation and narrows through checked conversion.
- `math-sparse-data` remains finite `f32` sparse storage. Matrix products reject non-finite intermediate or accumulated values rather than silently storing them.

No third-party probability, matrix, or sparse backend type is exposed publicly. `nalgebra` remains a reference/test path. No RNG engine, general forecasting framework, nonsymmetric eigensolver, sparse solver, or optimization framework was introduced.

## Release boundary

This evidence is source-development evidence only. The analytical expansion does not authorize publication, tags, releases, or downstream version changes. `moenarch-math-probability` is registered as a foundation-owned post-extraction package with publication disabled until a separate release task explicitly authorizes it.
