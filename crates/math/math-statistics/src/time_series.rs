//! Lag-aware covariance and correlation for finite scalar series.
//!
//! Positive lag has one direction throughout this module: `left[t]` is paired
//! with `right[t + lag]`. Autocorrelation uses the same series on both sides.
//! Means and variances are computed over the overlapping slices, so lag zero is
//! exactly equivalent to the crate's existing covariance/correlation contracts.

use super::{
    correlation, covariance, invalid_argument, validate_pair, validate_series,
    variance_denominator, Result, VarianceMode,
};

/// Returns autocovariance for one non-negative lag.
///
/// The overlap is `values[..len-lag]` paired with `values[lag..]`. Each overlap
/// slice is centered by its own mean, matching [`covariance`]. Population mode
/// accepts a one-observation overlap; sample mode requires at least two.
pub fn autocovariance(values: &[f64], lag: usize, mode: VarianceMode) -> Result<f64> {
    validate_series(values, "series")?;
    let overlap = validate_lag(values.len(), lag, mode)?;
    covariance(&values[..overlap], &values[lag..], mode)
}

/// Returns autocorrelation for one non-negative lag.
///
/// Correlation always requires at least two overlapping observations and
/// non-zero variance in both overlapping slices, consistent with [`correlation`].
pub fn autocorrelation(values: &[f64], lag: usize) -> Result<f64> {
    validate_series(values, "series")?;
    let overlap = validate_correlation_lag(values.len(), lag)?;
    correlation(&values[..overlap], &values[lag..])
}

/// Returns directional cross-covariance for one non-negative lag.
///
/// Positive `lag` pairs `left[t]` with `right[t + lag]`. Both input series must
/// have equal length; only the overlapping observations participate in the
/// means, covariance numerator, and normalization denominator.
pub fn cross_covariance(
    left: &[f64],
    right: &[f64],
    lag: usize,
    mode: VarianceMode,
) -> Result<f64> {
    validate_pair(left, right)?;
    let overlap = validate_lag(left.len(), lag, mode)?;
    covariance(&left[..overlap], &right[lag..], mode)
}

/// Returns directional cross-correlation for one non-negative lag.
///
/// Positive `lag` pairs `left[t]` with `right[t + lag]`. At least two
/// overlapping observations with non-zero variance are required.
pub fn cross_correlation(left: &[f64], right: &[f64], lag: usize) -> Result<f64> {
    validate_pair(left, right)?;
    let overlap = validate_correlation_lag(left.len(), lag)?;
    correlation(&left[..overlap], &right[lag..])
}

/// Returns autocovariances for every lag from zero through `max_lag` inclusive.
pub fn autocovariance_series(
    values: &[f64],
    max_lag: usize,
    mode: VarianceMode,
) -> Result<Vec<f64>> {
    validate_series(values, "series")?;
    validate_lag(values.len(), max_lag, mode)?;
    (0..=max_lag)
        .map(|lag| autocovariance(values, lag, mode))
        .collect()
}

/// Returns autocorrelations for every lag from zero through `max_lag` inclusive.
pub fn autocorrelation_series(values: &[f64], max_lag: usize) -> Result<Vec<f64>> {
    validate_series(values, "series")?;
    validate_correlation_lag(values.len(), max_lag)?;
    (0..=max_lag)
        .map(|lag| autocorrelation(values, lag))
        .collect()
}

/// Returns directional cross-covariances for lags zero through `max_lag` inclusive.
pub fn cross_covariance_series(
    left: &[f64],
    right: &[f64],
    max_lag: usize,
    mode: VarianceMode,
) -> Result<Vec<f64>> {
    validate_pair(left, right)?;
    validate_lag(left.len(), max_lag, mode)?;
    (0..=max_lag)
        .map(|lag| cross_covariance(left, right, lag, mode))
        .collect()
}

/// Returns directional cross-correlations for lags zero through `max_lag` inclusive.
pub fn cross_correlation_series(
    left: &[f64],
    right: &[f64],
    max_lag: usize,
) -> Result<Vec<f64>> {
    validate_pair(left, right)?;
    validate_correlation_lag(left.len(), max_lag)?;
    (0..=max_lag)
        .map(|lag| cross_correlation(left, right, lag))
        .collect()
}

