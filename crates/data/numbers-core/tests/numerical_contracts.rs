#[path = "support/numerical.rs"]
mod numerical;

use numbers_core::{checked_f64_to_f32, ApproxTolerance};
use proptest::prelude::*;

#[test]
fn relative_comparison_handles_opposite_sign_finite_extremes() {
    for (left, right, limit) in [
        (f64::MAX, -f64::MAX, 2.0),
        (f64::MAX, -f64::MAX / 2.0, 1.5),
        (-f64::MAX, f64::MAX / 2.0, 1.5),
    ] {
        let inclusive = ApproxTolerance::new(0.0, limit).unwrap();
        let too_small = ApproxTolerance::new(0.0, limit - 0.01).unwrap();
        assert!(inclusive.allows_f64(left, right));
        assert!(inclusive.allows_f64(right, left));
        assert!(!too_small.allows_f64(left, right));
    }
    let exact = ApproxTolerance::new(0.0, 0.0).unwrap();
    assert!(exact.allows_f64(f64::MAX, f64::MAX));
    assert!(!exact.allows_f64(f64::MAX, f64::MAX.next_down()));
    assert!(!exact.allows_f64(f64::from_bits(1), 0.0));
    assert!(exact.allows_f64(0.0, -0.0));
    assert!(!ApproxTolerance::new(f64::MAX, f64::MAX)
        .unwrap()
        .allows_f64(f64::INFINITY, 0.0));
}

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
