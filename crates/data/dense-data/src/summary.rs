//! Weighted coordinate and scalar summaries of dense point sets.

use media_core::Result;
use numbers_core::{NumberSummary, RunningStats};
use serde::{Deserialize, Serialize};

use crate::validation::{invalid_argument, validate_point_set};
use crate::DensePoint;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Weighted averages for coordinates and optional scalar values.
pub struct DenseAverages {
    /// Number of items represented by this value.
    pub count: u64,
    /// Sum of point weights represented by the averages.
    pub weight_sum: f64,
    /// Weighted average coordinate for each dimension.
    pub coordinates: Vec<f64>,
    /// Number of points that carried a scalar value.
    pub value_count: u64,
    /// Sum of weights for points that carried a scalar value.
    pub value_weight_sum: f64,
    /// Weighted average scalar value when at least one value exists.
    pub value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Summary of a dense point set.
pub struct DenseSummary {
    /// Number of items represented by this value.
    pub count: u64,
    /// Number of coordinate dimensions.
    pub dimensions: usize,
    /// Sum of point weights.
    pub weight_sum: f64,
    /// Weighted numeric summary for each coordinate dimension.
    pub coordinate_stats: Vec<NumberSummary>,
    /// Weighted numeric summary for optional scalar values.
    pub value_stats: Option<NumberSummary>,
    /// Per-dimension coordinate bounds.
    pub bounds: DenseBounds,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Per-dimension coordinate bounds.
pub struct DenseBounds {
    /// Minimum coordinate for each dimension.
    pub min: Vec<f64>,
    /// Maximum coordinate for each dimension.
    pub max: Vec<f64>,
}

/// Computes weighted coordinate, value, and bounds summaries for dense points.
pub fn dense_summary(points: &[DensePoint]) -> Result<DenseSummary> {
    SummaryAccumulator::from_points(points)?.summary()
}

/// Computes weighted coordinate averages and optional scalar value averages.
pub fn dense_averages(points: &[DensePoint]) -> Result<DenseAverages> {
    SummaryAccumulator::from_points(points)?.averages()
}

/// Computes per-dimension coordinate bounds for a dense point set.
pub fn dense_bounds(points: &[DensePoint]) -> Result<DenseBounds> {
    SummaryAccumulator::from_points(points)?.bounds()
}

#[derive(Debug, Clone)]
pub(crate) struct SummaryAccumulator {
    count: u64,
    pub(crate) weight_sum: f64,
    coordinate_stats: Vec<RunningStats>,
    value_stats: RunningStats,
    has_values: bool,
    min: Vec<f64>,
    max: Vec<f64>,
}

impl SummaryAccumulator {
    pub(crate) fn new() -> Self {
        Self {
            count: 0,
            weight_sum: 0.0,
            coordinate_stats: Vec::new(),
            value_stats: RunningStats::new(),
            has_values: false,
            min: Vec::new(),
            max: Vec::new(),
        }
    }

    fn from_points(points: &[DensePoint]) -> Result<Self> {
        validate_point_set(points)?;
        let mut summary = Self::new();
        for point in points {
            summary.push(point);
        }
        Ok(summary)
    }

    pub(crate) fn push(&mut self, point: &DensePoint) {
        if self.coordinate_stats.is_empty() {
            self.coordinate_stats
                .resize_with(point.dimensions(), RunningStats::new);
            self.min.resize(point.dimensions(), f64::INFINITY);
            self.max.resize(point.dimensions(), f64::NEG_INFINITY);
        }

        self.count += 1;
        self.weight_sum += point.weight;
        for (index, (stats, coordinate)) in self
            .coordinate_stats
            .iter_mut()
            .zip(&point.coordinates)
            .enumerate()
        {
            stats
                .push_weighted(*coordinate, point.weight)
                .expect("validated points have valid coordinate weights");
            self.min[index] = self.min[index].min(*coordinate);
            self.max[index] = self.max[index].max(*coordinate);
        }
        if let Some(point_value) = point.value {
            self.value_stats
                .push_weighted(point_value, point.weight)
                .expect("validated points have valid scalar weights");
            self.has_values = true;
        }
    }

    pub(crate) fn averages(&self) -> Result<DenseAverages> {
        if self.count == 0 {
            return Err(invalid_argument("dense point set must not be empty"));
        }
        let coordinates = self
            .coordinate_stats
            .iter()
            .map(|stats| {
                stats
                    .summary()
                    .mean
                    .expect("coordinate summary has a weighted mean")
            })
            .collect();
        let value_summary = self.has_values.then(|| self.value_stats.summary());
        Ok(DenseAverages {
            count: self.count,
            weight_sum: self.weight_sum,
            coordinates,
            value_count: value_summary.as_ref().map_or(0, |summary| summary.count),
            value_weight_sum: value_summary
                .as_ref()
                .map_or(0.0, |summary| summary.weight_sum),
            value: value_summary.and_then(|summary| summary.mean),
        })
    }

    pub(crate) fn bounds(&self) -> Result<DenseBounds> {
        if self.count == 0 {
            return Err(invalid_argument("dense point set must not be empty"));
        }
        Ok(DenseBounds {
            min: self.min.clone(),
            max: self.max.clone(),
        })
    }

    fn summary(&self) -> Result<DenseSummary> {
        Ok(DenseSummary {
            count: self.count,
            dimensions: self.coordinate_stats.len(),
            weight_sum: self.weight_sum,
            coordinate_stats: self
                .coordinate_stats
                .iter()
                .map(RunningStats::summary)
                .collect(),
            value_stats: self.has_values.then(|| self.value_stats.summary()),
            bounds: self.bounds()?,
        })
    }
}
