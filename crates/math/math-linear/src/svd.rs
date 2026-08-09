use crate::{invalid_argument, F32Matrix, F64Matrix, F64MatrixView, MatrixShape};
use media_core::{DetectError, Result};

const DEFAULT_SVD_SWEEPS: usize = 64;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
/// Options for deterministic real-valued SVD.
pub struct SvdOptions {
    /// Absolute convergence tolerance. When absent, a scale-aware default is used.
    pub tolerance: Option<f64>,
    /// Maximum Jacobi sweeps. When absent, a conservative default is used.
    pub max_sweeps: Option<usize>,
    /// Whether to retain thin `u` and `vt` factors in the returned decomposition.
    pub compute_factors: bool,
    /// Optional maximum allowed `max(rows, cols)`.
    pub max_dimension: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
/// Options for Moore-Penrose pseudoinverse calculation.
pub struct PseudoinverseOptions {
    /// Absolute singular-value tolerance. When absent, a scale-aware default is used.
    pub tolerance: Option<f64>,
    /// Maximum Jacobi sweeps. When absent, the SVD default is used.
    pub max_sweeps: Option<usize>,
    /// Optional maximum allowed `max(rows, cols)`.
    pub max_dimension: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// SVD reconstruction diagnostics.
pub struct ReconstructionDiagnostics {
    /// Frobenius norm of `A - U * S * V^T`.
    pub residual_frobenius: f64,
    /// Residual divided by the input Frobenius norm when available.
    pub relative_residual: f64,
    /// Largest absolute reconstructed element error.
    pub max_abs_diff: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// Thin real-valued SVD decomposition and diagnostics.
pub struct SvdDecomposition {
    /// Singular values sorted descending.
    pub singular_values: Vec<f64>,
    /// Numerical rank under the resolved tolerance.
    pub rank: usize,
    /// Ratio of largest to smallest retained singular value when estimable.
    pub condition_estimate: Option<f64>,
    /// Optional thin left singular vectors, shape `rows x min(rows, cols)`.
    pub u: Option<F64Matrix>,
    /// Optional thin right singular vectors transposed, shape `min(rows, cols) x cols`.
    pub vt: Option<F64Matrix>,
    /// Jacobi sweeps used.
    pub sweeps: usize,
    /// Resolved convergence/rank tolerance.
    pub tolerance: f64,
    /// Reconstruction diagnostics computed from thin factors.
    pub reconstruction: ReconstructionDiagnostics,
}

impl F64Matrix {
    /// Computes a pure Rust one-sided Jacobi SVD for this finite real matrix.
    pub fn svd(&self, options: SvdOptions) -> Result<SvdDecomposition> {
        self.as_view().svd(options)
    }

    /// Computes the Moore-Penrose pseudoinverse from the SVD.
    pub fn pseudoinverse(&self, options: PseudoinverseOptions) -> Result<F64Matrix> {
        self.as_view().pseudoinverse(options)
    }

    /// Computes a singular-value based numerical rank.
    pub fn numerical_rank(&self, tolerance: Option<f64>) -> Result<usize> {
        self.as_view().numerical_rank(tolerance)
    }
}

impl F64MatrixView<'_> {
    /// Computes a pure Rust one-sided Jacobi SVD for this finite real matrix.
    pub fn svd(&self, options: SvdOptions) -> Result<SvdDecomposition> {
        self.validate()?;
        validate_svd_options(self.shape(), options)?;
        let rows = self.shape().rows;
        let cols = self.shape().cols;
        let thin = rows.min(cols);
        let max_sweeps = options.max_sweeps.unwrap_or(DEFAULT_SVD_SWEEPS);
        let input_norm = self.frobenius_norm()?;
        let tolerance = resolve_svd_tolerance(options.tolerance, rows, cols, input_norm)?;

        let mut working = self.into_owned()?.into_values();
        let mut v = F64Matrix::identity(cols)?.into_values();
        let mut sweeps = 0;
        let mut converged = false;

        for sweep in 0..max_sweeps {
            sweeps = sweep + 1;
            let mut max_off = 0.0_f64;
            let mut rotations = 0_usize;
            for p in 0..cols {
                for q in (p + 1)..cols {
                    let (alpha, beta, gamma) = column_pair_stats(&working, rows, cols, p, q);
                    max_off = max_off.max(gamma.abs());
                    let threshold = tolerance * alpha.sqrt().max(1.0) * beta.sqrt().max(1.0);
                    if gamma.abs() <= threshold {
                        continue;
                    }
                    let tau = (beta - alpha) / (2.0 * gamma);
                    let t = tau.signum() / (tau.abs() + (1.0 + tau * tau).sqrt());
                    let c = 1.0 / (1.0 + t * t).sqrt();
                    let s = c * t;
                    rotate_columns(&mut working, rows, cols, p, q, c, s);
                    rotate_columns(&mut v, cols, cols, p, q, c, s);
                    rotations += 1;
                }
            }
            if rotations == 0 || max_off <= tolerance {
                converged = true;
                break;
            }
        }

        let mut indexed = (0..cols)
            .map(|col| {
                let norm = column_norm(&working, rows, cols, col);
                (col, norm)
            })
            .collect::<Vec<_>>();
        indexed.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.0.cmp(&right.0))
        });

