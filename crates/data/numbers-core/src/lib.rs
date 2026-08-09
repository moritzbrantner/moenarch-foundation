#![doc = include_str!("../README.md")]

pub mod surface;
use media_core::{DetectError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Summary statistics for a stream or slice of scalar values.
pub struct NumberSummary {
    /// Number of input items, including non-finite values.
    pub count: u64,
    /// Number of finite values that contributed to numeric statistics.
    pub finite_count: u64,
    /// Number of `NaN` or infinite values skipped by numeric statistics.
    pub non_finite_count: u64,
    /// Minimum finite value.
    pub min: Option<f64>,
    /// Maximum finite value.
    pub max: Option<f64>,
    /// Weighted sum of finite values.
    pub sum: Option<f64>,
    /// Weighted mean of finite values.
    pub mean: Option<f64>,
    /// Weighted population variance of finite values.
    pub variance: Option<f64>,
    /// Square root of the weighted population variance.
    pub std_dev: Option<f64>,
    /// Sum of weights for finite values.
    pub weight_sum: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
/// Inclusive finite numeric range used for normalization and histograms.
pub struct NumberRange {
    /// Inclusive lower bound.
    pub min: f64,
    /// Inclusive upper bound.
    pub max: f64,
}

impl NumberRange {
    /// Creates a finite range whose lower bound is not greater than its upper bound.
    pub fn new(min: f64, max: f64) -> Result<Self> {
        if !min.is_finite() || !max.is_finite() {
            return Err(invalid_argument("number range bounds must be finite"));
        }
        if min > max {
            return Err(invalid_argument("number range min must not exceed max"));
        }
        Ok(Self { min, max })
    }

    /// Clamps a value into this range.
    pub fn clamp(self, value: f64) -> Result<f64> {
        if !value.is_finite() {
            return Err(invalid_argument("range value must be finite"));
        }
        Ok(value.clamp(self.min, self.max))
    }

    /// Clamps and normalizes a finite value into `0.0..=1.0`.
    pub fn normalize(self, value: f64) -> Result<f64> {
        let value = self.clamp(value)?;
        if self.min == self.max {
            return Ok(0.0);
        }
        Ok((value - self.min) / (self.max - self.min))
    }

    /// Maps a finite normalized value back into this range.
    pub fn denormalize(self, value: f64) -> Result<f64> {
        if !value.is_finite() {
            return Err(invalid_argument("normalized value must be finite"));
        }
        if self.min == self.max {
            return Ok(self.min);
        }
        Ok(self.min + value * (self.max - self.min))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Interpolated quartile summary over finite values.
pub struct QuartileSummary {
    /// The first quartile value.
    pub q1: f64,
    /// The median value.
    pub median: f64,
    /// The third quartile value.
    pub q3: f64,
    /// The interquartile range value.
    pub iqr: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
/// Configuration for fixed-width histogram generation.
pub struct HistogramConfig {
    /// Number of fixed-width bins to produce.
    pub bins: usize,
    /// Optional explicit inclusive range. When omitted, finite inputs define the range.
    pub range: Option<NumberRange>,
}

impl HistogramConfig {
    /// Creates a new value.
    pub fn new(bins: usize) -> Result<Self> {
        let config = Self { bins, range: None };
        config.validate()?;
        Ok(config)
    }

    /// Sets an explicit inclusive histogram range.
    pub fn with_range(mut self, range: NumberRange) -> Self {
        self.range = Some(range);
        self
    }

    /// Validates this value.
    pub fn validate(&self) -> Result<()> {
        if self.bins == 0 {
            return Err(invalid_argument("histogram bin count must be positive"));
        }
        if let Some(range) = self.range {
            NumberRange::new(range.min, range.max)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// One fixed-width histogram bin.
pub struct HistogramBin {
    /// Inclusive lower bin boundary.
    pub start: f64,
    /// Upper bin boundary. The final bin includes this value.
    pub end: f64,
    /// Number of finite input values in this bin.
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Fixed-width histogram over finite values.
pub struct Histogram {
    /// Histogram bins in ascending order.
    pub bins: Vec<HistogramBin>,
    /// Number of finite values counted into bins.
    pub count: u64,
    /// Inclusive histogram lower bound.
    pub min: f64,
    /// Inclusive histogram upper bound.
    pub max: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
/// Online weighted scalar statistics accumulator.
pub struct RunningStats {
    count: u64,
    finite_count: u64,
    non_finite_count: u64,
    min: Option<f64>,
    max: Option<f64>,
    sum: f64,
    weight_sum: f64,
    mean: f64,
    m2: f64,
}

impl RunningStats {
    /// Creates an empty accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one value with unit weight.
    pub fn push(&mut self, value: f64) {
        self.push_weighted_internal(value, 1.0, false)
            .expect("unit weight is valid");
    }

    /// Adds one value with a finite positive weight.
    pub fn push_weighted(&mut self, value: f64, weight: f64) -> Result<()> {
        self.push_weighted_internal(value, weight, true)
    }

    /// Adds values with unit weight.
    pub fn extend(&mut self, values: impl IntoIterator<Item = f64>) {
        for value in values {
            self.push(value);
        }
    }

    /// Merges another accumulator into this one.
    pub fn merge(&mut self, other: &Self) {
        self.count += other.count;
        self.finite_count += other.finite_count;
        self.non_finite_count += other.non_finite_count;
        self.min = merge_lower_bound(self.min, other.min);
        self.max = merge_upper_bound(self.max, other.max);
        self.sum += other.sum;

        if other.weight_sum == 0.0 {
            return;
        }
        if self.weight_sum == 0.0 {
            self.weight_sum = other.weight_sum;
            self.mean = other.mean;
            self.m2 = other.m2;
            return;
        }

        let total_weight = self.weight_sum + other.weight_sum;
        let delta = other.mean - self.mean;
        self.m2 += other.m2 + delta * delta * self.weight_sum * other.weight_sum / total_weight;
        self.mean += delta * other.weight_sum / total_weight;
        self.weight_sum = total_weight;
    }

    /// Returns the current weighted summary.
    ///
    /// Variance is the weighted population variance over finite values. Non-finite values
    /// increase `count` and `non_finite_count`, but do not change weighted statistics.
    pub fn summary(&self) -> NumberSummary {
        let variance = (self.weight_sum > 0.0).then_some(self.m2 / self.weight_sum);
        NumberSummary {
            count: self.count,
            finite_count: self.finite_count,
            non_finite_count: self.non_finite_count,
            min: self.min,
            max: self.max,
            sum: (self.weight_sum > 0.0).then_some(self.sum),
            mean: (self.weight_sum > 0.0).then_some(self.mean),
            variance,
            std_dev: variance.map(f64::sqrt),
            weight_sum: self.weight_sum,
        }
    }

    fn push_weighted_internal(
        &mut self,
        value: f64,
        weight: f64,
        validate_weight: bool,
    ) -> Result<()> {
        if validate_weight && (!weight.is_finite() || weight <= 0.0) {
            return Err(invalid_argument("stat weight must be finite and positive"));
        }

        self.count += 1;
        if !value.is_finite() {
            self.non_finite_count += 1;
            return Ok(());
        }

        self.record_finite_value(value, weight);
        Ok(())
    }

    fn record_finite_value(&mut self, value: f64, weight: f64) {
        self.finite_count += 1;
        self.min = Some(self.min.map_or(value, |min| min.min(value)));
        self.max = Some(self.max.map_or(value, |max| max.max(value)));

        let previous_weight = self.weight_sum;
        let next_weight = previous_weight + weight;
        let delta = value - self.mean;
        let next_mean = if previous_weight == 0.0 {
            value
        } else {
            self.mean + delta * weight / next_weight
        };
        let delta2 = value - next_mean;

        self.sum += value * weight;
        self.weight_sum = next_weight;
        self.mean = next_mean;
        self.m2 += weight * delta * delta2;
    }
}

/// Summarizes values with unit weights, counting but skipping non-finite values.
pub fn summarize_numbers(values: &[f64]) -> NumberSummary {
    let mut stats = RunningStats::new();
    stats.extend(values.iter().copied());
    stats.summary()
}

/// Computes a linearly interpolated quantile over finite values.
pub fn quantile(values: &[f64], quantile: f64) -> Result<f64> {
    if !quantile.is_finite() || !(0.0..=1.0).contains(&quantile) {
        return Err(invalid_argument(
            "quantile must be finite and between 0 and 1",
        ));
    }

    let mut finite = collect_finite_values(values, "quantile")?;
    finite.sort_by(f64::total_cmp);
    let rank = quantile * (finite.len() - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        return Ok(finite[lower]);
    }
    let fraction = rank - lower as f64;
    Ok(finite[lower] + (finite[upper] - finite[lower]) * fraction)
}

/// Computes interpolated first quartile, median, third quartile, and IQR.
pub fn quartiles(values: &[f64]) -> Result<QuartileSummary> {
    let q1 = quantile(values, 0.25)?;
    let median = quantile(values, 0.5)?;
    let q3 = quantile(values, 0.75)?;
    Ok(QuartileSummary {
        q1,
        median,
        q3,
        iqr: q3 - q1,
    })
}

/// Builds fixed-width bins over finite values.
pub fn histogram(values: &[f64], config: HistogramConfig) -> Result<Histogram> {
    config.validate()?;

    let finite = collect_finite_values(values, "histogram")?;
    let range = config.range.unwrap_or(derive_range(&finite));
    let mut bins = build_histogram_bins(range, config.bins);

    for value in finite {
        if value < range.min || value > range.max {
            continue;
        }
        let bin_index = histogram_bin_index(value, range, config.bins);
        bins[bin_index.min(config.bins - 1)].count += 1;
    }

    Ok(Histogram {
        count: bins.iter().map(|bin| bin.count).sum(),
        bins,
        min: range.min,
        max: range.max,
    })
}

fn invalid_argument(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}

fn merge_lower_bound(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn merge_upper_bound(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn collect_finite_values(values: &[f64], operation: &str) -> Result<Vec<f64>> {
    let finite = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if finite.is_empty() {
        return Err(invalid_argument(format!(
            "{operation} requires at least one finite numeric value"
        )));
    }
    Ok(finite)
}

fn derive_range(values: &[f64]) -> NumberRange {
    NumberRange {
        min: *values
            .iter()
            .min_by(|left, right| left.total_cmp(right))
            .expect("finite values exist"),
        max: *values
            .iter()
            .max_by(|left, right| left.total_cmp(right))
            .expect("finite values exist"),
    }
}

fn build_histogram_bins(range: NumberRange, bins: usize) -> Vec<HistogramBin> {
    if range.min == range.max {
        return (0..bins)
            .map(|_| HistogramBin {
                start: range.min,
                end: range.max,
                count: 0,
            })
            .collect();
    }

    let width = (range.max - range.min) / bins as f64;
    (0..bins)
        .map(|index| {
            let start = range.min + index as f64 * width;
            let end = if index + 1 == bins {
                range.max
            } else {
                start + width
            };
            HistogramBin {
                start,
                end,
                count: 0,
            }
        })
        .collect()
}

fn histogram_bin_index(value: f64, range: NumberRange, bins: usize) -> usize {
    if range.min == range.max || value == range.max {
        bins - 1
    } else {
        (((value - range.min) / (range.max - range.min)) * bins as f64).floor() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f64, right: f64) {
        assert!((left - right).abs() < 1.0e-12, "{left} != {right}");
    }

    #[test]
    fn running_stats_track_finite_and_non_finite_values() {
        let mut stats = RunningStats::new();
        stats.push(1.0);
        stats.push(f64::INFINITY);
        stats.push_weighted(3.0, 2.0).unwrap();

        let summary = stats.summary();
        assert_eq!(summary.count, 3);
        assert_eq!(summary.finite_count, 2);
        assert_eq!(summary.non_finite_count, 1);
        assert_eq!(summary.weight_sum, 3.0);
        assert_eq!(summary.sum, Some(7.0));
        assert_close(summary.mean.unwrap(), 7.0 / 3.0);
    }

    #[test]
    fn running_stats_merge_preserves_weighted_summary() {
        let mut left = RunningStats::new();
        left.push(1.0);
        left.push_weighted(5.0, 2.0).unwrap();

        let mut right = RunningStats::new();
        right.push(f64::NAN);
        right.push_weighted(3.0, 3.0).unwrap();

        left.merge(&right);
        let summary = left.summary();
        assert_eq!(summary.count, 4);
        assert_eq!(summary.finite_count, 3);
        assert_eq!(summary.non_finite_count, 1);
        assert_eq!(summary.weight_sum, 6.0);
        assert_close(summary.mean.unwrap(), 20.0 / 6.0);
    }

    #[test]
    fn merged_running_stats_match_single_pass_stats() {
        let mut left = RunningStats::new();
        left.push_weighted(1.0, 2.0).unwrap();
        left.push(f64::NAN);

        let mut right = RunningStats::new();
        right.push_weighted(3.0, 4.0).unwrap();
        right.push(f64::INFINITY);

        let mut merged = left.clone();
        merged.merge(&right);

        let mut single_pass = RunningStats::new();
        single_pass.push_weighted(1.0, 2.0).unwrap();
        single_pass.push(f64::NAN);
        single_pass.push_weighted(3.0, 4.0).unwrap();
        single_pass.push(f64::INFINITY);

        let merged = merged.summary();
        let single = single_pass.summary();
        assert_eq!(merged.count, single.count);
        assert_eq!(merged.finite_count, single.finite_count);
        assert_eq!(merged.non_finite_count, single.non_finite_count);
        assert_close(merged.mean.unwrap(), single.mean.unwrap());
        assert_close(merged.variance.unwrap(), single.variance.unwrap());
    }

    #[test]
    fn quantile_interpolates_even_sized_inputs() {
        assert_eq!(quantile(&[1.0, 2.0, 3.0, 4.0], 0.5).unwrap(), 2.5);
        assert_eq!(quartiles(&[1.0, 2.0, 3.0, 4.0]).unwrap().q1, 1.75);
    }

    #[test]
    fn quantiles_are_monotonic() {
        let values = [10.0, -2.0, 4.0, 4.0, 9.0, f64::NAN];
        let q10 = quantile(&values, 0.10).unwrap();
        let q50 = quantile(&values, 0.50).unwrap();
        let q90 = quantile(&values, 0.90).unwrap();

        assert!(q10 <= q50);
        assert!(q50 <= q90);
    }

    #[test]
    fn histogram_places_max_value_in_last_bin() {
        let histogram = histogram(&[0.0, 0.5, 1.0], HistogramConfig::new(2).unwrap()).unwrap();
        assert_eq!(histogram.count, 3);
        assert_eq!(histogram.bins[0].count, 1);
        assert_eq!(histogram.bins[1].count, 2);
    }

    #[test]
    fn histogram_counts_only_finite_values_inside_explicit_range() {
        let histogram = histogram(
            &[-10.0, 0.0, 0.25, 0.75, 1.0, 10.0, f64::NAN],
            HistogramConfig::new(4)
                .unwrap()
                .with_range(NumberRange::new(0.0, 1.0).unwrap()),
        )
        .unwrap();

        assert_eq!(histogram.count, 4);
        assert_eq!(
            histogram.bins.iter().map(|bin| bin.count).sum::<u64>(),
            histogram.count
        );
    }

    #[test]
    fn histogram_supports_degenerate_ranges() {
        let range = NumberRange::new(3.0, 3.0).unwrap();
        let histogram = histogram(
            &[3.0, 3.0, 3.0],
            HistogramConfig::new(4).unwrap().with_range(range),
        )
        .unwrap();
        assert_eq!(histogram.bins[3].count, 3);
    }

    #[test]
    fn rejects_invalid_inputs() {
        assert!(NumberRange::new(2.0, 1.0).is_err());
        assert!(HistogramConfig::new(0).is_err());
        assert!(quantile(&[], 0.5).is_err());
        assert!(quantile(&[1.0], 2.0).is_err());
        assert!(RunningStats::new().push_weighted(1.0, 0.0).is_err());
    }
}
