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
- Dense and sparse conversion bridges

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