        let singular_values = indexed
            .iter()
            .take(thin)
            .map(|(_, value)| *value)
            .collect::<Vec<_>>();
        let rank_tolerance =
            resolve_rank_tolerance_from_singulars(options.tolerance, rows, cols, &singular_values)?;
        let rank = singular_values
            .iter()
            .filter(|value| **value > rank_tolerance)
            .count();
        let condition_estimate = singular_values
            .iter()
            .copied()
            .find(|value| *value > rank_tolerance)
            .and_then(|largest| {
                singular_values
                    .iter()
                    .rev()
                    .copied()
                    .find(|value| *value > rank_tolerance)
                    .map(|smallest| largest / smallest)
            });

        let (u_full, vt_full) = build_thin_factors(&working, &v, rows, cols, thin, &indexed)?;
        let reconstruction = reconstruction_diagnostics(
            *self,
            &singular_values,
            &u_full.as_view(),
            &vt_full.as_view(),
            input_norm,
        )?;
        if !converged {
            return Err(svd_non_convergence_error(
                self.shape(),
                tolerance,
                max_sweeps,
                reconstruction,
            ));
        }

        Ok(SvdDecomposition {
            singular_values,
            rank,
            condition_estimate,
            u: options.compute_factors.then_some(u_full),
            vt: options.compute_factors.then_some(vt_full),
            sweeps,
            tolerance,
            reconstruction,
        })
    }

    /// Computes the Moore-Penrose pseudoinverse from the SVD.
    pub fn pseudoinverse(&self, options: PseudoinverseOptions) -> Result<F64Matrix> {
        let svd = self.svd(SvdOptions {
            tolerance: options.tolerance,
            max_sweeps: options.max_sweeps,
            compute_factors: true,
            max_dimension: options.max_dimension,
        })?;
        let u = svd.u.as_ref().expect("factors requested");
        let vt = svd.vt.as_ref().expect("factors requested");
        let rows = self.shape().rows;
        let cols = self.shape().cols;
        let thin = rows.min(cols);
        let mut values = vec![0.0; cols * rows];
        let tolerance = resolve_rank_tolerance_from_singulars(
            options.tolerance,
            rows,
            cols,
            &svd.singular_values,
        )?;
        for component in 0..thin {
            let singular = svd.singular_values[component];
            if singular <= tolerance {
                continue;
            }
            let scale = 1.0 / singular;
            for row in 0..cols {
                let v_value = vt.as_view().get(component, row)?;
                for col in 0..rows {
                    let u_value = u.as_view().get(col, component)?;
                    values[row * rows + col] += v_value * scale * u_value;
                }
            }
        }
        F64Matrix::new(MatrixShape::new(cols, rows)?, values)
    }

    /// Computes a singular-value based numerical rank.
    pub fn numerical_rank(&self, tolerance: Option<f64>) -> Result<usize> {
        Ok(self
            .svd(SvdOptions {
                tolerance,
                compute_factors: false,
                ..SvdOptions::default()
            })?
            .rank)
    }
}

impl F32Matrix {
    /// Computes an f64 SVD by promoting this matrix without changing `F32Matrix`.
    pub fn svd(&self, options: SvdOptions) -> Result<SvdDecomposition> {
        F64Matrix::try_from(self)?.svd(options)
    }

    /// Computes an f64 pseudoinverse by promoting this matrix without changing `F32Matrix`.
    pub fn pseudoinverse(&self, options: PseudoinverseOptions) -> Result<F64Matrix> {
        F64Matrix::try_from(self)?.pseudoinverse(options)
    }

    /// Computes a singular-value based numerical rank after f64 promotion.
    pub fn numerical_rank(&self, tolerance: Option<f64>) -> Result<usize> {
        F64Matrix::try_from(self)?.numerical_rank(tolerance)
    }
}

fn validate_svd_options(shape: MatrixShape, options: SvdOptions) -> Result<()> {
    if let Some(max_dimension) = options.max_dimension {
        if shape.rows.max(shape.cols) > max_dimension {
            return Err(invalid_argument(format!(
                "SVD matrix max dimension {} exceeds limit {max_dimension}",
                shape.rows.max(shape.cols)
            )));
        }
    }
    if matches!(options.max_sweeps, Some(0)) {
        return Err(invalid_argument(
            "SVD max_sweeps must be greater than zero when provided",
        ));
    }
    if let Some(tolerance) = options.tolerance {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(invalid_argument(
                "SVD tolerance must be finite and non-negative",
            ));
        }
    }
    Ok(())
}

