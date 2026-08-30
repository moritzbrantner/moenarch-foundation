#[path = "../../../test-support/numerical.rs"]
mod numerical;

use numbers_core::{checked_f64_to_f32, ApproxTolerance};
use proptest::prelude::*;

proptest! {
    #![proptest_config(numerical::deterministic_config())]

    #[test]
    fn checked_f64_to_f32_never_returns_a_non_finite_value(value in numerical::finite_f64()) {
        if let Some(converted) = checked_f64_to_f32(value) {
            prop_assert!(converted.is_finite());
            prop_assert!(value.is_finite());
            prop_assert!(value >= f32::MIN as f64);
            prop_assert!(value <= f32::MAX as f64);
        }
    }

    #[test]
    fn approximate_tolerance_is_symmetric(
        left in numerical::finite_f64(),
        right in numerical::finite_f64(),
    ) {
        let tolerance = ApproxTolerance::new(1.0e-9, 1.0e-6).unwrap();
        prop_assert_eq!(
            tolerance.allows_f64(left, right),
            tolerance.allows_f64(right, left),
        );
    }

    #[test]
    fn non_finite_values_never_satisfy_a_tolerance(value in numerical::non_finite_f64()) {
        let tolerance = ApproxTolerance::new(1.0, 1.0).unwrap();
        prop_assert!(!tolerance.allows_f64(value, 0.0));
    }
}
