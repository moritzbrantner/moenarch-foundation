//! Small shared numerical contracts.
//!
//! These helpers intentionally describe only semantics shared by multiple
//! foundation owners. They do not define a universal scalar trait, select
//! algorithm tolerances, or merge floating-point and exact-number domains.

use serde::{Deserialize, Serialize};

/// Explicit absolute and relative error limits for one numerical comparison.
///
/// Callers choose this value at their algorithm boundary. A tolerance has no
/// global default because scale and acceptable error are operation-specific.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ApproxTolerance {
    /// Maximum permitted absolute error.
    pub absolute: f64,
    /// Maximum permitted relative error, measured against the larger magnitude.
    pub relative: f64,
}

impl ApproxTolerance {
    /// Creates finite, non-negative absolute and relative limits.
    pub fn new(absolute: f64, relative: f64) -> Option<Self> {
        (absolute.is_finite() && absolute >= 0.0 && relative.is_finite() && relative >= 0.0)
            .then_some(Self { absolute, relative })
    }

    /// Returns whether finite `f64` values satisfy either configured limit.
    pub fn allows_f64(self, left: f64, right: f64) -> bool {
        if !left.is_finite() || !right.is_finite() {
            return false;
        }
        let absolute_error = (left - right).abs();
        let scale = left.abs().max(right.abs());
        let relative_error = if scale == 0.0 {
            0.0
        } else {
            absolute_error / scale
        };
        absolute_error <= self.absolute || relative_error <= self.relative
    }

    /// Returns whether finite `f32` values satisfy either configured limit.
    pub fn allows_f32(self, left: f32, right: f32) -> bool {
        self.allows_f64(left as f64, right as f64)
    }
}

/// Returns whether every element of an `f32` slice is finite.
pub fn is_finite_f32_slice(values: &[f32]) -> bool {
    values.iter().all(|value| value.is_finite())
}

/// Returns whether every element of an `f64` slice is finite.
pub fn is_finite_f64_slice(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

/// Converts `f64` to `f32` only when the source is finite and representable.
pub fn checked_f64_to_f32(value: f64) -> Option<f32> {
    (value.is_finite() && value >= f32::MIN as f64 && value <= f32::MAX as f64)
        .then_some(value as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerance_requires_finite_non_negative_limits() {
        assert!(ApproxTolerance::new(1.0e-6, 1.0e-3).is_some());
        assert!(ApproxTolerance::new(-1.0, 0.0).is_none());
        assert!(ApproxTolerance::new(f64::NAN, 0.0).is_none());
    }

    #[test]
    fn tolerance_uses_absolute_or_relative_error() {
        let tolerance = ApproxTolerance::new(1.0e-6, 1.0e-3).unwrap();
        assert!(tolerance.allows_f64(0.0, 5.0e-7));
        assert!(tolerance.allows_f64(10_000.0, 10_005.0));
        assert!(!tolerance.allows_f64(0.0, 1.0));
        assert!(!tolerance.allows_f64(f64::NAN, 0.0));
    }

    #[test]
    fn checked_conversion_rejects_non_finite_and_out_of_range_values() {
        assert_eq!(checked_f64_to_f32(1.5), Some(1.5));
        assert_eq!(checked_f64_to_f32(f64::NAN), None);
        assert_eq!(checked_f64_to_f32(f64::INFINITY), None);
        assert_eq!(checked_f64_to_f32(f64::MAX), None);
    }
}
