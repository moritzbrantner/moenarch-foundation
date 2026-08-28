//! Shared property-test support for foundation numerical crates.
//!
//! This is intentionally a source module rather than a workspace crate. The
//! repository boundary checker fixes the extracted workspace at 60 packages;
//! integration tests include this module with `#[path]` so that test-only
//! support remains reusable without changing production package ownership.

#![allow(dead_code)]

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, RngSeed};

/// Fixed number of cases for fast deterministic property tests.
pub const PROPERTY_CASES: u32 = 64;
/// Stable seed used by shared property tests.
pub const PROPERTY_SEED: u64 = 0x4d4f_454e_4152_4348;

/// Returns the standard deterministic configuration for numerical properties.
pub fn deterministic_config() -> ProptestConfig {
    let mut config = ProptestConfig::with_cases(PROPERTY_CASES);
    config.rng_seed = RngSeed::Fixed(PROPERTY_SEED);
    config.failure_persistence = None;
    config
}

/// Explicit absolute and relative error limits for a single assertion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ApproxTolerance {
    /// Maximum permitted absolute error.
    pub absolute: f64,
    /// Maximum permitted relative error, scaled by the larger magnitude.
    pub relative: f64,
}

impl ApproxTolerance {
    /// Creates a tolerance after rejecting invalid limits in test code.
    pub fn new(absolute: f64, relative: f64) -> Self {
        assert!(absolute.is_finite() && absolute >= 0.0);
        assert!(relative.is_finite() && relative >= 0.0);
        Self { absolute, relative }
    }
}

/// Panics with a numerical diagnostic unless two `f64` values are sufficiently close.
pub fn assert_approx_eq_f64(left: f64, right: f64, tolerance: ApproxTolerance) {
    let absolute_error = (left - right).abs();
    let scale = left.abs().max(right.abs());
    let relative_error = if scale == 0.0 {
        0.0
    } else {
        absolute_error / scale
    };
    assert!(
        absolute_error <= tolerance.absolute || relative_error <= tolerance.relative,
        "values differ: left={left:?}, right={right:?}, absolute_error={absolute_error:e}, \
         relative_error={relative_error:e}, absolute_tolerance={:e}, relative_tolerance={:e}",
        tolerance.absolute,
        tolerance.relative,
    );
}

/// Panics with a numerical diagnostic unless two `f32` values are sufficiently close.
pub fn assert_approx_eq_f32(left: f32, right: f32, tolerance: ApproxTolerance) {
    assert_approx_eq_f64(left as f64, right as f64, tolerance);
}

/// Compares two equally shaped row-major `f64` matrices with one explicit tolerance.
pub fn assert_matrix_approx_eq_f64(
    left: &[f64],
    right: &[f64],
    rows: usize,
    cols: usize,
    tolerance: ApproxTolerance,
) {
    assert_eq!(left.len(), rows * cols, "left matrix shape mismatch");
    assert_eq!(right.len(), rows * cols, "right matrix shape mismatch");
    for (index, (left, right)) in left.iter().zip(right).enumerate() {
        let row = index / cols;
        let col = index % cols;
        let absolute_error = (*left - *right).abs();
        let scale = left.abs().max(right.abs());
        let relative_error = if scale == 0.0 {
            0.0
        } else {
            absolute_error / scale
        };
        assert!(
            absolute_error <= tolerance.absolute || relative_error <= tolerance.relative,
            "matrix values differ at ({row}, {col}): left={left:?}, right={right:?}, \
             absolute_error={absolute_error:e}, relative_error={relative_error:e}, \
             absolute_tolerance={:e}, relative_tolerance={:e}",
            tolerance.absolute,
            tolerance.relative,
        );
    }
}

/// Generates finite `f32` values spanning ordinary and adversarial magnitudes.
pub fn finite_f32() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0),
        Just(-0.0),
        Just(f32::MIN_POSITIVE),
        Just(-f32::MIN_POSITIVE),
        Just(f32::EPSILON),
        Just(-f32::EPSILON),
        Just(1.0e-20),
        Just(-1.0e-20),
        Just(1.0e20),
        Just(-1.0e20),
        -1.0e6_f32..=1.0e6_f32,
    ]
    .boxed()
}

