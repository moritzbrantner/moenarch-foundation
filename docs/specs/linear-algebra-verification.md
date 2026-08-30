# Linear algebra verification contract

F3 verifies the existing pure-Rust `math-linear` implementation through its
public matrix, tensor, and vector interfaces. Benchmarks remain Criterion
scenarios and are not part of correctness gates.

## Property suite

`tests/linear_laws.rs` uses F1's deterministic seed and F2's explicit
`ApproxTolerance` values. It verifies:

- f32 transpose involution, identity multiplication, distributivity, Gram
  symmetry, and matrix/vector consistency;
- rank-two tensor/matrix round trips;
- LU permutation reconstruction, determinant agreement, solve residuals, and
  inverse residuals;
- QR reconstruction and orthogonality, plus Cholesky reconstruction;
- f64 SVD reconstruction and all four Moore-Penrose pseudoinverse identities.

The tolerances are test evidence, not new production defaults: `2e-4` absolute
or relative for small f32 generated cases and `1e-8` for f64 SVD/pseudoinverse
residuals. The algorithms retain their operation-specific scale-aware rank/SVD
thresholds and LU pivot threshold. F3 reviewed the existing `EPSILON` uses and
does not replace them with one global epsilon because they represent distinct
algorithmic decisions.

## Adversarial behavior

Singular and near-singular f32 LU inputs deterministically return their typed
invalid-argument rejection. Small and large finite f64 diagonal inputs run
through SVD; the ill-conditioned diagonal example has numerical rank two under
the current scale-aware default. Rectangular matrices are generated in the law
and reference suites. Non-finite construction remains rejected by F1/F2 tests.

## Independent agreement

`numerical_verification.rs`, run with `nalgebra-backend`, compares generated
matrix products against `nalgebra::DMatrix` after converting to foundation
row-major values. The optional backend stays test-only; no backend type enters
the canonical public interface.
