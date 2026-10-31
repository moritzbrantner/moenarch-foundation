//! Deterministic fixed-grid bucketing.

use std::collections::BTreeMap;

use media_core::Result;
use serde::{Deserialize, Serialize};

use crate::summary::SummaryAccumulator;
use crate::validation::{invalid_argument, validate_coordinates, validate_point_set};
use crate::{DenseAverages, DenseBounds, DensePoint};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Fixed-width grid used to bucket dense points.
pub struct BucketGrid {
    /// Per-dimension grid origin.
    pub origin: Vec<f64>,
    /// Per-dimension positive cell width.
    pub widths: Vec<f64>,
}

impl BucketGrid {
    /// Creates a new value.
    pub fn new(origin: impl Into<Vec<f64>>, widths: impl Into<Vec<f64>>) -> Result<Self> {
        let grid = Self {
            origin: origin.into(),
            widths: widths.into(),
        };
        grid.validate()?;
        Ok(grid)
    }

    /// Creates a grid with zero origin and the same width in every dimension.
    pub fn uniform(dimensions: usize, width: f64) -> Result<Self> {
        if dimensions == 0 {
            return Err(invalid_argument("bucket grid dimensions must be positive"));
        }
        Self::new(vec![0.0; dimensions], vec![width; dimensions])
    }

    /// Returns the grid dimension count.
    pub fn dimensions(&self) -> usize {
        self.origin.len()
    }

    /// Returns the bucket key for one point.
    pub fn key_for(&self, point: &DensePoint) -> Result<BucketKey> {
        self.validate()?;
        point.validate()?;
        if point.dimensions() != self.dimensions() {
            return Err(invalid_argument(
                "point dimensions must match bucket grid dimensions",
            ));
        }

        let mut indices = Vec::with_capacity(point.dimensions());
        for ((coordinate, origin), width) in
            point.coordinates.iter().zip(&self.origin).zip(&self.widths)
        {
            let index = ((*coordinate - *origin) / *width).floor();
            if index < i64::MIN as f64 || index > i64::MAX as f64 {
                return Err(invalid_argument("bucket index is out of range"));
            }
            indices.push(index as i64);
        }
        Ok(BucketKey { indices })
    }

    /// Validates this value.
    pub fn validate(&self) -> Result<()> {
        validate_coordinates(&self.origin, "bucket grid origin")?;
        if self.widths.is_empty() {
            return Err(invalid_argument("bucket grid widths must not be empty"));
        }
        if self.origin.len() != self.widths.len() {
            return Err(invalid_argument(
                "bucket grid origin and widths must have the same dimensions",
            ));
        }
        if self
            .widths
            .iter()
            .any(|width| *width <= 0.0 || !width.is_finite())
        {
            return Err(invalid_argument(
                "bucket grid widths must be finite and positive",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
/// Integer coordinate identifying one bucket in a fixed grid.
pub struct BucketKey {
    /// Per-dimension integer bucket indices.
    pub indices: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Aggregated dense points that share a bucket key.
pub struct DenseBucket {
    /// Bucket key shared by represented points.
    pub key: BucketKey,
    /// Number of items represented by this value.
    pub count: u64,
    /// Sum of represented point weights.
    pub weight_sum: f64,
    /// Weighted coordinate and scalar averages for represented points.
    pub averages: DenseAverages,
    /// Coordinate bounds for represented points.
    pub bounds: DenseBounds,
    /// Source point indices in the input slice.
    pub point_indices: Vec<usize>,
}

/// Assigns dense points to deterministic fixed-grid buckets.
pub fn bucket_points(points: &[DensePoint], grid: &BucketGrid) -> Result<Vec<DenseBucket>> {
    validate_point_set(points)?;
    grid.validate()?;
    let mut buckets = BTreeMap::<BucketKey, BucketAccumulator>::new();
    for (index, point) in points.iter().enumerate() {
        let key = grid.key_for(point)?;
        buckets
            .entry(key.clone())
            .or_insert_with(|| BucketAccumulator::new(key))
            .push(index, point);
    }
    buckets
        .into_values()
        .map(BucketAccumulator::finish)
        .collect()
}

#[derive(Debug, Clone)]
struct BucketAccumulator {
    key: BucketKey,
    count: u64,
    point_indices: Vec<usize>,
    summary: SummaryAccumulator,
}

impl BucketAccumulator {
    fn new(key: BucketKey) -> Self {
        Self {
            key,
            count: 0,
            point_indices: Vec::new(),
            summary: SummaryAccumulator::new(),
        }
    }

    fn push(&mut self, point_index: usize, point: &DensePoint) {
        self.count += 1;
        self.point_indices.push(point_index);
        self.summary.push(point);
    }

    fn finish(self) -> Result<DenseBucket> {
        Ok(DenseBucket {
            key: self.key,
            count: self.count,
            weight_sum: self.summary.weight_sum,
            averages: self.summary.averages()?,
            bounds: self.summary.bounds()?,
            point_indices: self.point_indices,
        })
    }
}