fn resolve_svd_tolerance(
    requested: Option<f64>,
    rows: usize,
    cols: usize,
    input_norm: f64,
) -> Result<f64> {
    if let Some(tolerance) = requested {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(invalid_argument(
                "SVD tolerance must be finite and non-negative",
            ));
        }
        return Ok(tolerance.max(f64::MIN_POSITIVE));
    }
    Ok(f64::EPSILON * rows.max(cols) as f64 * input_norm.max(1.0))
}

fn resolve_rank_tolerance_from_singulars(
    requested: Option<f64>,
    rows: usize,
    cols: usize,
    singular_values: &[f64],
) -> Result<f64> {
    if let Some(tolerance) = requested {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(invalid_argument(
                "rank tolerance must be finite and non-negative",
            ));
        }
        return Ok(tolerance);
    }
    let largest = singular_values.first().copied().unwrap_or(0.0).max(1.0);
    Ok(f64::EPSILON * rows.max(cols) as f64 * largest)
}

fn column_pair_stats(
    values: &[f64],
    rows: usize,
    cols: usize,
    p: usize,
    q: usize,
) -> (f64, f64, f64) {
    let mut alpha = 0.0;
    let mut beta = 0.0;
    let mut gamma = 0.0;
    for row in 0..rows {
        let left = values[row * cols + p];
        let right = values[row * cols + q];
        alpha += left * left;
        beta += right * right;
        gamma += left * right;
    }
    (alpha, beta, gamma)
}

fn column_norm(values: &[f64], rows: usize, cols: usize, col: usize) -> f64 {
    let mut sum = 0.0;
    for row in 0..rows {
        let value = values[row * cols + col];
        sum += value * value;
    }
    sum.sqrt()
}

fn rotate_columns(
    values: &mut [f64],
    rows: usize,
    cols: usize,
    p: usize,
    q: usize,
    c: f64,
    s: f64,
) {
    for row in 0..rows {
        let left = values[row * cols + p];
        let right = values[row * cols + q];
        values[row * cols + p] = c * left - s * right;
        values[row * cols + q] = s * left + c * right;
    }
}

fn build_thin_factors(
    working: &[f64],
    v: &[f64],
    rows: usize,
    cols: usize,
    thin: usize,
    indexed: &[(usize, f64)],
) -> Result<(F64Matrix, F64Matrix)> {
    let mut u_values = vec![0.0; rows * thin];
    let mut vt_values = vec![0.0; thin * cols];
    for component in 0..thin {
        let source_col = indexed[component].0;
        let singular = indexed[component].1;
        for row in 0..rows {
            u_values[row * thin + component] = if singular > f64::EPSILON {
                working[row * cols + source_col] / singular
            } else {
                0.0
            };
        }
        for col in 0..cols {
            vt_values[component * cols + col] = v[col * cols + source_col];
        }
    }
    Ok((
        F64Matrix::new(MatrixShape::new(rows, thin)?, u_values)?,
        F64Matrix::new(MatrixShape::new(thin, cols)?, vt_values)?,
    ))
}

fn reconstruction_diagnostics(
    original: F64MatrixView<'_>,
    singular_values: &[f64],
    u: &F64MatrixView<'_>,
    vt: &F64MatrixView<'_>,
    input_norm: f64,
) -> Result<ReconstructionDiagnostics> {
    let rows = original.shape().rows;
    let cols = original.shape().cols;
    let thin = singular_values.len();
    let mut residual_sum = 0.0;
    let mut max_abs_diff = 0.0;
    for row in 0..rows {
        for col in 0..cols {
            let mut reconstructed = 0.0;
            for (component, singular) in singular_values.iter().enumerate().take(thin) {
                reconstructed += u.get(row, component)? * singular * vt.get(component, col)?;
            }
            let diff = original.get(row, col)? - reconstructed;
            residual_sum += diff * diff;
            max_abs_diff = f64::max(max_abs_diff, diff.abs());
        }
    }
    let residual_frobenius = residual_sum.sqrt();
    let relative_residual = if input_norm > f64::EPSILON {
        residual_frobenius / input_norm
    } else {
        residual_frobenius
    };
    Ok(ReconstructionDiagnostics {
        residual_frobenius,
        relative_residual,
        max_abs_diff,
    })
}