/// Generates finite `f64` values spanning ordinary and adversarial magnitudes.
pub fn finite_f64() -> BoxedStrategy<f64> {
    prop_oneof![
        Just(0.0),
        Just(-0.0),
        Just(f64::MIN_POSITIVE),
        Just(-f64::MIN_POSITIVE),
        Just(f64::EPSILON),
        Just(-f64::EPSILON),
        Just(1.0e-100),
        Just(-1.0e-100),
        Just(1.0e100),
        Just(-1.0e100),
        -1.0e12_f64..=1.0e12_f64,
    ]
    .boxed()
}

/// Generates non-finite values for constructor rejection tests.
pub fn non_finite_f32() -> BoxedStrategy<f32> {
    prop_oneof![Just(f32::NAN), Just(f32::INFINITY), Just(f32::NEG_INFINITY)].boxed()
}

/// Generates non-finite values for constructor rejection tests.
pub fn non_finite_f64() -> BoxedStrategy<f64> {
    prop_oneof![Just(f64::NAN), Just(f64::INFINITY), Just(f64::NEG_INFINITY)].boxed()
}

/// Generates a finite vector with a caller-selected non-empty length range.
pub fn finite_vector_f32(min_len: usize, max_len: usize) -> BoxedStrategy<Vec<f32>> {
    assert!(min_len > 0 && min_len <= max_len);
    proptest::collection::vec(finite_f32(), min_len..=max_len).boxed()
}

/// A small row-major finite matrix generated for property tests.
#[derive(Debug, Clone, PartialEq)]
pub struct FiniteMatrixF64 {
    pub rows: usize,
    pub cols: usize,
    pub values: Vec<f64>,
}

/// Generates small rectangular finite matrices, including square matrices.
pub fn finite_matrix_f64(max_rows: usize, max_cols: usize) -> BoxedStrategy<FiniteMatrixF64> {
    assert!(max_rows > 0 && max_cols > 0);
    (1_usize..=max_rows, 1_usize..=max_cols)
        .prop_flat_map(|(rows, cols)| {
            proptest::collection::vec(finite_f64(), rows * cols)
                .prop_map(move |values| FiniteMatrixF64 { rows, cols, values })
        })
        .boxed()
}

/// Generates compatible finite matrices for multiplication, including rectangular shapes.
pub fn compatible_matrix_pair_f64(
    max_rows: usize,
    max_shared: usize,
    max_cols: usize,
) -> BoxedStrategy<(FiniteMatrixF64, FiniteMatrixF64)> {
    assert!(max_rows > 0 && max_shared > 0 && max_cols > 0);
    (1_usize..=max_rows, 1_usize..=max_shared, 1_usize..=max_cols)
        .prop_flat_map(|(rows, shared, cols)| {
            (
                proptest::collection::vec(finite_f64(), rows * shared),
                proptest::collection::vec(finite_f64(), shared * cols),
            )
                .prop_map(move |(left, right)| {
                    (
                        FiniteMatrixF64 {
                            rows,
                            cols: shared,
                            values: left,
                        },
                        FiniteMatrixF64 {
                            rows: shared,
                            cols,
                            values: right,
                        },
                    )
                })
        })
        .boxed()
}

/// Generates exactly singular 2x2 matrices by making the second row a multiple of the first.
pub fn singular_matrix_2x2() -> BoxedStrategy<FiniteMatrixF64> {
    (finite_f64(), finite_f64(), -16.0_f64..=16.0_f64)
        .prop_map(|(a, b, scale)| FiniteMatrixF64 {
            rows: 2,
            cols: 2,
            values: vec![a, b, a * scale, b * scale],
        })
        .boxed()
}

/// Generates diagonal 2x2 matrices with a deliberately large condition ratio.
pub fn ill_conditioned_diagonal_2x2() -> BoxedStrategy<FiniteMatrixF64> {
    (1.0_f64..=1.0e6_f64)
        .prop_map(|large| FiniteMatrixF64 {
            rows: 2,
            cols: 2,
            values: vec![large, 0.0, 0.0, large * 1.0e-12],
        })
        .boxed()
}
