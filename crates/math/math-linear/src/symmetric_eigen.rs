//! Deterministic eigendecomposition for finite real symmetric matrices.

use crate::{invalid_argument, F32Matrix, F64Matrix, F64MatrixView, MatrixShape};
use media_core::Result;
use numbers_core::checked_f64_to_f32;

const DEFAULT_MAX_SWEEPS: usize = 64;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
/// Options for deterministic symmetric eigendecomposition.
pub struct SymmetricEigenOptions {
    /// Absolute symmetry tolerance. When absent, a scale-aware tolerance is derived.
    pub symmetry_tolerance: Option<f64>,
    /// Absolute Jacobi convergence tolerance. When absent, a scale-aware tolerance is derived.
    pub convergence_tolerance: Option<f64>,
    /// Maximum cyclic Jacobi sweeps. When absent, a conservative default is used.
    pub max_sweeps: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
/// Real symmetric eigendecomposition with eigenvectors stored as columns.
pub struct SymmetricEigenDecomposition {
    /// Eigenvalues sorted in deterministic descending order.
    pub eigenvalues: Vec<f64>,
    /// Orthonormal eigenvectors stored as columns in the same order as `eigenvalues`.
    pub eigenvectors: F64Matrix,
    /// Number of Jacobi sweeps used.
    pub sweeps: usize,
    /// Resolved absolute symmetry tolerance.
    pub symmetry_tolerance: f64,
    /// Resolved absolute convergence tolerance.
    pub convergence_tolerance: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// Checked f32 compatibility result for symmetric eigendecomposition.
pub struct SymmetricEigenDecompositionF32 {
    /// Eigenvalues narrowed from the canonical f64 path with checked conversion.
    pub eigenvalues: Vec<f32>,
    /// Orthonormal eigenvectors narrowed from the canonical f64 path.
    pub eigenvectors: F32Matrix,
    /// Number of Jacobi sweeps used by the canonical f64 calculation.
    pub sweeps: usize,
}

impl SymmetricEigenDecomposition {
    /// Reconstructs `Q * diag(lambda) * Q^T`.
    pub fn reconstruct(&self) -> Result<F64Matrix> {
        let diagonal = F64Matrix::from_diag(&self.eigenvalues)?;
        self.eigenvectors
            .matmul(&diagonal.as_view())?
            .matmul(&self.eigenvectors.transpose_view())
    }
}

impl F64Matrix {
    /// Computes a pure-Rust cyclic Jacobi eigendecomposition for a symmetric matrix.
    pub fn symmetric_eigendecomposition(
        &self,
        options: SymmetricEigenOptions,
    ) -> Result<SymmetricEigenDecomposition> {
        self.as_view().symmetric_eigendecomposition(options)
    }
}

impl F64MatrixView<'_> {
    /// Computes a pure-Rust cyclic Jacobi eigendecomposition for a symmetric matrix.
    pub fn symmetric_eigendecomposition(
        &self,
        options: SymmetricEigenOptions,
    ) -> Result<SymmetricEigenDecomposition> {
        self.validate()?;
        if !self.is_square() {
            return Err(invalid_argument(
                "symmetric eigendecomposition requires a square matrix",
            ));
        }
        validate_options(options)?;

        let size = self.shape().rows;
        let max_abs = self
            .values()
            .iter()
            .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
        let scale = max_abs.max(1.0);
        let symmetry_tolerance = options
            .symmetry_tolerance
            .unwrap_or_else(|| derived_tolerance(size, scale));
        let convergence_tolerance = options
            .convergence_tolerance
            .unwrap_or_else(|| derived_tolerance(size, scale));
        let max_sweeps = options.max_sweeps.unwrap_or(DEFAULT_MAX_SWEEPS);

        let mut working = self.into_owned()?.into_values();
        validate_symmetry(&working, size, symmetry_tolerance)?;
        // Inputs accepted within tolerance are made exactly symmetric before Jacobi rotations.
        for row in 0..size {
            for col in (row + 1)..size {
                let average = (working[row * size + col] + working[col * size + row]) * 0.5;
                working[row * size + col] = average;
                working[col * size + row] = average;
            }
        }

        let mut eigenvectors = F64Matrix::identity(size)?.into_values();
        let mut sweeps = 0usize;
        let mut converged = size == 1;

        for sweep in 0..max_sweeps {
            sweeps = sweep + 1;
            let mut max_off_diagonal = 0.0_f64;
            let mut rotations = 0usize;

            for p in 0..size {
                for q in (p + 1)..size {
                    let apq = working[p * size + q];
                    max_off_diagonal = max_off_diagonal.max(apq.abs());
                    if apq.abs() <= convergence_tolerance {
                        continue;
                    }
                    jacobi_rotate(&mut working, &mut eigenvectors, size, p, q)?;
                    rotations += 1;
                }
            }

            if rotations == 0 || max_off_diagonal <= convergence_tolerance {
                converged = true;
                break;
            }
        }

        if !converged {
            let remaining = max_off_diagonal_abs(&working, size);
            if remaining > convergence_tolerance {
                return Err(invalid_argument(format!(
                    "symmetric eigendecomposition did not converge within {max_sweeps} sweeps; remaining off-diagonal magnitude {remaining} exceeds tolerance {convergence_tolerance}"
                )));
            }
        }

        let eigenvalues = (0..size)
            .map(|index| working[index * size + index])
            .collect::<Vec<_>>();
        let (eigenvalues, eigenvectors) =
            sort_and_canonicalize_eigenpairs(eigenvalues, eigenvectors, size);

        Ok(SymmetricEigenDecomposition {
            eigenvalues,
            eigenvectors: F64Matrix::new(MatrixShape::new(size, size)?, eigenvectors)?,
            sweeps,
            symmetry_tolerance,
            convergence_tolerance,
        })
    }
}

impl F32Matrix {
    /// Runs the canonical f64 symmetric eigendecomposition and narrows results safely.
    pub fn symmetric_eigendecomposition(
        &self,
        options: SymmetricEigenOptions,
    ) -> Result<SymmetricEigenDecompositionF32> {
        let promoted = F64Matrix::try_from(self)?;
        let decomposition = promoted.symmetric_eigendecomposition(options)?;
        let eigenvalues = decomposition
            .eigenvalues
            .iter()
            .map(|value| {
                checked_f64_to_f32(*value).ok_or_else(|| {
                    invalid_argument(
                        "symmetric eigendecomposition eigenvalue is outside the f32 range",
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let eigenvectors = F32Matrix::try_from(&decomposition.eigenvectors)?;
        Ok(SymmetricEigenDecompositionF32 {
            eigenvalues,
            eigenvectors,
            sweeps: decomposition.sweeps,
        })
    }
}

fn validate_options(options: SymmetricEigenOptions) -> Result<()> {
    for (name, tolerance) in [
        ("symmetry tolerance", options.symmetry_tolerance),
        ("convergence tolerance", options.convergence_tolerance),
    ] {
        if let Some(value) = tolerance {
            if !value.is_finite() || value < 0.0 {
                return Err(invalid_argument(format!(
                    "{name} must be finite and non-negative"
                )));
            }
        }
    }
    if matches!(options.max_sweeps, Some(0)) {
        return Err(invalid_argument(
            "symmetric eigendecomposition max_sweeps must be greater than zero",
        ));
    }
    Ok(())
}

fn derived_tolerance(size: usize, scale: f64) -> f64 {
    f64::EPSILON * size.max(1) as f64 * scale * 64.0
}

fn validate_symmetry(values: &[f64], size: usize, tolerance: f64) -> Result<()> {
    for row in 0..size {
        for col in (row + 1)..size {
            let difference = (values[row * size + col] - values[col * size + row]).abs();
            if difference > tolerance {
                return Err(invalid_argument(format!(
                    "matrix must be symmetric within tolerance {tolerance}; ({row}, {col}) differs by {difference}"
                )));
            }
        }
    }
    Ok(())
}

fn jacobi_rotate(
    values: &mut [f64],
    eigenvectors: &mut [f64],
    size: usize,
    p: usize,
    q: usize,
) -> Result<()> {
    let app = values[p * size + p];
    let aqq = values[q * size + q];
    let apq = values[p * size + q];
    if apq == 0.0 {
        return Ok(());
    }

    let tau = (aqq - app) / (2.0 * apq);
    let t = if tau >= 0.0 {
        1.0 / (tau + (1.0 + tau * tau).sqrt())
    } else {
        -1.0 / (-tau + (1.0 + tau * tau).sqrt())
    };
    let cosine = 1.0 / (1.0 + t * t).sqrt();
    let sine = t * cosine;
    if !cosine.is_finite() || !sine.is_finite() {
        return Err(invalid_argument(
            "symmetric eigendecomposition produced a non-finite Jacobi rotation",
        ));
    }

    for k in 0..size {
        if k == p || k == q {
            continue;
        }
        let akp = values[k * size + p];
        let akq = values[k * size + q];
        let new_kp = cosine * akp - sine * akq;
        let new_kq = sine * akp + cosine * akq;
        values[k * size + p] = new_kp;
        values[p * size + k] = new_kp;
        values[k * size + q] = new_kq;
        values[q * size + k] = new_kq;
    }
    values[p * size + p] = app - t * apq;
    values[q * size + q] = aqq + t * apq;
    values[p * size + q] = 0.0;
    values[q * size + p] = 0.0;

    for row in 0..size {
        let vkp = eigenvectors[row * size + p];
        let vkq = eigenvectors[row * size + q];
        eigenvectors[row * size + p] = cosine * vkp - sine * vkq;
        eigenvectors[row * size + q] = sine * vkp + cosine * vkq;
    }
    Ok(())
}

fn max_off_diagonal_abs(values: &[f64], size: usize) -> f64 {
    let mut maximum = 0.0_f64;
    for row in 0..size {
        for col in (row + 1)..size {
            maximum = maximum.max(values[row * size + col].abs());
        }
    }
    maximum
}

fn sort_and_canonicalize_eigenpairs(
    eigenvalues: Vec<f64>,
    eigenvectors: Vec<f64>,
    size: usize,
) -> (Vec<f64>, Vec<f64>) {
    let mut order = (0..size).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        eigenvalues[*right]
            .total_cmp(&eigenvalues[*left])
            .then_with(|| left.cmp(right))
    });

    let sorted_values = order.iter().map(|index| eigenvalues[*index]).collect();
    let mut sorted_vectors = vec![0.0; size * size];
    for (target_col, source_col) in order.into_iter().enumerate() {
        let mut pivot_row = 0usize;
        let mut pivot_abs = 0.0_f64;
        for row in 0..size {
            let value = eigenvectors[row * size + source_col];
            if value.abs() > pivot_abs {
                pivot_abs = value.abs();
                pivot_row = row;
            }
        }
        let sign = if eigenvectors[pivot_row * size + source_col] < 0.0 {
            -1.0
        } else {
            1.0
        };
        for row in 0..size {
            sorted_vectors[row * size + target_col] = eigenvectors[row * size + source_col] * sign;
        }
    }
    (sorted_values, sorted_vectors)
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

    fn assert_matrix_close(left: &F64Matrix, right: &F64Matrix, tolerance: f64) {
        assert_eq!(left.shape(), right.shape());
        for (left, right) in left.values().iter().zip(right.values()) {
            assert_close(*left, *right, tolerance);
        }
    }

    #[test]
    fn diagonal_matrix_sorts_eigenvalues_descending() {
        let matrix = F64Matrix::from_diag(&[1.0, 3.0, 2.0]).unwrap();
        let decomposition = matrix
            .symmetric_eigendecomposition(SymmetricEigenOptions::default())
            .unwrap();
        assert_eq!(decomposition.eigenvalues, vec![3.0, 2.0, 1.0]);
        assert_matrix_close(&decomposition.reconstruct().unwrap(), &matrix, 1.0e-12);
    }

    #[test]
    fn reconstructs_indefinite_matrix_and_has_orthonormal_vectors() {
        let matrix = F64Matrix::from_rows([[0.0, 1.0], [1.0, 0.0]]).unwrap();
        let decomposition = matrix
            .symmetric_eigendecomposition(SymmetricEigenOptions::default())
            .unwrap();
        assert_close(decomposition.eigenvalues[0], 1.0, 1.0e-12);
        assert_close(decomposition.eigenvalues[1], -1.0, 1.0e-12);
        assert_matrix_close(&decomposition.reconstruct().unwrap(), &matrix, 1.0e-12);

        let qtq = decomposition
            .eigenvectors
            .transpose_owned()
            .unwrap()
            .matmul(&decomposition.eigenvectors.as_view())
            .unwrap();
        assert_matrix_close(&qtq, &F64Matrix::identity(2).unwrap(), 1.0e-12);
    }

    #[test]
    fn covariance_like_matrix_has_expected_spectrum() {
        let matrix = F64Matrix::from_rows([[2.0, 1.0], [1.0, 2.0]]).unwrap();
        let decomposition = matrix
            .symmetric_eigendecomposition(SymmetricEigenOptions::default())
            .unwrap();
        assert_close(decomposition.eigenvalues[0], 3.0, 1.0e-12);
        assert_close(decomposition.eigenvalues[1], 1.0, 1.0e-12);
        assert_matrix_close(&decomposition.reconstruct().unwrap(), &matrix, 1.0e-12);
    }

    #[test]
    fn repeated_eigenvalues_are_deterministic() {
        let matrix = F64Matrix::identity(3).unwrap();
        let left = matrix
            .symmetric_eigendecomposition(SymmetricEigenOptions::default())
            .unwrap();
        let right = matrix
            .symmetric_eigendecomposition(SymmetricEigenOptions::default())
            .unwrap();
        assert_eq!(left, right);
        assert_eq!(left.eigenvalues, vec![1.0, 1.0, 1.0]);
    }

    #[test]
    fn accepts_tiny_asymmetry_only_with_explicit_tolerance() {
        let matrix = F64Matrix::from_rows([[2.0, 1.0 + 1.0e-9], [1.0, 3.0]]).unwrap();
        assert!(matrix
            .symmetric_eigendecomposition(SymmetricEigenOptions::default())
            .is_err());
        let decomposition = matrix
            .symmetric_eigendecomposition(SymmetricEigenOptions {
                symmetry_tolerance: Some(2.0e-9),
                ..SymmetricEigenOptions::default()
            })
            .unwrap();
        assert!(decomposition
            .eigenvalues
            .iter()
            .all(|value| value.is_finite()));
    }

    #[test]
    fn f32_entry_point_uses_checked_canonical_path() {
        let matrix = F32Matrix::from_rows([[2.0, 1.0], [1.0, 2.0]]).unwrap();
        let decomposition = matrix
            .symmetric_eigendecomposition(SymmetricEigenOptions::default())
            .unwrap();
        assert!((decomposition.eigenvalues[0] - 3.0).abs() < 1.0e-5);
        assert!((decomposition.eigenvalues[1] - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn rejects_invalid_options_and_shapes() {
        let rectangular = F64Matrix::from_rows([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]).unwrap();
        assert!(rectangular
            .symmetric_eigendecomposition(SymmetricEigenOptions::default())
            .is_err());
        let square = F64Matrix::identity(2).unwrap();
        assert!(square
            .symmetric_eigendecomposition(SymmetricEigenOptions {
                convergence_tolerance: Some(f64::NAN),
                ..SymmetricEigenOptions::default()
            })
            .is_err());
        assert!(square
            .symmetric_eigendecomposition(SymmetricEigenOptions {
                max_sweeps: Some(0),
                ..SymmetricEigenOptions::default()
            })
            .is_err());
    }

    #[cfg(feature = "nalgebra-backend")]
    #[test]
    fn eigenvalues_match_nalgebra_reference() {
        use nalgebra::{linalg::SymmetricEigen, DMatrix};

        let values = [4.0, 1.0, -2.0, 1.0, 2.0, 0.5, -2.0, 0.5, 3.0];
        let matrix = F64Matrix::new(MatrixShape::new(3, 3).unwrap(), values.to_vec()).unwrap();
        let ours = matrix
            .symmetric_eigendecomposition(SymmetricEigenOptions::default())
            .unwrap();
        let reference = SymmetricEigen::new(DMatrix::from_row_slice(3, 3, &values));
        let mut expected = reference.eigenvalues.iter().copied().collect::<Vec<_>>();
        expected.sort_by(|left, right| right.total_cmp(left));
        for (actual, expected) in ours.eigenvalues.iter().zip(expected) {
            assert_close(*actual, expected, 1.0e-10);
        }
    }
}