fn svd_non_convergence_error(
    shape: MatrixShape,
    tolerance: f64,
    sweep_count: usize,
    diagnostics: ReconstructionDiagnostics,
) -> DetectError {
    invalid_argument(format!(
        "SVD did not converge for shape {}x{} with tolerance {tolerance:e} after {sweep_count} sweeps; residual_frobenius={:e}, relative_residual={:e}, max_abs_diff={:e}",
        shape.rows,
        shape.cols,
        diagnostics.residual_frobenius,
        diagnostics.relative_residual,
        diagnostics.max_abs_diff
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f64, right: f64, tolerance: f64) {
        assert!(
            (left - right).abs() <= tolerance,
            "expected {left} to be within {tolerance} of {right}"
        );
    }

    fn assert_identity(matrix: &F64Matrix, tolerance: f64) {
        let shape = matrix.shape();
        for row in 0..shape.rows {
            for col in 0..shape.cols {
                let expected = if row == col { 1.0 } else { 0.0 };
                assert_close(matrix.as_view().get(row, col).unwrap(), expected, tolerance);
            }
        }
    }

    #[test]
    fn f64_matrix_validates_and_converts() {
        let f32_matrix = F32Matrix::from_rows([[1.0, 2.0], [3.0, 4.0]]).unwrap();
        let f64_matrix = F64Matrix::try_from(&f32_matrix).unwrap();
        assert_eq!(f64_matrix.values(), &[1.0, 2.0, 3.0, 4.0]);
        let round_trip = F32Matrix::try_from(&f64_matrix).unwrap();
        assert_eq!(round_trip, f32_matrix);
        assert!(F64Matrix::new(MatrixShape::new(1, 1).unwrap(), vec![f64::INFINITY]).is_err());
    }

    #[test]
    fn svd_handles_square_tall_wide_and_diagonal() {
        for matrix in [
            F64Matrix::from_rows([[1.0, 0.0], [0.0, 2.0]]).unwrap(),
            F64Matrix::from_rows([[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]).unwrap(),
            F64Matrix::from_rows([[1.0, 0.0, 1.0], [0.0, 1.0, 1.0]]).unwrap(),
        ] {
            let svd = matrix
                .svd(SvdOptions {
                    compute_factors: true,
                    ..SvdOptions::default()
                })
                .unwrap();
            assert_eq!(svd.singular_values, {
                let mut values = svd.singular_values.clone();
                values.sort_by(|left, right| right.partial_cmp(left).unwrap());
                values
            });
            assert!(svd.reconstruction.relative_residual < 1.0e-10);
            assert_eq!(svd.u.as_ref().unwrap().shape().rows, matrix.shape().rows);
            assert_eq!(svd.vt.as_ref().unwrap().shape().cols, matrix.shape().cols);
        }
    }

    #[test]
    fn svd_reports_rank_and_condition() {
        let matrix = F64Matrix::from_rows([[1.0, 2.0], [2.0, 4.0], [3.0, 6.0]]).unwrap();
        let svd = matrix.svd(SvdOptions::default()).unwrap();
        assert_eq!(svd.rank, 1);
        assert_eq!(matrix.numerical_rank(Some(1.0e-8)).unwrap(), 1);

        let ill = F64Matrix::from_diag(&[1.0, 1.0e-8]).unwrap();
        let ill_svd = ill.svd(SvdOptions::default()).unwrap();
        assert!(ill_svd.condition_estimate.unwrap() > 1.0e7);
    }

    #[test]
    fn svd_factors_are_orthogonal_for_full_rank_columns() {
        let matrix = F64Matrix::from_rows([[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]).unwrap();
        let svd = matrix
            .svd(SvdOptions {
                compute_factors: true,
                ..SvdOptions::default()
            })
            .unwrap();
        let u = svd.u.as_ref().unwrap();
        let vt = svd.vt.as_ref().unwrap();
        let u_t_u = u.transpose_owned().unwrap().matmul(&u.as_view()).unwrap();
        let v_t_v = vt.matmul(&vt.transpose_view()).unwrap();
        assert_identity(&u_t_u, 1.0e-10);
        assert_identity(&v_t_v, 1.0e-10);
    }

    #[test]
    fn pseudoinverse_satisfies_moore_penrose_identity() {
        let matrix = F64Matrix::from_rows([[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]).unwrap();
        let pinv = matrix
            .pseudoinverse(PseudoinverseOptions::default())
            .unwrap();
        let reconstructed = matrix
            .matmul(&pinv.as_view())
            .unwrap()
            .matmul(&matrix.as_view())
            .unwrap();
        for (actual, expected) in reconstructed.values().iter().zip(matrix.values()) {
            assert_close(*actual, *expected, 1.0e-9);
        }
    }

    #[test]
    fn svd_rejects_size_cap_and_zero_sweeps() {
        let matrix = F64Matrix::identity(2).unwrap();
        assert!(matrix
            .svd(SvdOptions {
                max_dimension: Some(1),
                ..SvdOptions::default()
            })
            .is_err());
        assert!(matrix
            .svd(SvdOptions {
                max_sweeps: Some(0),
                ..SvdOptions::default()
            })
            .is_err());
    }
}
