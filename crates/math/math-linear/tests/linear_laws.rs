#[path = "../../../test-support/numerical.rs"]
mod numerical;

use math_linear::{F32Matrix, F64Matrix, MatrixShape, PseudoinverseOptions, SvdOptions};
use numerical::{assert_matrix_approx_eq_f64, deterministic_config, ApproxTolerance};
use proptest::prelude::*;
use tensor_data::F32Tensor;
use vector_analysis_core::DenseVector;

fn f32_tolerance() -> ApproxTolerance {
    ApproxTolerance::new(2.0e-4, 2.0e-4).unwrap()
}

fn f64_tolerance() -> ApproxTolerance {
    ApproxTolerance::new(1.0e-8, 1.0e-8).unwrap()
}

fn f32_matrix(rows: usize, cols: usize, values: Vec<f32>) -> F32Matrix {
    F32Matrix::new(MatrixShape::new(rows, cols).unwrap(), values).unwrap()
}

fn f64_matrix(rows: usize, cols: usize, values: Vec<f64>) -> F64Matrix {
    F64Matrix::new(MatrixShape::new(rows, cols).unwrap(), values).unwrap()
}

fn assert_f32_matrix_close(left: &F32Matrix, right: &F32Matrix, tolerance: ApproxTolerance) {
    assert_eq!(left.shape(), right.shape());
    assert_matrix_approx_eq_f64(
        &left
            .values()
            .iter()
            .map(|value| *value as f64)
            .collect::<Vec<_>>(),
        &right
            .values()
            .iter()
            .map(|value| *value as f64)
            .collect::<Vec<_>>(),
        left.shape().rows,
        left.shape().cols,
        tolerance,
    );
}

fn assert_f64_matrix_close(left: &F64Matrix, right: &F64Matrix, tolerance: ApproxTolerance) {
    assert_eq!(left.shape(), right.shape());
    assert_matrix_approx_eq_f64(
        left.values(),
        right.values(),
        left.shape().rows,
        left.shape().cols,
        tolerance,
    );
}

fn transpose_f64(matrix: &F64Matrix) -> F64Matrix {
    matrix.transpose_owned().unwrap()
}

