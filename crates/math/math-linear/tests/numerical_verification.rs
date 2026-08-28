#[path = "../../../test-support/numerical.rs"]
mod numerical;

use math_linear::{F64Matrix, MatrixShape};
use numerical::{
    assert_approx_eq_f64, assert_matrix_approx_eq_f64, compatible_matrix_pair_f64,
    deterministic_config, finite_matrix_f64, non_finite_f64, ApproxTolerance,
};
use proptest::prelude::*;

fn strict_tolerance() -> ApproxTolerance {
    ApproxTolerance::new(1.0e-12, 1.0e-12).unwrap()
}

proptest! {
    #![proptest_config(deterministic_config())]

    #[test]
    fn owned_transpose_is_an_involution(matrix in finite_matrix_f64(6, 6)) {
        let original = F64Matrix::new(
            MatrixShape::new(matrix.rows, matrix.cols).unwrap(),
            matrix.values.clone(),
        ).unwrap();
        let twice_transposed = original.transpose_owned().unwrap().transpose_owned().unwrap();

        prop_assert_eq!(twice_transposed.shape(), original.shape());
        prop_assert_eq!(twice_transposed.values(), original.values());
    }

    #[test]
    fn matrix_multiplication_matches_independent_reference(
        (left, right) in compatible_matrix_pair_f64(5, 5, 5),
    ) {
        let left_matrix = F64Matrix::new(
            MatrixShape::new(left.rows, left.cols).unwrap(),
            left.values.clone(),
        ).unwrap();
        let right_matrix = F64Matrix::new(
            MatrixShape::new(right.rows, right.cols).unwrap(),
            right.values.clone(),
        ).unwrap();
        let actual = left_matrix.matmul(&right_matrix.as_view()).unwrap();

        let expected = reference_matmul(&left, &right);
        prop_assert_eq!(actual.shape().rows, left.rows);
        prop_assert_eq!(actual.shape().cols, right.cols);
        assert_matrix_approx_eq_f64(
            actual.values(),
            &expected,
            left.rows,
            right.cols,
            strict_tolerance(),
        );
    }

    #[test]
    fn non_finite_matrix_values_are_rejected(value in non_finite_f64()) {
        let result = F64Matrix::new(MatrixShape::new(1, 1).unwrap(), vec![value]);
        prop_assert!(result.is_err());
    }
}

#[cfg(feature = "nalgebra-backend")]
proptest! {
    #![proptest_config(deterministic_config())]

    #[test]
    fn matrix_multiplication_matches_nalgebra(
        (left, right) in compatible_matrix_pair_f64(5, 5, 5),
    ) {
        let left_matrix = F64Matrix::new(
            MatrixShape::new(left.rows, left.cols).unwrap(),
            left.values.clone(),
        ).unwrap();
        let right_matrix = F64Matrix::new(
            MatrixShape::new(right.rows, right.cols).unwrap(),
            right.values.clone(),
        ).unwrap();
        let actual = left_matrix.matmul(&right_matrix.as_view()).unwrap();

        let reference_left = nalgebra::DMatrix::from_row_slice(left.rows, left.cols, &left.values);
        let reference_right = nalgebra::DMatrix::from_row_slice(
            right.rows,
            right.cols,
            &right.values,
        );
        let reference = reference_left * reference_right;
        let mut expected = Vec::with_capacity(left.rows * right.cols);
        for row in 0..left.rows {
            for col in 0..right.cols {
                expected.push(reference[(row, col)]);
            }
        }

        assert_matrix_approx_eq_f64(
            actual.values(),
            &expected,
            left.rows,
            right.cols,
            strict_tolerance(),
        );
    }
}

#[test]
fn reference_comparison_reports_absolute_and_relative_error() {
    assert_approx_eq_f64(1.0, 1.0 + 1.0e-13, strict_tolerance());
}

fn reference_matmul(
    left: &numerical::FiniteMatrixF64,
    right: &numerical::FiniteMatrixF64,
) -> Vec<f64> {
    let mut result = vec![0.0; left.rows * right.cols];
    for row in 0..left.rows {
        for col in 0..right.cols {
            for shared in 0..left.cols {
                result[row * right.cols + col] +=
                    left.values[row * left.cols + shared] * right.values[shared * right.cols + col];
            }
        }
    }
    result
}
