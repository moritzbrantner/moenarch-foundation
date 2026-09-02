use math_linear::{F32Matrix, F64Matrix, SymmetricEigenOptions};
use math_probability::Bernoulli;
use math_sparse_data::CooMatrix;
use math_statistics::{cross_correlation, summarize_series, RunningCovariance, VarianceMode};

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual} with tolerance {tolerance}"
    );
}

#[test]
fn explicit_probability_fixture_matches_statistical_moments() {
    let distribution = Bernoulli::new(0.25).expect("valid Bernoulli distribution");
    // Deliberately explicit data: this is evidence for composition, not a hidden sampler.
    let observations = [0.0, 0.0, 0.0, 1.0];
    let summary = summarize_series(&observations, VarianceMode::Population)
        .expect("finite deterministic fixture");

    assert_close(summary.mean, distribution.mean(), 1.0e-15);
    assert_close(summary.variance, distribution.variance(), 1.0e-15);
}

#[test]
fn shifted_fixture_uses_the_documented_positive_lag_direction() {
    let left = [1.0, 2.0, 3.0, 4.0, 5.0];
    let right = [99.0, 1.0, 2.0, 3.0, 4.0];
    assert_close(
        cross_correlation(&left, &right, 1).expect("non-degenerate overlap"),
        1.0,
        1.0e-15,
    );
}

#[test]
fn covariance_round_trips_through_symmetric_eigendecomposition() {
    let observations = F32Matrix::from_rows([[0.0, 0.0], [1.0, 2.0], [2.0, 1.0], [3.0, 3.0]])
        .expect("finite observation matrix");
    let covariance = RunningCovariance::from_matrix(&observations.as_view())
        .expect("valid observations")
        .covariance_matrix()
        .expect("non-empty covariance");
    let matrix = F64Matrix::try_from(&covariance.matrix).expect("f32 to f64 promotion");
    let decomposition = matrix
        .symmetric_eigendecomposition(SymmetricEigenOptions::default())
        .expect("covariance matrix is symmetric");
    let reconstructed = decomposition.reconstruct().expect("reconstruction");

    assert!(decomposition.sweeps > 0);
    assert!(decomposition.sweeps <= 64);
    assert!(decomposition
        .eigenvalues
        .iter()
        .all(|value| *value >= -1.0e-10));
    for (actual, expected) in reconstructed.values().iter().zip(matrix.values()) {
        assert_close(*actual, *expected, 1.0e-10);
    }
}

#[test]
fn sparse_product_matches_dense_oracle_and_exposes_less_candidate_work() {
    let left = CooMatrix::new(
        3,
        4,
        vec![(0, 0, 2.0), (0, 3, -1.0), (1, 1, 3.0), (2, 2, 4.0)],
    )
    .expect("valid left COO")
    .to_csr()
    .expect("left CSR");
    let right = CooMatrix::new(
        4,
        2,
        vec![(0, 0, 5.0), (1, 1, 2.0), (2, 0, -2.0), (3, 1, 7.0)],
    )
    .expect("valid right COO")
    .to_csr()
    .expect("right CSR");

    let (product, work) = left
        .mul_csr_with_stats(&right)
        .expect("compatible sparse multiplication");
    let actual = product.to_dense_matrix().expect("dense result bridge");
    let left_dense = left.to_dense_matrix().expect("dense left bridge");
    let right_dense = right.to_dense_matrix().expect("dense right bridge");
    let expected = left_dense
        .matmul(&right_dense.as_view())
        .expect("dense multiplication oracle");

    assert_eq!(actual.shape(), expected.shape());
    for (actual, expected) in actual.values().iter().zip(expected.values()) {
        assert!((actual - expected).abs() <= 1.0e-5);
    }

    let dense_candidate_products = left.rows() * left.cols() * right.cols();
    assert!(work.candidate_products < dense_candidate_products);
    assert_eq!(
        work.output_nnz,
        product
            .to_coo()
            .expect("canonical COO evidence")
            .entries()
            .len()
    );

    let finite_values = actual
        .values()
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let summary = summarize_series(&finite_values, VarianceMode::Population)
        .expect("sparse-to-dense output remains finite");
    assert_eq!(summary.count, actual.shape().rows * actual.shape().cols);
}
