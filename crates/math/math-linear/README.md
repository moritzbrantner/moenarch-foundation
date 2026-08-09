# math-linear

Dense matrix and kernel contracts bridging `moritzbrantner-tensor-data` and
`moritzbrantner-vector-analysis-core`.

## Highlights

- Checked small dense matrix shapes and views
- Parallel finite `F32Matrix` and `F64Matrix` row-major matrix types
- Row and column iteration with transpose views
- Identity, zero, transpose, add, subtract, scale, trace, and mean utilities
- Diagonal construction, Gram matrices, and row/column centering
- Matrix multiply, matrix-vector multiply, and row cosine helpers
- Tolerance-aware rank estimates and QR-based least-squares fits
- Pure Rust real-valued SVD, pseudoinverse, and singular-value numerical rank
- Pure Rust LU decomposition with partial pivoting, determinant, solve, and
  inverse helpers
- Pure Rust Cholesky and modified Gram-Schmidt QR decomposition for
  deterministic small and medium matrix workflows
- Shared `Kernel2d` and `Kernel1d` types for image and video processing
- Bridges between rank-2 tensors, dense vectors, and matrices

## Example

```rust,no_run
use math_linear::{F32Matrix, Kernel2d};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matrix = F32Matrix::from_rows([[2.0, 1.0], [1.0, 3.0]])?;
    let product = matrix.matmul(&matrix.as_view())?;
    let solution = matrix.solve_vector(&[1.0, 2.0])?;
    let inverse = matrix.inverse()?;
    let design = F32Matrix::from_rows([[1.0, 1.0], [1.0, 2.0], [1.0, 3.0]])?;
    let least_squares = design.least_squares(&[3.0, 5.0, 7.0], 0.0)?;
    let kernel = Kernel2d::sharpen_3x3();
    assert_eq!(product.shape().rows, 2);
    assert_eq!(solution.len(), 2);
    assert_eq!(inverse.shape().cols, 2);
    assert_eq!(least_squares.coefficients.len(), 2);
    assert_eq!(kernel.as_array_3x3()?, [0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0]);
    Ok(())
}
```

## Behavior

`MatrixShape` rejects zero rows or columns and computes element counts with
checked multiplication. `F32Matrix` stores values in row-major order. A
transposed `F32MatrixView` swaps shape and layout metadata without copying the
underlying values.

Matrix construction and validation require the value count to match
`rows * cols`, and every value must be finite. Matrix/vector multiplication also
requires a finite vector whose length matches the matrix column count.

Owned utility results are emitted as row-major `F32Matrix` values. Transpose
views do not copy, while `transpose_owned()` writes the logical transpose into a
new row-major buffer. Elementwise add, subtract, and scale validate shapes and
reject non-finite scale factors before producing owned output.

Matrix multiplication requires `left.cols == right.rows`. Pairwise row dot and
pairwise row cosine require both inputs to have the same column count and return
a matrix shaped `left.rows x right.rows`.

LU decomposition is implemented in pure Rust for deterministic small and
medium-size matrix workflows. It uses partial pivoting, rejects non-square and
singular or near-singular matrices, and powers determinant, vector solve, matrix
solve, and inverse helpers. This crate owns deterministic Analytical Math
Crates matrix primitives for workspace packages, not a user-selectable
numerical backend layer.

Cholesky decomposition requires a symmetric positive definite square matrix. QR
decomposition currently computes a thin factorization and requires
`rows >= cols` with full column rank.

Least-squares fitting is QR-based and deterministic for small and medium local
matrices. It requires full column rank, rejects non-finite inputs and invalid
tolerances, and treats `tolerance == 0.0` as an automatic tolerance derived from
matrix size and maximum column L2 norm.

SVD-class operations promote f32 callers to f64 by default and use a pure Rust
real-valued Jacobi path. Package surfaces cap `max(rows, cols)` at 512, return
compact singular values, rank, condition, and reconstruction diagnostics by
default, and include thin `u`/`vt` factors only when requested. `faer` and
`nalgebra` are hidden feature-gated reference and benchmark paths, not runtime
selection options.

Row and column L2 normalization use `vector-analysis-core` normalization rules.
Rows or columns with an effectively zero norm return an error instead of
producing non-finite values.

`Kernel2d` values are stored in row-major order and are not normalized
automatically. For example, `Kernel2d::blur_3x3()` contains nine `1.0`
coefficients; callers can scale the output or coefficients when they need an
averaging blur.

Rank-2 `tensor-data::F32Tensor` values can be converted to matrices, and
matrices can be converted back to tensors with shape `[rows, cols]`.

## Package surface

Primary workflow: `linear.matmul`.

Workflow operations:

- `linear.matmul`: Multiplies two finite f32 row-major matrices.
- `linear.transpose`: Returns a row-major owned transpose of a finite f32 matrix.
- `linear.solve`: Solves a square finite f32 matrix against a vector or matrix right-hand side.
- `linear.decompose`: Decomposes a square finite f32 matrix with partial-pivoted LU.
- `linear.inverse`: Returns the inverse of a finite square f32 matrix.
- `linear.kernel1d`: Validates and optionally normalizes a finite 1D f32 kernel.
- `linear.tensorBridge`: Projects rank-2 tensor payloads to matrix shape or matrix-shaped payloads to tensor shape.
- `linear.gram`: Computes row or column Gram matrices for a finite f32 matrix.
- `linear.cholesky`: Factors a symmetric positive definite matrix into lower-triangular Cholesky form.
- `linear.qr`: Factors a full-column-rank matrix with deterministic modified Gram-Schmidt QR.
- `linear.center`: Subtracts row or column means from a finite f32 matrix.
- `linear.leastSquares`: Fits a full-column-rank QR least-squares model for a finite f32 design matrix.
- `linear.svd`: Computes compact SVD diagnostics for a finite real matrix, defaulting to f64 precision.
- `linear.pseudoinverse`: Computes a Moore-Penrose pseudoinverse from the SVD path.
- `linear.rank`: Computes singular-value numerical rank for a finite real matrix.

Debug operations:

- `describe`: inspect package metadata and runtime support.

Runtime support: library, CLI, server, and WASM wrappers expose these operations.

Run the primary workflow through the CLI:

```bash
cargo run -p moritzbrantner-math-linear-cli -- run \
  --operation linear.matmul \
  --json '{"left":{"cols":2,"rows":2,"values":[1.0,2.0,3.0,4.0]},"right":{"cols":1,"rows":2,"values":[5.0,6.0]}}'
```

Successful responses use the shared package-surface shape with `operation`,
`title`, `message`, `summary`, and `result`. Default surface calls are
deterministic, local-first, and do not download models, write persistent files,
or execute external tools unless an operation explicitly documents native or
external-tool execution.

## Related crates

- `tensor-data`
- `vector-analysis-core`
- `image-analysis-processing`