fn validate_lag(len: usize, lag: usize, mode: VarianceMode) -> Result<usize> {
    if lag >= len {
        return Err(invalid_argument("lag must be smaller than series length"));
    }
    let overlap = len - lag;
    variance_denominator(overlap, mode)?;
    Ok(overlap)
}

fn validate_correlation_lag(len: usize, lag: usize) -> Result<usize> {
    if lag >= len {
        return Err(invalid_argument("lag must be smaller than series length"));
    }
    let overlap = len - lag;
    if overlap < 2 {
        return Err(invalid_argument(
            "correlation lag must leave at least two overlapping observations",
        ));
    }
    Ok(overlap)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f64, right: f64) {
        assert!(
            (left - right).abs() < 1.0e-10,
            "expected {left} to be close to {right}"
        );
    }

    #[test]
    fn lag_zero_matches_existing_statistics() {
        let values = [1.0, 4.0, 2.0, 8.0, 5.0];
        assert_close(
            autocovariance(&values, 0, VarianceMode::Population).unwrap(),
            super::super::variance(&values, VarianceMode::Population).unwrap(),
        );
        assert_close(
            autocovariance(&values, 0, VarianceMode::Sample).unwrap(),
            super::super::variance(&values, VarianceMode::Sample).unwrap(),
        );
        assert_close(autocorrelation(&values, 0).unwrap(), 1.0);
    }

    #[test]
    fn positive_cross_lag_has_explicit_direction() {
        let left = [1.0, 2.0, 3.0, 4.0, 5.0];
        let right = [99.0, 1.0, 2.0, 3.0, 4.0];
        assert_close(cross_correlation(&left, &right, 1).unwrap(), 1.0);
        assert!(cross_correlation(&right, &left, 1).unwrap() < 1.0);
    }

    #[test]
    fn alternating_series_has_negative_lag_one_autocorrelation() {
        let values = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
        assert_close(autocorrelation(&values, 1).unwrap(), -1.0);
    }

    #[test]
    fn series_helpers_match_repeated_single_lag_calls() {
        let values = [3.0, 1.0, 4.0, 1.5, 5.0, 9.0];
        let auto = autocovariance_series(&values, 3, VarianceMode::Sample).unwrap();
        for (lag, value) in auto.into_iter().enumerate() {
            assert_close(
                value,
                autocovariance(&values, lag, VarianceMode::Sample).unwrap(),
            );
        }

        let right = [2.0, 7.0, 1.0, 8.0, 2.0, 8.0];
        let cross = cross_correlation_series(&values, &right, 3).unwrap();
        for (lag, value) in cross.into_iter().enumerate() {
            assert_close(value, cross_correlation(&values, &right, lag).unwrap());
        }
    }

    #[test]
    fn population_mode_allows_one_pair_but_sample_and_correlation_do_not() {
        let values = [1.0, 2.0, 3.0];
        assert_close(
            autocovariance(&values, 2, VarianceMode::Population).unwrap(),
            0.0,
        );
        assert!(autocovariance(&values, 2, VarianceMode::Sample).is_err());
        assert!(autocorrelation(&values, 2).is_err());
    }

    #[test]
    fn invalid_inputs_and_degenerate_overlaps_are_rejected() {
        assert!(autocorrelation(&[], 0).is_err());
        assert!(autocovariance(&[1.0, f64::NAN], 0, VarianceMode::Population).is_err());
        assert!(autocorrelation(&[1.0, 2.0], 2).is_err());
        assert!(cross_correlation(&[1.0, 2.0], &[1.0], 0).is_err());
        assert!(autocorrelation(&[2.0, 2.0, 2.0], 0).is_err());
    }

    #[test]
    fn reversing_swaps_autocovariance_windows_without_changing_result() {
        let values = [1.0, 2.0, 5.0, 4.0, 8.0, 7.0];
        let reversed = values.iter().rev().copied().collect::<Vec<_>>();
        for lag in 0..4 {
            assert_close(
                autocovariance(&values, lag, VarianceMode::Population).unwrap(),
                autocovariance(&reversed, lag, VarianceMode::Population).unwrap(),
            );
        }
    }
}