proptest! {
    #![proptest_config(deterministic_config())]

    #[test]
    fn f32_transpose_identity_and_gram_laws(values in proptest::collection::vec(-20.0_f32..=20.0_f32, 6)) {
        let matrix = f32_matrix(2, 3, values);
        let twice_transposed = matrix.transpose_owned().unwrap().transpose_owned().unwrap();
        assert_f32_matrix_close(&matrix, &twice_transposed, f32_tolerance());

        let identity = F32Matrix::identity(3).unwrap();
        let right_identity_product = matrix.matmul(&identity.as_view()).unwrap();
        assert_f32_matrix_close(&matrix, &right_identity_product, f32_tolerance());

        let gram = matrix.gram_rows().unwrap();
        let gram_transpose = gram.transpose_owned().unwrap();
        assert_f32_matrix_close(&gram, &gram_transpose, f32_tolerance());
    }

    #[test]
    fn f32_distributivity_and_matrix_vector_consistency(
        left_values in proptest::collection::vec(-10.0_f32..=10.0_f32, 6),
        middle_values in proptest::collection::vec(-10.0_f32..=10.0_f32, 6),
        right_values in proptest::collection::vec(-10.0_f32..=10.0_f32, 6),
        vector_values in proptest::collection::vec(-10.0_f32..=10.0_f32, 3),
    ) {
        let left = f32_matrix(2, 3, left_values);
        let middle = f32_matrix(3, 2, middle_values);
        let right = f32_matrix(3, 2, right_values);
        let sum = middle.as_view().add(&right.as_view()).unwrap();

        let left_distributed = left.matmul(&sum.as_view()).unwrap();
        let distributed_sum = left
            .matmul(&middle.as_view()).unwrap()
            .as_view()
            .add(&left.matmul(&right.as_view()).unwrap().as_view())
            .unwrap();
        assert_f32_matrix_close(&left_distributed, &distributed_sum, f32_tolerance());

        let vector = DenseVector::new(vector_values.clone()).unwrap();
        let matrix_vector = left.matmul(&f32_matrix(3, 1, vector_values).as_view()).unwrap();
        let vector_result = left.matvec(vector.as_slice()).unwrap();
        prop_assert_eq!(matrix_vector.shape(), MatrixShape::new(2, 1).unwrap());
        for row in 0..2 {
            prop_assert!(f32_tolerance().allows_f32(matrix_vector.values()[row], vector_result.as_slice()[row]));
        }
    }

    #[test]
    fn tensor_matrix_bridge_preserves_rank_two_values(
        values in proptest::collection::vec(-100.0_f32..=100.0_f32, 6),
    ) {
        let tensor = F32Tensor::from_dims([2, 3], values.clone()).unwrap();
        let matrix = F32Matrix::try_from(&tensor).unwrap();
        let round_trip = F32Tensor::try_from(&matrix).unwrap();

        prop_assert_eq!(matrix.shape(), MatrixShape::new(2, 3).unwrap());
        prop_assert_eq!(matrix.values(), values.as_slice());
        prop_assert_eq!(round_trip.shape().dimensions(), &[2, 3]);
        prop_assert_eq!(round_trip.values(), values.as_slice());
    }

    #[test]
    fn lu_reconstructs_permuted_input_and_solves_consistently(
        a in -10.0_f32..=10.0_f32,
        b in -10.0_f32..=10.0_f32,
        c in -10.0_f32..=10.0_f32,
        d in -10.0_f32..=10.0_f32,
        target in proptest::collection::vec(-10.0_f32..=10.0_f32, 2),
    ) {
        prop_assume!((a * d - b * c).abs() > 1.0);
        let matrix = f32_matrix(2, 2, vec![a, b, c, d]);
        let decomposition = matrix.lu_decompose().unwrap();
        let lower = decomposition.lower_matrix().unwrap();
        let upper = decomposition.upper_matrix().unwrap();
        let reconstructed = lower.matmul(&upper.as_view()).unwrap();
        let permutation = decomposition.pivots();
        let permuted = f32_matrix(
            2,
            2,
            permutation
                .iter()
                .flat_map(|row| matrix.as_view().row(*row).unwrap().as_slice())
                .collect(),
        );
        assert_f32_matrix_close(&reconstructed, &permuted, f32_tolerance());

        let solution = decomposition.solve_vector(&target).unwrap();
        let reconstructed_target = matrix.matvec(&solution).unwrap();
        for (actual, expected) in reconstructed_target.as_slice().iter().zip(&target) {
            prop_assert!(f32_tolerance().allows_f32(*actual, *expected));
        }

        let inverse = matrix.inverse().unwrap();
        let identity = matrix.matmul(&inverse.as_view()).unwrap();
        assert_f32_matrix_close(&identity, &F32Matrix::identity(2).unwrap(), f32_tolerance());
        prop_assert!(f32_tolerance().allows_f32(
            matrix.determinant().unwrap(),
            decomposition.determinant().unwrap(),
        ));
    }

    #[test]
    fn qr_and_cholesky_reconstruct_their_inputs(
        a in -5.0_f32..=5.0_f32,
        b in -5.0_f32..=5.0_f32,
        c in -5.0_f32..=5.0_f32,
        d in -5.0_f32..=5.0_f32,
    ) {
        let tall = f32_matrix(3, 2, vec![1.0, 0.0, 0.0, 1.0, a, b]);
        let qr = tall.qr_decompose().unwrap();
        let reconstructed = qr.q.matmul(&qr.r.as_view()).unwrap();
        assert_f32_matrix_close(&reconstructed, &tall, f32_tolerance());
        let q_transpose_q = qr.q.transpose_view().matmul(&qr.q.as_view()).unwrap();
        assert_f32_matrix_close(&q_transpose_q, &F32Matrix::identity(2).unwrap(), f32_tolerance());

        let base = f32_matrix(2, 2, vec![a, b, c, d]);
        let spd = base
            .matmul(&base.transpose_view())
            .unwrap()
            .as_view()
            .add(&F32Matrix::identity(2).unwrap().as_view())
            .unwrap();
        let cholesky = spd.cholesky_decompose().unwrap();
        let reconstructed_spd = cholesky
            .lower
            .matmul(&cholesky.lower.transpose_view())
            .unwrap();
        assert_f32_matrix_close(&reconstructed_spd, &spd, f32_tolerance());
    }

    #[test]
    fn svd_and_pseudoinverse_satisfy_reconstruction_laws(
        values in proptest::collection::vec(-10.0_f64..=10.0_f64, 6),
    ) {
        let matrix = f64_matrix(2, 3, values);
        let decomposition = matrix.svd(SvdOptions {
            compute_factors: true,
            ..SvdOptions::default()
        }).unwrap();
        prop_assert!(decomposition.reconstruction.relative_residual <= 1.0e-8);

        let pseudoinverse = matrix.pseudoinverse(PseudoinverseOptions::default()).unwrap();
        let a_a_plus = matrix.matmul(&pseudoinverse.as_view()).unwrap();
        let a_plus_a = pseudoinverse.matmul(&matrix.as_view()).unwrap();
        assert_f64_matrix_close(
            &a_a_plus.matmul(&matrix.as_view()).unwrap(),
            &matrix,
            f64_tolerance(),
        );
        assert_f64_matrix_close(
            &a_plus_a.matmul(&pseudoinverse.as_view()).unwrap(),
            &pseudoinverse,
            f64_tolerance(),
        );
        assert_f64_matrix_close(&transpose_f64(&a_a_plus), &a_a_plus, f64_tolerance());
        assert_f64_matrix_close(&transpose_f64(&a_plus_a), &a_plus_a, f64_tolerance());
    }
}

#[test]
fn adversarial_linear_cases_have_documented_outcomes() {
    let singular = f32_matrix(2, 2, vec![1.0, 1.0, 1.0, 1.0]);
    assert!(singular.lu_decompose().is_err());

    let nearly_singular = f32_matrix(2, 2, vec![1.0, 1.0, 1.0, 1.0 + 1.0e-7]);
    assert!(nearly_singular.lu_decompose().is_err());

    let ill_conditioned = f64_matrix(2, 2, vec![1.0e6, 0.0, 0.0, 1.0e-6]);
    assert_eq!(ill_conditioned.numerical_rank(None).unwrap(), 2);

    let tiny = f64_matrix(2, 2, vec![1.0e-100, 0.0, 0.0, 2.0e-100]);
    assert!(tiny.svd(SvdOptions::default()).is_ok());

    let large = f64_matrix(2, 2, vec![1.0e100, 0.0, 0.0, 2.0e100]);
    assert!(large.svd(SvdOptions::default()).is_ok());
}
