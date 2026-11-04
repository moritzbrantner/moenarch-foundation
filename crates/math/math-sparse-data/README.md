# math-sparse-data

Sparse vector and matrix contracts for text, retrieval, and feature indexing.
This crate is part of the Analytical Math Crates family.

## Highlights

- Checked sparse vectors and COO/CSR matrix formats
- Canonicalization of unsorted indices
- Sparse dot and cosine similarity helpers
- Sparse vector norms, scaling, addition, Hadamard product, pruning, and top-k entries
- CSR row/column counts and sums, matrix summaries, row normalization,
  matrix-vector multiply, dense matrix multiply, COO/CSR transpose, and COO round trips
- Canonical CSR×CSR multiplication and sparse row/column Gram composition
- Dense and sparse conversion bridges

## Ownership boundary

This crate owns sparse representation, canonicalization, numerical validation, and the ordering contract for sparse values. Generic bounded top-k retention is delegated to `search-kernels` in `moritzbrantner/rust-kernels`; sparse value semantics remain local.

## Example

```rust,no_run
use math_linear::F32Matrix;
use math_sparse_data::{CooMatrix, SparseVector};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let vector = SparseVector::new(4, vec![0, 3], vec![1.0, 2.0])?.canonicalized()?;
    let matrix = CooMatrix::new(2, 4, vec![(0, 0, 1.0), (1, 3, 2.0)])?;
    let csr = matrix.to_csr()?;
    let dense = csr.to_dense_matrix()?;
    let right = F32Matrix::from_rows([[1.0], [2.0], [3.0], [4.0]])?;
    let product = csr.mul_dense_matrix(&right.as_view())?;
    assert_eq!(vector.to_dense(), vec![1.0, 0.0, 0.0, 2.0]);
    assert_eq!(matrix.nnz(), 2);
    assert_eq!(dense.shape().rows, 2);
    assert_eq!(product.shape().cols, 1);
    Ok(())
}
```

## Behavior

COO inputs are canonicalized before CSR construction, combining duplicate
coordinates and dropping exact zero stored values. Matrix summaries and dense
products are deterministic helpers for small and medium sparse feature
matrices. Dense matrix outputs use `math-linear::F32Matrix`, allowing sparse
feature workflows to move into linear algebra and statistics without adding an
external math backend.

CSR×CSR multiplication uses a row-local sparse accumulator rather than a dense output or dense row buffer. Output columns are sorted, duplicate contributions are combined, exact-zero cancellations are removed, and `SparseProductStats` exposes candidate scalar products plus output nnz as deterministic performance evidence.

### CSR×CSR backend decision (issue #118)

