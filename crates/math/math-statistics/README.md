# math-statistics

Shared scalar, pairwise, rolling, multivariate, and matrix statistics for
finite local inputs.

## Highlights

- Streaming covariance accumulation
- Scalar series summaries, sample/population variance, and z-scores
- Pairwise covariance and correlation
- Average ranks, Spearman correlation, simple linear regression, and OLS
  regression over dense design matrices
- OLS diagnostics, deterministic ridge regression, and row-wise covariance and
  correlation matrices
- Difference, relative-change, and log-ratio derived series
- Rolling mean, standard deviation, min/max ranges, and correlation
- Empirical tail risk and compounded-path drawdown helpers
- Z-score and min/max normalizers
- Dense covariance matrix generation
- f64-default matrix package operations with explicit f32 compatibility
- Centered-data SVD PCA for package workflows

## Example

```rust,no_run
use math_linear::F32Matrix;
use math_statistics::{changes, ordinary_least_squares_diagnostics, summarize_series, ChangeMethod, PrincipalComponents, RunningCovariance, VarianceMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let returns = changes(&[100.0, 102.0, 99.0], ChangeMethod::Relative)?;
    let summary = summarize_series(&returns, VarianceMode::Sample)?;

    let matrix = F32Matrix::from_rows([[1.0, 0.0], [0.0, 1.0], [1.0, 1.0]])?;
    let covariance = RunningCovariance::from_matrix(&matrix.as_view())?.covariance_matrix()?;
    let pca = PrincipalComponents::fit(&matrix.as_view(), 1)?;
    let design = F32Matrix::from_rows([[1.0, 1.0], [1.0, 2.0], [1.0, 3.0], [1.0, 4.0]])?;
    let diagnostics = ordinary_least_squares_diagnostics(&design.as_view(), &[3.0, 5.0, 7.0, 9.0], 0.0)?;
    assert_eq!(summary.count, 2);
    assert_eq!(covariance.matrix.shape().rows, 2);
    assert_eq!(pca.components().shape().rows, 1);
    assert_eq!(diagnostics.degrees_of_freedom, 2);
    Ok(())
}
```

## Behavior

Scalar series helpers reject empty or non-finite inputs. Sample variance uses
`n - 1` and requires at least two observations; population variance uses `n`.
Relative changes and log-ratios require strictly positive adjacent values.
Drawdown and compounded-return helpers require period values greater than or
equal to `-1.0`.

`WeightedObservation` requires at least one finite value and a positive finite
weight. `RunningCovariance` has a fixed dimensionality; every pushed
observation must match it. `count` is the number of observations pushed, while
`weight_sum` is the sum of their weights.

Covariance is reported as weighted population covariance: accumulated second
moments are divided by `weight_sum`, not by `count - 1`. `from_matrix` treats
each matrix row as one unit-weight observation.

`CovarianceMatrix::correlation_matrix` divides each covariance cell by the
product of the corresponding standard deviations. Degenerate variance terms are
clamped with `f32::EPSILON` in the denominator so the transform remains finite.

`ZScoreNormalizer` fits per-column means and standard deviations from matrix
rows. Constant columns use an epsilon-scale standard deviation, so transforming
the same fitted constant values yields zero rather than `NaN`.

`MinMaxNormalizer` fits per-column ranges and delegates normalization to
`numbers-core::NumberRange`. Degenerate ranges normalize the exact range value
to `0.0`.

`PrincipalComponents` remains a small deterministic Rust helper for existing
library callers. Package-surface PCA uses centered-data SVD from `math-linear`
and returns explained variance, explained variance ratio, singular values,
components, and optional transformed rows.

OLS uses the `math-linear` QR path for full-column-rank library calls and uses
SVD pseudoinverse when a rank-deficient design needs a package-surface OLS
result. OLS diagnostics remain strict and require identifiable full-column-rank
designs.

Ridge regression is a deterministic regularized normal-equation helper for
small and medium local matrices: it solves `(X^T X + lambda I) beta = X^T y`
with finite, non-negative `lambda`. It is not a full optimizer suite.

## Package surface

Primary workflow: `stats.series.describe`.

Workflow operations:

- `stats.series.describe`: Computes summary statistics for a finite scalar series.
- `stats.series.changes`: Converts adjacent values into differences, relative changes, or log-ratios.
- `stats.series.compare`: Computes pairwise covariance and correlation for two finite scalar series.
- `stats.series.rolling`: Computes rolling mean, standard deviation, ranges, and optional paired correlations.
- `stats.series.tailRisk`: Computes empirical VaR/CVaR style tail-risk statistics.
- `stats.series.zScores`: Standardizes a finite scalar series into z-scores.
- `stats.normalize`: Applies z-score or min-max column normalization to a finite matrix, defaulting to f64 precision.
- `stats.covariance`: Computes covariance and optional correlation for matrix rows as observations, defaulting to f64 precision.
- `stats.pca`: Fits centered-data SVD PCA components and optionally transforms rows into component space.
- `stats.series.rankCorrelation`: Computes average ranks and Spearman correlation for finite paired scalar series.
- `stats.regression.linear`: Fits a simple y = intercept + slope * x regression for paired finite scalar observations.
- `stats.regression.ols`: Fits ordinary least squares from a finite dense design matrix and target vector, using pseudoinverse when needed.
- `stats.regression.diagnostics`: Fits strict full-column-rank OLS and returns residual, adjusted R-squared, standard-error, and t-statistic diagnostics.

Debug operations:

- `describe`: inspect package metadata and runtime support.

Runtime support: library, CLI, server, and WASM wrappers expose these operations.

Run the primary workflow through the CLI:

```bash
cargo run -p moritzbrantner-math-statistics-cli -- run \
  --operation stats.series.describe \
  --json '{"values":[1.0,2.0,3.0,4.0],"varianceMode":"sample"}'
```

Successful responses use the shared package-surface shape with `operation`,
`title`, `message`, `summary`, and `result`. Default surface calls are
deterministic, local-first, and do not download models, write persistent files,
or execute external tools unless an operation explicitly documents native or
external-tool execution.

## Related crates

- `dense-data`
- `video-analysis-features`
- `text-embeddings`
