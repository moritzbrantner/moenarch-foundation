//! Validated dense points and the dataset that owns them.

use math_linear::{F32Matrix, MatrixShape};
use math_statistics::{
    CovarianceMatrix, PrincipalComponents, RunningCovariance, WeightedObservation,
};
use media_core::Result;
use serde::{Deserialize, Serialize};

use crate::validation::{invalid_argument, validate_coordinates};
use crate::{
    bucket_points, dense_averages, dense_bounds, dense_summary, k_means, BucketGrid, ClusterResult,
    DenseAverages, DenseBounds, DenseBucket, DenseSummary, KMeansConfig,
};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Weighted point in a fixed-dimensional dense coordinate space.
pub struct DensePoint {
    /// Optional caller-owned identifier.
    pub id: Option<String>,
    /// Finite coordinates. All points in a dataset must have the same length.
    pub coordinates: Vec<f64>,
    /// Finite positive weight used by averages, summaries, covariance, and k-means.
    pub weight: f64,
    /// Optional finite scalar value summarized separately from coordinates.
    pub value: Option<f64>,
}

impl DensePoint {
    /// Creates a unit-weight point from finite coordinates.
    pub fn new(coordinates: impl Into<Vec<f64>>) -> Result<Self> {
        let point = Self {
            id: None,
            coordinates: coordinates.into(),
            weight: 1.0,
            value: None,
        };
        point.validate()?;
        Ok(point)
    }

    /// Attaches a caller-owned identifier.
    pub fn named(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the finite positive point weight.
    pub fn weighted(mut self, weight: f64) -> Result<Self> {
        self.weight = weight;
        self.validate()?;
        Ok(self)
    }

    /// Sets the optional finite scalar value.
    pub fn valued(mut self, value: f64) -> Result<Self> {
        self.value = Some(value);
        self.validate()?;
        Ok(self)
    }

    /// Returns the coordinate dimension count.
    pub fn dimensions(&self) -> usize {
        self.coordinates.len()
    }

    /// Validates this value.
    pub fn validate(&self) -> Result<()> {
        validate_coordinates(&self.coordinates, "point coordinates")?;
        if self.weight <= 0.0 || !self.weight.is_finite() {
            return Err(invalid_argument("point weight must be finite and positive"));
        }
        if self.value.is_some_and(|value| !value.is_finite()) {
            return Err(invalid_argument("point value must be finite"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
/// Collection of same-dimensional dense points.
pub struct DenseDataset {
    dimensions: Option<usize>,
    points: Vec<DensePoint>,
}

impl DenseDataset {
    /// Creates an empty dataset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a dataset while validating dimensions and point values.
    pub fn from_points(points: impl IntoIterator<Item = DensePoint>) -> Result<Self> {
        let mut dataset = Self::new();
        dataset.extend(points)?;
        Ok(dataset)
    }

    /// Adds one point, enforcing the dataset's existing dimension count.
    pub fn push(&mut self, point: DensePoint) -> Result<()> {
        point.validate()?;
        match self.dimensions {
            Some(dimensions) if dimensions != point.dimensions() => {
                return Err(invalid_argument(
                    "dense dataset points must have the same dimensions",
                ));
            }
            None => self.dimensions = Some(point.dimensions()),
            _ => {}
        }
        self.points.push(point);
        Ok(())
    }

    /// Adds multiple points.
    pub fn extend(&mut self, points: impl IntoIterator<Item = DensePoint>) -> Result<()> {
        for point in points {
            self.push(point)?;
        }
        Ok(())
    }

    /// Returns the dataset dimension count when at least one point exists.
    pub fn dimensions(&self) -> Option<usize> {
        self.dimensions
    }

    /// Returns whether the dataset has no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Returns the number of points in the dataset.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns the validated point slice.
    pub fn points(&self) -> &[DensePoint] {
        &self.points
    }

    /// Returns weighted per-dimension and optional scalar averages.
    pub fn averages(&self) -> Result<DenseAverages> {
        dense_averages(&self.points)
    }

    /// Returns weighted per-dimension stats, optional value stats, and bounds.
    pub fn summary(&self) -> Result<DenseSummary> {
        dense_summary(&self.points)
    }

    /// Returns per-dimension coordinate bounds.
    pub fn bounds(&self) -> Result<DenseBounds> {
        dense_bounds(&self.points)
    }

    /// Groups points into deterministic fixed-grid buckets.
    pub fn buckets(&self, grid: &BucketGrid) -> Result<Vec<DenseBucket>> {
        bucket_points(&self.points, grid)
    }

    /// Runs deterministic weighted k-means.
    pub fn k_means(&self, config: KMeansConfig) -> Result<ClusterResult> {
        k_means(&self.points, config)
    }

    /// Exports point coordinates as an `f32` matrix with rows as points.
    pub fn matrix(&self) -> Result<F32Matrix> {
        let dimensions = self
            .dimensions
            .ok_or_else(|| invalid_argument("dense dataset must not be empty"))?;
        let mut values = Vec::with_capacity(self.points.len() * dimensions);
        for point in &self.points {
            values.extend(point.coordinates.iter().map(|value| *value as f32));
        }
        F32Matrix::new(MatrixShape::new(self.points.len(), dimensions)?, values)
    }

    /// Returns the weighted coordinate covariance matrix.
    pub fn covariance_matrix(&self) -> Result<CovarianceMatrix> {
        let dimensions = self
            .dimensions
            .ok_or_else(|| invalid_argument("dense dataset must not be empty"))?;
        let mut covariance = RunningCovariance::new(dimensions)?;
        for point in &self.points {
            covariance.push(WeightedObservation::weighted(
                point.coordinates.clone(),
                point.weight,
            )?)?;
        }
        covariance.covariance_matrix()
    }

    /// Fits principal components from the coordinate matrix.
    pub fn principal_components(&self, component_count: usize) -> Result<PrincipalComponents> {
        let matrix = self.matrix()?;
        PrincipalComponents::fit(&matrix.as_view(), component_count)
    }
}