Decision: **keep the Foundation implementation as the only production path; use [`sprs` 0.11.6](https://docs.rs/sprs/0.11.6/sprs/) (MIT OR Apache-2.0, MSRV 1.88, `default-features = false`) as a dev-only differential oracle and benchmark candidate.** No backend selector or `sprs` type is added.

Differential evidence (`tests/sprs_differential.rs`): on canonical inputs both implementations accumulate every output entry in the same order, and the Foundation result is bit-identical to `sprs` (Bank & Douglas SMMP) once `sprs`' stored zeros are removed — across tiny, identity, empty/all-zero, rectangular, thin×wide, tall×short, dense-row, skewed-degree, transposed (`(AB)ᵀ = BᵀAᵀ`), and row/column Gram compositions. Non-canonical Foundation CSR input (unsorted or duplicate columns) is compared against the canonicalized oracle with a `1e-5` relative accumulation-order tolerance. `SparseProductStats` is checked against an independent structural count, because `sprs` has no such statistic.

Contract differences that a delegation would have to paper over:

- `sprs` keeps exact-zero cancellations as explicitly stored `0.0` entries; Foundation removes them.
- `sprs` propagates `inf`/`NaN`; Foundation rejects non-finite contributions and sums.
- `sprs` rejects unsorted/duplicate CSR columns that Foundation's `CsrMatrix` accepts, so delegation would need canonicalization first and would change result bits for such inputs.
- `sprs` allocates a dense `right.cols()` value and flag workspace per product. Foundation documents that it never materializes a dense output row; on the `thin_wide` workload (1,000,000 output columns) this is 5.2 MB against 96 KB, and at vocabulary-scale widths it would dominate memory.
- Both sides would need an O(nnz) copy into and out of `sprs`' owned storage.

Benchmark (`cargo bench --locked -p moenarch-math-sparse-data --bench csr_product -- --warm-up-time 1 --measurement-time 3`), Criterion median wall-clock per product, conversions outside the measured body:

| Workload | Foundation | sprs | Foundation / sprs |
| --- | --- | --- | --- |
| `tiny_4x4` | 499.9 ns | 477.5 ns | 1.05× |
| `highly_sparse_10k` | 1.26 ms | 592.25 µs | 2.12× |
| `thin_wide_16x4096_x_4096x1m` | 129.48 µs | 406.91 µs | 0.32× |
| `dense_rows_256` | 35.12 ms | 2.72 ms | 12.90× |
| `skewed_degree_5k` | 55.09 ms | 11.60 ms | 4.75× |
| `doc_term_gram_rows_2000x50k` | 13.23 ms | 4.61 ms | 2.87× |

Deterministic work evidence, printed by the same benchmark before timing (one product, counting global allocator; allocation counts are not latency):

| Workload | candidate products | output nnz | `sprs` stored zeros | Foundation allocs / bytes | `sprs` allocs / bytes |
| --- | --- | --- | --- | --- | --- |
| `tiny_4x4` | 12 | 10 | 0 | 11 / 952 | 17 / 652 |
| `highly_sparse_10k` | 39,993 | 39,988 | 3 | 10,031 / 3.09 MB | 17 / 1.17 MB |
| `thin_wide_16x4096_x_4096x1m` | 2,040 | 2,040 | 0 | 312 / 96 KB | 17 / 5.17 MB |
| `dense_rows_256` | 819,462 | 65,453 | 83 | 9,031 / 2.99 MB | 19 / 2.53 MB |
| `skewed_degree_5k` | 745,213 | 414,727 | 1,548 | 59,441 / 21.8 MB | 19 / 12.3 MB |
| `doc_term_gram_rows_2000x50k` | 207,879 | 128,112 | 13 | 17,347 / 5.85 MB | 17 / 3.50 MB |

`sprs` is 2–13× faster on square and dense-row shapes. The gap most likely comes from the per-row `BTreeMap` accumulator (ordered-tree inserts and node allocations per row, visible in the allocation counts) rather than from the O(candidate) finiteness checks; this was not profiled further. So the recorded follow-up is a first-party accumulator change bounded by each row's candidate count rather than by `right.cols()`; a hidden shape-based dispatch to `sprs` was rejected because it would reintroduce the dense workspace, require zero filtering, non-finite post-checks and input canonicalization, and maintain two code paths for one contract. Maintenance cost of keeping the current ~70-line implementation is low; `sprs` is mature (9.3M downloads, active releases) and remains the oracle that guards it.

Environment fingerprint: source `45ebb6f` plus this change, `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1`, `RUSTFLAGS` unset, release profile, Linux 6.18.44 x86_64 on a shared 4-vCPU Intel Xeon @ 2.10GHz VM with other workloads running (timings are indicative, not a gate; allocation and structural counts are deterministic for the committed generator).

## Package surface

Primary workflow: `sparse.similarity`.

Workflow operations:

- `sparse.similarity`: Computes sparse dot product or cosine similarity.
- `sparse.toDense`: Converts sparse vector coordinates into a dense f32 array.
- `sparse.matrixSummary`: Summarizes COO or CSR sparse matrix shape, nnz, density, and row nnz.
- `sparse.matrixStats`: Summarizes sparse matrix density, row/column nnz, row/column sums, and compact nnz statistics.
- `sparse.vectorOps`: Computes sparse vector norms, optional scaling, optional addition, and top-k entries.
- `sparse.matrixVector`: Multiplies a COO or CSR sparse matrix by a finite dense vector.
- `sparse.transpose`: Transposes a COO or CSR sparse matrix and returns canonical COO entries.

Debug operations:

- `describe`: inspect package metadata and runtime support.

Runtime support: library, CLI, server, and WASM wrappers expose these operations.

Run the primary workflow through the CLI:

```bash
cargo run -p moritzbrantner-math-sparse-data-cli -- run \
  --operation sparse.similarity \
  --json '{"left":{"dimensions":3,"indices":[0,2],"values":[1.0,2.0]},"metric":"dot","right":{"dimensions":3,"indices":[2],"values":[3.0]}}'
```

Successful responses use the shared package-surface shape with `operation`,
`title`, `message`, `summary`, and `result`. Default surface calls are
deterministic, local-first, and do not download models, write persistent files,
or execute external tools unless an operation explicitly documents native or
external-tool execution.

## Related crates

- `text-lexical`
- `text-embeddings`
- `vector-analysis-core`
