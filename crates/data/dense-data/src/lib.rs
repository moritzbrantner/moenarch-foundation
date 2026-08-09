#![doc = include_str!("../README.md")]

pub mod surface;
use std::collections::BTreeMap;

use math_linear::{F32Matrix, MatrixShape};
use math_statistics::{
    CovarianceMatrix, PrincipalComponents, RunningCovariance, WeightedObservation,
};
use media_core::{DetectError, Result};
use numbers_core::{NumberSummary, RunningStats};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
/// Configuration for deterministic weighted k-means clustering.
pub struct KMeansConfig {
    /// Number of clusters to fit.
    pub clusters: usize,
    /// Maximum number of centroid update iterations.
    pub max_iterations: usize,
    /// Non-negative movement tolerance used for convergence.
    pub tolerance: f64,
}

impl KMeansConfig {
    /// Creates a new value.
    pub fn new(clusters: usize) -> Result<Self> {
        let config = Self {
            clusters,
            ..Self::default()
        };
        config.validate()?;
        Ok(config)
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if self.clusters == 0 {
            return Err(invalid_argument("cluster count must be positive"));
        }
        if self.max_iterations == 0 {
            return Err(invalid_argument("max iterations must be positive"));
        }
        if self.tolerance < 0.0 || !self.tolerance.is_finite() {
            return Err(invalid_argument(
                "cluster tolerance must be finite and non-negative",
            ));
        }
        Ok(())
    }
}

impl Default for KMeansConfig {
    fn default() -> Self {
        Self {
            clusters: 8,
            max_iterations: 100,
            tolerance: 0.0001,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// One deterministic weighted k-means cluster.
pub struct DenseCluster {
    /// Stable cluster index.
    pub cluster_index: usize,
    /// Current centroid coordinates.
    pub centroid: Vec<f64>,
    /// Number of items represented by this value.
    pub count: u64,
    /// Sum of assigned point weights.
    pub weight_sum: f64,
    /// Weighted averages for assigned points.
    pub averages: Option<DenseAverages>,
    /// Coordinate bounds for assigned points.
    pub bounds: Option<DenseBounds>,
    /// Source point indices assigned to this cluster.
    pub point_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Result of deterministic weighted k-means clustering.
pub struct ClusterResult {
    /// Number of iterations performed before convergence or limit.
    pub iterations: usize,
    /// Clusters in stable index order.
    pub clusters: Vec<DenseCluster>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Numeric point used by chart-sized series binning.
pub struct NumericSeriesPoint {
    /// Source index used by callers to reattach application data.
    pub source_index: usize,
    /// The x coordinate.
    pub x: f64,
    /// The y coordinate.
    pub y: f64,
    /// Numeric metrics to aggregate with bins.
    pub metrics: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Query for binning an indexed numeric series.
pub struct NumericSeriesBinQuery {
    /// Visible x domain.
    pub x_domain: [f64; 2],
    /// Requested number of bins.
    pub target_bin_count: usize,
    /// Whether empty bins should be returned.
    #[serde(default)]
    pub include_empty_bins: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Query for display-oriented numeric series samples.
pub struct NumericSeriesQuery {
    /// Visible x domain.
    pub x_domain: [f64; 2],
    /// Requested number of bins.
    pub target_bin_count: usize,
    /// Value mode used for each returned sample.
    #[serde(default)]
    pub value_mode: NumericValueMode,
    /// Whether empty bins should be returned.
    #[serde(default)]
    pub include_empty_bins: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Supported numeric series sample value modes.
pub enum NumericValueMode {
    /// Average y value for points in the bin.
    #[default]
    Average,
    /// Point count for the bin.
    Count,
    /// Maximum y value for points in the bin.
    Max,
    /// Minimum y value for points in the bin.
    Min,
    /// Sum of y values for points in the bin.
    Sum,
}

impl NumericValueMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Average => "average",
            Self::Count => "count",
            Self::Max => "max",
            Self::Min => "min",
            Self::Sum => "sum",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Numeric point field used by histogram and heatmap aggregations.
pub enum NumericValueAccessor {
    /// Use the x coordinate.
    X,
    /// Use the y coordinate.
    #[default]
    Y,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Query for numeric histograms.
pub struct NumericHistogramQuery {
    /// Number of histogram buckets.
    pub bucket_count: usize,
    /// Whether empty buckets should be returned.
    #[serde(default = "default_true")]
    pub include_empty_buckets: bool,
    /// Optional x-domain filter applied before histogramming values.
    pub x_domain: Option<[f64; 2]>,
    /// Optional explicit value domain.
    pub value_domain: Option<[f64; 2]>,
    /// Point field to histogram.
    #[serde(default)]
    pub value_accessor: NumericValueAccessor,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Query for numeric heatmaps.
pub struct NumericHeatmapQuery {
    /// Number of x-axis bins.
    pub x_bin_count: usize,
    /// Visible x domain.
    pub x_domain: [f64; 2],
    /// Number of y-axis bins.
    pub y_bin_count: usize,
    /// Optional y domain. Derived from values when omitted.
    pub y_domain: Option<[f64; 2]>,
    /// Whether empty cells should be returned.
    #[serde(default = "default_true")]
    pub include_empty_cells: bool,
    /// Point field to use for the heatmap y value.
    #[serde(default)]
    pub value_accessor: NumericValueAccessor,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Bounds for an indexed numeric series.
pub struct NumericSeriesBounds {
    /// Minimum x value.
    pub min_x: f64,
    /// Maximum x value.
    pub max_x: f64,
    /// Minimum y value.
    pub min_y: f64,
    /// Maximum y value.
    pub max_y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// One chart-sized numeric series bin.
pub struct NumericSeriesBin {
    /// Average y value.
    pub average_y: Option<f64>,
    /// Source index of the first point in this bin.
    pub first_point_index: Option<usize>,
    /// Bin index.
    pub index: usize,
    /// Source index of the last point in this bin.
    pub last_point_index: Option<usize>,
    /// Maximum y value.
    pub max_y: Option<f64>,
    /// Aggregated metrics.
    pub metrics: BTreeMap<String, f64>,
    /// Minimum y value.
    pub min_y: Option<f64>,
    /// Number of points in this bin.
    pub point_count: u64,
    /// Sum of y values.
    pub sum_y: f64,
    /// Inclusive lower x boundary.
    pub x0: f64,
    /// Upper x boundary, except the last bin includes the domain end.
    pub x1: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Result of numeric series binning.
pub struct NumericSeriesBinResult {
    /// Bins produced for the query.
    pub bins: Vec<NumericSeriesBin>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// One display-oriented numeric series sample.
pub struct NumericSeriesSample {
    /// Average y value.
    pub average_y: Option<f64>,
    /// Source index of the first point in this sample.
    pub first_point_index: Option<usize>,
    /// Sample index.
    pub index: usize,
    /// Source index of the last point in this sample.
    pub last_point_index: Option<usize>,
    /// Maximum y value.
    pub max_y: Option<f64>,
    /// Aggregated metrics.
    pub metrics: BTreeMap<String, f64>,
    /// Minimum y value.
    pub min_y: Option<f64>,
    /// Number of points in this sample.
    pub point_count: u64,
    /// Sum of y values.
    pub sum_y: f64,
    /// Sample center x value.
    pub x: f64,
    /// Inclusive lower x boundary.
    pub x0: f64,
    /// Upper x boundary, except the last bin includes the domain end.
    pub x1: f64,
    /// Selected y value for the query's value mode.
    pub y: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Summary for numeric chart series output.
pub struct NumericSeriesSummary {
    /// Number of bins returned.
    pub bin_count: usize,
    /// Aggregated metrics across returned bins.
    pub metrics: BTreeMap<String, f64>,
    /// Number of points represented by returned bins.
    pub point_count: u64,
    /// Number of samples returned.
    pub sample_count: usize,
    /// Value mode used by the samples.
    pub value_mode: String,
    /// Normalized x domain used for the query.
    pub x_domain: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Display-oriented numeric series result.
pub struct NumericSeriesResult {
    /// Bins produced for the query.
    pub bins: Vec<NumericSeriesBin>,
    /// Samples produced for rendering.
    pub samples: Vec<NumericSeriesSample>,
    /// Series summary.
    pub summary: NumericSeriesSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// One numeric histogram bucket.
pub struct NumericHistogramBucket {
    /// Average value for points in this bucket.
    pub average_value: Option<f64>,
    /// Source index of the first point in this bucket.
    pub first_point_index: Option<usize>,
    /// Bucket index.
    pub index: usize,
    /// Source index of the last point in this bucket.
    pub last_point_index: Option<usize>,
    /// Maximum value.
    pub max_value: Option<f64>,
    /// Aggregated metrics.
    pub metrics: BTreeMap<String, f64>,
    /// Minimum value.
    pub min_value: Option<f64>,
    /// Number of points in this bucket.
    pub point_count: u64,
    /// Sum of values.
    pub sum_value: f64,
    /// Bucket center value.
    pub value: f64,
    /// Inclusive lower value boundary.
    pub value0: f64,
    /// Upper value boundary, except the last bucket includes the domain end.
    pub value1: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Numeric histogram summary.
pub struct NumericHistogramSummary {
    /// Number of buckets returned.
    pub bucket_count: usize,
    /// Aggregated metrics across returned buckets.
    pub metrics: BTreeMap<String, f64>,
    /// Number of points represented by returned buckets.
    pub point_count: u64,
    /// Value domain used for buckets.
    pub value_domain: [f64; 2],
    /// Optional normalized x-domain filter.
    pub x_domain: Option<[f64; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Numeric histogram result.
pub struct NumericHistogramResult {
    /// Buckets produced for the query.
    pub buckets: Vec<NumericHistogramBucket>,
    /// Histogram summary.
    pub summary: NumericHistogramSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// One numeric heatmap cell.
pub struct NumericHeatmapCell {
    /// Average value for points in this cell.
    pub average_value: Option<f64>,
    /// Source index of the first point in this cell.
    pub first_point_index: Option<usize>,
    /// Linear cell index.
    pub index: usize,
    /// Source index of the last point in this cell.
    pub last_point_index: Option<usize>,
    /// Aggregated metrics.
    pub metrics: BTreeMap<String, f64>,
    /// Number of points in this cell.
    pub point_count: u64,
    /// Sum of values.
    pub sum_value: f64,
    /// Normalized density value in the range 0..=1.
    pub value: f64,
    /// Cell center x value.
    pub x: f64,
    /// Inclusive lower x boundary.
    pub x0: f64,
    /// Upper x boundary, except the last x bin includes the domain end.
    pub x1: f64,
    /// X bin index.
    pub x_index: usize,
    /// Cell center y value.
    pub y: f64,
    /// Inclusive lower y boundary.
    pub y0: f64,
    /// Upper y boundary, except the last y bin includes the domain end.
    pub y1: f64,
    /// Y bin index.
    pub y_index: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Numeric heatmap summary.
pub struct NumericHeatmapSummary {
    /// Largest point count in any returned cell.
    pub max_cell_count: u64,
    /// Aggregated metrics across returned cells.
    pub metrics: BTreeMap<String, f64>,
    /// Number of points represented by returned cells.
    pub point_count: u64,
    /// Number of x-axis bins.
    pub x_bin_count: usize,
    /// Normalized x domain.
    pub x_domain: [f64; 2],
    /// Number of y-axis bins.
    pub y_bin_count: usize,
    /// Normalized y domain.
    pub y_domain: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Numeric heatmap result.
pub struct NumericHeatmapResult {
    /// Cells produced for the query.
    pub cells: Vec<NumericHeatmapCell>,
    /// Heatmap summary.
    pub summary: NumericHeatmapSummary,
}

#[derive(Debug, Clone, PartialEq)]
/// Sorted numeric series index for repeated chart viewport queries.
pub struct NumericSeriesIndex {
    points: Vec<NumericSeriesPoint>,
    metric_keys: Vec<String>,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl NumericSeriesIndex {
    /// Builds an index by dropping invalid records, normalizing metrics, and sorting by x.
    pub fn from_points(points: Vec<NumericSeriesPoint>) -> Result<Self> {
        let mut normalized_points = points
            .into_iter()
            .filter_map(|point| {
                if !point.x.is_finite() || !point.y.is_finite() {
                    return None;
                }

                Some(NumericSeriesPoint {
                    source_index: point.source_index,
                    x: point.x,
                    y: point.y,
                    metrics: point
                        .metrics
                        .into_iter()
                        .filter(|(_, value)| value.is_finite())
                        .collect(),
                })
            })
            .collect::<Vec<_>>();

        normalized_points.sort_by(|left, right| {
            left.x
                .total_cmp(&right.x)
                .then_with(|| left.source_index.cmp(&right.source_index))
        });

        let metric_keys = collect_numeric_series_metric_keys(&normalized_points);
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for point in &normalized_points {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }

        if normalized_points.is_empty() {
            min_x = 0.0;
            max_x = 0.0;
            min_y = 0.0;
            max_y = 0.0;
        }

        Ok(Self {
            points: normalized_points,
            metric_keys,
            min_x,
            max_x,
            min_y,
            max_y,
        })
    }

    /// Returns series bounds, or none when no finite points were indexed.
    pub fn bounds(&self) -> Option<NumericSeriesBounds> {
        (!self.points.is_empty()).then_some(NumericSeriesBounds {
            min_x: self.min_x,
            max_x: self.max_x,
            min_y: self.min_y,
            max_y: self.max_y,
        })
    }

    /// Bins points in the requested x domain.
    pub fn bin(&self, query: NumericSeriesBinQuery) -> Result<NumericSeriesBinResult> {
        if query.target_bin_count == 0 {
            return Err(invalid_argument("targetBinCount must be positive"));
        }
        if !query.x_domain[0].is_finite() || !query.x_domain[1].is_finite() {
            return Err(invalid_argument("xDomain bounds must be finite"));
        }

        let x0 = query.x_domain[0].min(query.x_domain[1]);
        let x1 = query.x_domain[0].max(query.x_domain[1]);
        let span = x1 - x0;
        let bin_width = if span > 0.0 {
            span / query.target_bin_count as f64
        } else {
            1.0
        };
        let mut bins = (0..query.target_bin_count)
            .map(|index| {
                NumericSeriesBinAccumulator::new(
                    index,
                    query.target_bin_count,
                    x0,
                    x1,
                    bin_width,
                    &self.metric_keys,
                )
            })
            .collect::<Vec<_>>();
        let start_index = self.lower_bound_by_x(x0);
        let end_index = self.upper_bound_by_x(x1);

        for point in &self.points[start_index..end_index] {
            let bin_index = if span > 0.0 {
                (((point.x - x0) / bin_width).floor() as usize).min(query.target_bin_count - 1)
            } else {
                0
            };
            bins[bin_index].push(point, &self.metric_keys);
        }

        Ok(NumericSeriesBinResult {
            bins: bins
                .into_iter()
                .filter(|bin| query.include_empty_bins || bin.point_count > 0)
                .map(NumericSeriesBinAccumulator::finish)
                .collect(),
        })
    }

    /// Returns display-oriented chart samples for points in the requested x domain.
    pub fn get_chart_series(&self, query: NumericSeriesQuery) -> Result<NumericSeriesResult> {
        let x_domain = normalize_numeric_domain(query.x_domain)?;
        let binned = self.bin(NumericSeriesBinQuery {
            x_domain,
            target_bin_count: query.target_bin_count,
            include_empty_bins: query.include_empty_bins,
        })?;
        let samples = binned
            .bins
            .iter()
            .map(|bin| numeric_series_sample(bin, query.value_mode))
            .collect::<Vec<_>>();

        Ok(NumericSeriesResult {
            summary: NumericSeriesSummary {
                bin_count: binned.bins.len(),
                metrics: sum_numeric_metrics(binned.bins.iter().map(|bin| &bin.metrics)),
                point_count: binned.bins.iter().map(|bin| bin.point_count).sum(),
                sample_count: samples.len(),
                value_mode: query.value_mode.as_str().to_string(),
                x_domain,
            },
            bins: binned.bins,
            samples,
        })
    }

    /// Returns a histogram for selected point values.
    pub fn get_histogram(&self, query: NumericHistogramQuery) -> Result<NumericHistogramResult> {
        if query.bucket_count == 0 {
            return Err(invalid_argument("bucketCount must be positive"));
        }

        let x_domain = query.x_domain.map(normalize_numeric_domain).transpose()?;
        let selected_points = self.points_in_x_domain(x_domain);
        let valued_points = selected_points
            .iter()
            .filter_map(|point| {
                numeric_point_value(point, query.value_accessor).map(|value| (point, value))
            })
            .collect::<Vec<_>>();
        let value_domain = match query.value_domain {
            Some(domain) => normalize_numeric_domain(domain)?,
            None => derive_numeric_value_domain(&valued_points).unwrap_or([0.0, 0.0]),
        };
        let mut buckets =
            create_numeric_histogram_buckets(query.bucket_count, value_domain, &self.metric_keys);

        for (point, value) in valued_points {
            if value < value_domain[0] || value > value_domain[1] {
                continue;
            }

            let bucket_index = numeric_bucket_index(value, value_domain, query.bucket_count);
            if let Some(bucket) = buckets.get_mut(bucket_index) {
                bucket.push(point, value, &self.metric_keys);
            }
        }

        let buckets = buckets
            .into_iter()
            .filter(|bucket| query.include_empty_buckets || bucket.point_count > 0)
            .map(NumericHistogramAccumulator::finish)
            .collect::<Vec<_>>();

        Ok(NumericHistogramResult {
            summary: NumericHistogramSummary {
                bucket_count: buckets.len(),
                metrics: sum_numeric_metrics(buckets.iter().map(|bucket| &bucket.metrics)),
                point_count: buckets.iter().map(|bucket| bucket.point_count).sum(),
                value_domain,
                x_domain,
            },
            buckets,
        })
    }

    /// Returns a heatmap for selected point values.
    pub fn get_heatmap(&self, query: NumericHeatmapQuery) -> Result<NumericHeatmapResult> {
        if query.x_bin_count == 0 || query.y_bin_count == 0 {
            return Err(invalid_argument("heatmap bin counts must be positive"));
        }

        let x_domain = normalize_numeric_domain(query.x_domain)?;
        let selected_points = self.points_in_x_domain(Some(x_domain));
        let valued_points = selected_points
            .iter()
            .filter_map(|point| {
                numeric_point_value(point, query.value_accessor).map(|value| (point, value))
            })
            .collect::<Vec<_>>();
        let y_domain = match query.y_domain {
            Some(domain) => normalize_numeric_domain(domain)?,
            None => derive_numeric_value_domain(&valued_points).unwrap_or([0.0, 0.0]),
        };
        let mut cells = create_numeric_heatmap_cells(
            query.x_bin_count,
            query.y_bin_count,
            x_domain,
            y_domain,
            &self.metric_keys,
        );

        for (point, value) in valued_points {
            if value < y_domain[0] || value > y_domain[1] {
                continue;
            }

            let x_index = numeric_bucket_index(point.x, x_domain, query.x_bin_count);
            let y_index = numeric_bucket_index(value, y_domain, query.y_bin_count);
            let cell_index = y_index * query.x_bin_count + x_index;
            if let Some(cell) = cells.get_mut(cell_index) {
                cell.push(point, value, &self.metric_keys);
            }
        }

        let max_cell_count = cells.iter().map(|cell| cell.point_count).max().unwrap_or(0);
        let cells = cells
            .into_iter()
            .filter(|cell| query.include_empty_cells || cell.point_count > 0)
            .map(|cell| cell.finish(max_cell_count))
            .collect::<Vec<_>>();

        Ok(NumericHeatmapResult {
            summary: NumericHeatmapSummary {
                max_cell_count,
                metrics: sum_numeric_metrics(cells.iter().map(|cell| &cell.metrics)),
                point_count: cells.iter().map(|cell| cell.point_count).sum(),
                x_bin_count: query.x_bin_count,
                x_domain,
                y_bin_count: query.y_bin_count,
                y_domain,
            },
            cells,
        })
    }

    fn lower_bound_by_x(&self, x: f64) -> usize {
        let mut low = 0;
        let mut high = self.points.len();

        while low < high {
            let middle = (low + high) / 2;

            if self.points[middle].x < x {
                low = middle + 1;
            } else {
                high = middle;
            }
        }

        low
    }

    fn upper_bound_by_x(&self, x: f64) -> usize {
        let mut low = 0;
        let mut high = self.points.len();

        while low < high {
            let middle = (low + high) / 2;

            if self.points[middle].x <= x {
                low = middle + 1;
            } else {
                high = middle;
            }
        }

        low
    }

    fn points_in_x_domain(&self, x_domain: Option<[f64; 2]>) -> &[NumericSeriesPoint] {
        match x_domain {
            Some(domain) => {
                let start = self.lower_bound_by_x(domain[0]);
                let end = self.upper_bound_by_x(domain[1]);
                &self.points[start..end]
            }
            None => &self.points,
        }
    }
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

/// Clusters dense points with deterministic k-means initialization.
pub fn k_means(points: &[DensePoint], config: KMeansConfig) -> Result<ClusterResult> {
    config.validate()?;
    let dimensions = validate_point_set(points)?;
    if config.clusters > points.len() {
        return Err(invalid_argument(
            "cluster count must not exceed the number of points",
        ));
    }

    let mut centroids = initial_centroids(points, config.clusters);
    let mut assignments = vec![usize::MAX; points.len()];
    let mut iterations = 0;

    for iteration in 1..=config.max_iterations {
        iterations = iteration;
        let changed = assign_points(points, &centroids, &mut assignments);
        let next_centroids = recompute_centroids(points, &assignments, &centroids, dimensions);
        let shift = centroid_shift(&centroids, &next_centroids);
        centroids = next_centroids;
        if !changed || shift <= config.tolerance {
            break;
        }
    }

    let mut accumulators = centroids
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, centroid)| ClusterAccumulator::new(index, centroid))
        .collect::<Vec<_>>();
    for (point_index, point) in points.iter().enumerate() {
        accumulators[assignments[point_index]].push(point_index, point);
    }

    Ok(ClusterResult {
        iterations,
        clusters: accumulators
            .into_iter()
            .map(ClusterAccumulator::finish)
            .collect(),
    })
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

#[derive(Debug, Clone)]
struct ClusterAccumulator {
    cluster_index: usize,
    centroid: Vec<f64>,
    count: u64,
    point_indices: Vec<usize>,
    summary: SummaryAccumulator,
}

impl ClusterAccumulator {
    fn new(cluster_index: usize, centroid: Vec<f64>) -> Self {
        Self {
            cluster_index,
            centroid,
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

    fn finish(self) -> DenseCluster {
        DenseCluster {
            cluster_index: self.cluster_index,
            centroid: self.centroid,
            count: self.count,
            weight_sum: self.summary.weight_sum,
            averages: self.summary.averages().ok(),
            bounds: self.summary.bounds().ok(),
            point_indices: self.point_indices,
        }
    }
}

#[derive(Debug, Clone)]
struct SummaryAccumulator {
    count: u64,
    weight_sum: f64,
    coordinate_stats: Vec<RunningStats>,
    value_stats: RunningStats,
    has_values: bool,
    min: Vec<f64>,
    max: Vec<f64>,
}

impl SummaryAccumulator {
    fn new() -> Self {
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

    fn push(&mut self, point: &DensePoint) {
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

    fn averages(&self) -> Result<DenseAverages> {
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

    fn bounds(&self) -> Result<DenseBounds> {
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

fn initial_centroids(points: &[DensePoint], clusters: usize) -> Vec<Vec<f64>> {
    if clusters == 1 {
        return vec![points[0].coordinates.clone()];
    }
    let last_index = points.len() - 1;
    (0..clusters)
        .map(|cluster| {
            let index = (cluster * last_index + (clusters - 1) / 2) / (clusters - 1);
            points[index].coordinates.clone()
        })
        .collect()
}

fn assign_points(points: &[DensePoint], centroids: &[Vec<f64>], assignments: &mut [usize]) -> bool {
    let mut changed = false;
    for (point_index, point) in points.iter().enumerate() {
        let mut best_cluster = 0;
        let mut best_distance = f64::INFINITY;
        for (cluster_index, centroid) in centroids.iter().enumerate() {
            let distance = squared_distance(&point.coordinates, centroid);
            if distance < best_distance {
                best_distance = distance;
                best_cluster = cluster_index;
            }
        }
        if assignments[point_index] != best_cluster {
            assignments[point_index] = best_cluster;
            changed = true;
        }
    }
    changed
}

fn recompute_centroids(
    points: &[DensePoint],
    assignments: &[usize],
    centroids: &[Vec<f64>],
    dimensions: usize,
) -> Vec<Vec<f64>> {
    let mut next = vec![vec![0.0; dimensions]; centroids.len()];
    let mut weights = vec![0.0; centroids.len()];

    for (point, cluster) in points.iter().zip(assignments) {
        weights[*cluster] += point.weight;
        for (value, coordinate) in next[*cluster].iter_mut().zip(&point.coordinates) {
            *value += point.weight * coordinate;
        }
    }

    for (cluster, centroid) in next.iter_mut().enumerate() {
        if weights[cluster] == 0.0 {
            *centroid = centroids[cluster].clone();
            continue;
        }
        for value in centroid {
            *value /= weights[cluster];
        }
    }
    next
}

fn centroid_shift(left: &[Vec<f64>], right: &[Vec<f64>]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| squared_distance(left, right).sqrt())
        .fold(0.0, f64::max)
}

fn squared_distance(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let delta = left - right;
            delta * delta
        })
        .sum()
}

fn collect_numeric_series_metric_keys(points: &[NumericSeriesPoint]) -> Vec<String> {
    let mut keys = BTreeMap::<String, ()>::new();
    for point in points {
        for key in point.metrics.keys() {
            keys.insert(key.clone(), ());
        }
    }
    keys.into_keys().collect()
}

fn default_true() -> bool {
    true
}

fn normalize_numeric_domain(domain: [f64; 2]) -> Result<[f64; 2]> {
    if !domain[0].is_finite() || !domain[1].is_finite() {
        return Err(invalid_argument("numeric domain bounds must be finite"));
    }

    Ok(if domain[0] <= domain[1] {
        domain
    } else {
        [domain[1], domain[0]]
    })
}

fn numeric_bin_width(domain: [f64; 2], bin_count: usize) -> f64 {
    let span = domain[1] - domain[0];

    if span > 0.0 {
        span / bin_count as f64
    } else {
        1.0
    }
}

fn numeric_bucket_index(value: f64, domain: [f64; 2], bucket_count: usize) -> usize {
    let width = numeric_bin_width(domain, bucket_count);
    let index = ((value - domain[0]) / width).floor() as isize;

    index.clamp(0, bucket_count.saturating_sub(1) as isize) as usize
}

fn numeric_point_value(point: &NumericSeriesPoint, accessor: NumericValueAccessor) -> Option<f64> {
    let value = match accessor {
        NumericValueAccessor::X => point.x,
        NumericValueAccessor::Y => point.y,
    };

    value.is_finite().then_some(value)
}

fn derive_numeric_value_domain(valued_points: &[(&NumericSeriesPoint, f64)]) -> Option<[f64; 2]> {
    let (_, first_value) = valued_points.first()?;
    let mut min = *first_value;
    let mut max = *first_value;

    for (_, value) in valued_points {
        min = min.min(*value);
        max = max.max(*value);
    }

    Some([min, max])
}

fn sum_numeric_metrics<'a>(
    metrics: impl IntoIterator<Item = &'a BTreeMap<String, f64>>,
) -> BTreeMap<String, f64> {
    let mut totals = BTreeMap::new();
    for metric_record in metrics {
        for (key, value) in metric_record {
            *totals.entry(key.clone()).or_insert(0.0) += *value;
        }
    }
    totals
}

fn numeric_series_sample(
    bin: &NumericSeriesBin,
    value_mode: NumericValueMode,
) -> NumericSeriesSample {
    NumericSeriesSample {
        average_y: bin.average_y,
        first_point_index: bin.first_point_index,
        index: bin.index,
        last_point_index: bin.last_point_index,
        max_y: bin.max_y,
        metrics: bin.metrics.clone(),
        min_y: bin.min_y,
        point_count: bin.point_count,
        sum_y: bin.sum_y,
        x: bin.x0 + (bin.x1 - bin.x0) / 2.0,
        x0: bin.x0,
        x1: bin.x1,
        y: match value_mode {
            NumericValueMode::Average => bin.average_y,
            NumericValueMode::Count => Some(bin.point_count as f64),
            NumericValueMode::Max => bin.max_y,
            NumericValueMode::Min => bin.min_y,
            NumericValueMode::Sum => (bin.point_count > 0).then_some(bin.sum_y),
        },
    }
}

#[derive(Debug, Clone)]
struct NumericSeriesBinAccumulator {
    average_y: Option<f64>,
    first_point_index: Option<usize>,
    index: usize,
    last_point_index: Option<usize>,
    max_y: Option<f64>,
    metrics: BTreeMap<String, f64>,
    min_y: Option<f64>,
    point_count: u64,
    sum_y: f64,
    x0: f64,
    x1: f64,
}

impl NumericSeriesBinAccumulator {
    fn new(
        index: usize,
        bin_count: usize,
        x0: f64,
        x1: f64,
        bin_width: f64,
        metric_keys: &[String],
    ) -> Self {
        let bin_x0 = x0 + index as f64 * bin_width;
        Self {
            average_y: None,
            first_point_index: None,
            index,
            last_point_index: None,
            max_y: None,
            metrics: metric_keys.iter().map(|key| (key.clone(), 0.0)).collect(),
            min_y: None,
            point_count: 0,
            sum_y: 0.0,
            x0: bin_x0,
            x1: if index + 1 == bin_count {
                x1
            } else {
                bin_x0 + bin_width
            },
        }
    }

    fn push(&mut self, point: &NumericSeriesPoint, metric_keys: &[String]) {
        self.first_point_index.get_or_insert(point.source_index);
        self.last_point_index = Some(point.source_index);
        self.point_count += 1;
        self.sum_y += point.y;
        self.average_y = Some(self.sum_y / self.point_count as f64);
        self.min_y = Some(self.min_y.map_or(point.y, |min_y| min_y.min(point.y)));
        self.max_y = Some(self.max_y.map_or(point.y, |max_y| max_y.max(point.y)));

        for metric_key in metric_keys {
            *self.metrics.entry(metric_key.clone()).or_insert(0.0) +=
                point.metrics.get(metric_key).copied().unwrap_or(0.0);
        }
    }

    fn finish(self) -> NumericSeriesBin {
        NumericSeriesBin {
            average_y: self.average_y,
            first_point_index: self.first_point_index,
            index: self.index,
            last_point_index: self.last_point_index,
            max_y: self.max_y,
            metrics: self.metrics,
            min_y: self.min_y,
            point_count: self.point_count,
            sum_y: self.sum_y,
            x0: self.x0,
            x1: self.x1,
        }
    }
}

#[derive(Debug, Clone)]
struct NumericHistogramAccumulator {
    average_value: Option<f64>,
    first_point_index: Option<usize>,
    index: usize,
    last_point_index: Option<usize>,
    max_value: Option<f64>,
    metrics: BTreeMap<String, f64>,
    min_value: Option<f64>,
    point_count: u64,
    sum_value: f64,
    value: f64,
    value0: f64,
    value1: f64,
}

impl NumericHistogramAccumulator {
    fn push(&mut self, point: &NumericSeriesPoint, value: f64, metric_keys: &[String]) {
        self.first_point_index.get_or_insert(point.source_index);
        self.last_point_index = Some(point.source_index);
        self.point_count += 1;
        self.sum_value += value;
        self.average_value = Some(self.sum_value / self.point_count as f64);
        self.min_value = Some(
            self.min_value
                .map_or(value, |min_value| min_value.min(value)),
        );
        self.max_value = Some(
            self.max_value
                .map_or(value, |max_value| max_value.max(value)),
        );

        for metric_key in metric_keys {
            *self.metrics.entry(metric_key.clone()).or_insert(0.0) +=
                point.metrics.get(metric_key).copied().unwrap_or(0.0);
        }
    }

    fn finish(self) -> NumericHistogramBucket {
        NumericHistogramBucket {
            average_value: self.average_value,
            first_point_index: self.first_point_index,
            index: self.index,
            last_point_index: self.last_point_index,
            max_value: self.max_value,
            metrics: self.metrics,
            min_value: self.min_value,
            point_count: self.point_count,
            sum_value: self.sum_value,
            value: self.value,
            value0: self.value0,
            value1: self.value1,
        }
    }
}

fn create_numeric_histogram_buckets(
    bucket_count: usize,
    value_domain: [f64; 2],
    metric_keys: &[String],
) -> Vec<NumericHistogramAccumulator> {
    let width = numeric_bin_width(value_domain, bucket_count);

    (0..bucket_count)
        .map(|index| {
            let value0 = value_domain[0] + index as f64 * width;
            NumericHistogramAccumulator {
                average_value: None,
                first_point_index: None,
                index,
                last_point_index: None,
                max_value: None,
                metrics: metric_keys.iter().map(|key| (key.clone(), 0.0)).collect(),
                min_value: None,
                point_count: 0,
                sum_value: 0.0,
                value: value0 + width / 2.0,
                value0,
                value1: if index + 1 == bucket_count {
                    value_domain[1]
                } else {
                    value0 + width
                },
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
struct NumericHeatmapAccumulator {
    average_value: Option<f64>,
    first_point_index: Option<usize>,
    index: usize,
    last_point_index: Option<usize>,
    metrics: BTreeMap<String, f64>,
    point_count: u64,
    sum_value: f64,
    x: f64,
    x0: f64,
    x1: f64,
    x_index: usize,
    y: f64,
    y0: f64,
    y1: f64,
    y_index: usize,
}

impl NumericHeatmapAccumulator {
    fn push(&mut self, point: &NumericSeriesPoint, value: f64, metric_keys: &[String]) {
        self.first_point_index.get_or_insert(point.source_index);
        self.last_point_index = Some(point.source_index);
        self.point_count += 1;
        self.sum_value += value;
        self.average_value = Some(self.sum_value / self.point_count as f64);

        for metric_key in metric_keys {
            *self.metrics.entry(metric_key.clone()).or_insert(0.0) +=
                point.metrics.get(metric_key).copied().unwrap_or(0.0);
        }
    }

    fn finish(self, max_cell_count: u64) -> NumericHeatmapCell {
        NumericHeatmapCell {
            average_value: self.average_value,
            first_point_index: self.first_point_index,
            index: self.index,
            last_point_index: self.last_point_index,
            metrics: self.metrics,
            point_count: self.point_count,
            sum_value: self.sum_value,
            value: if max_cell_count > 0 {
                self.point_count as f64 / max_cell_count as f64
            } else {
                0.0
            },
            x: self.x,
            x0: self.x0,
            x1: self.x1,
            x_index: self.x_index,
            y: self.y,
            y0: self.y0,
            y1: self.y1,
            y_index: self.y_index,
        }
    }
}

fn create_numeric_heatmap_cells(
    x_bin_count: usize,
    y_bin_count: usize,
    x_domain: [f64; 2],
    y_domain: [f64; 2],
    metric_keys: &[String],
) -> Vec<NumericHeatmapAccumulator> {
    let x_width = numeric_bin_width(x_domain, x_bin_count);
    let y_width = numeric_bin_width(y_domain, y_bin_count);

    (0..x_bin_count * y_bin_count)
        .map(|index| {
            let x_index = index % x_bin_count;
            let y_index = index / x_bin_count;
            let x0 = x_domain[0] + x_index as f64 * x_width;
            let y0 = y_domain[0] + y_index as f64 * y_width;

            NumericHeatmapAccumulator {
                average_value: None,
                first_point_index: None,
                index,
                last_point_index: None,
                metrics: metric_keys.iter().map(|key| (key.clone(), 0.0)).collect(),
                point_count: 0,
                sum_value: 0.0,
                x: x0 + x_width / 2.0,
                x0,
                x1: if x_index + 1 == x_bin_count {
                    x_domain[1]
                } else {
                    x0 + x_width
                },
                x_index,
                y: y0 + y_width / 2.0,
                y0,
                y1: if y_index + 1 == y_bin_count {
                    y_domain[1]
                } else {
                    y0 + y_width
                },
                y_index,
            }
        })
        .collect()
}

fn validate_point_set(points: &[DensePoint]) -> Result<usize> {
    if points.is_empty() {
        return Err(invalid_argument("dense point set must not be empty"));
    }
    let dimensions = points[0].dimensions();
    for point in points {
        point.validate()?;
        if point.dimensions() != dimensions {
            return Err(invalid_argument(
                "dense point set coordinates must have the same dimensions",
            ));
        }
    }
    Ok(dimensions)
}

fn validate_coordinates(coordinates: &[f64], name: &str) -> Result<()> {
    if coordinates.is_empty() {
        return Err(invalid_argument(format!("{name} must not be empty")));
    }
    if coordinates.iter().any(|coordinate| !coordinate.is_finite()) {
        return Err(invalid_argument(format!("{name} must be finite")));
    }
    Ok(())
}

fn invalid_argument(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(coordinates: impl Into<Vec<f64>>) -> DensePoint {
        DensePoint::new(coordinates).unwrap()
    }

    fn assert_close(left: f64, right: f64) {
        assert!((left - right).abs() < 1.0e-12, "{left} != {right}");
    }

    #[test]
    fn weighted_averages_include_coordinates_and_values() {
        let points = [
            point([0.0, 2.0])
                .weighted(1.0)
                .unwrap()
                .valued(10.0)
                .unwrap(),
            point([10.0, 6.0])
                .weighted(3.0)
                .unwrap()
                .valued(20.0)
                .unwrap(),
        ];

        let averages = dense_averages(&points).unwrap();

        assert_eq!(averages.count, 2);
        assert_eq!(averages.weight_sum, 4.0);
        assert_eq!(averages.coordinates, vec![7.5, 5.0]);
        assert_eq!(averages.value, Some(17.5));
    }

    #[test]
    fn dense_summary_matches_weighted_averages_and_bounds() {
        let points = [
            point([0.0, 2.0])
                .weighted(1.0)
                .unwrap()
                .valued(10.0)
                .unwrap(),
            point([10.0, 6.0])
                .weighted(3.0)
                .unwrap()
                .valued(20.0)
                .unwrap(),
        ];

        let summary = dense_summary(&points).unwrap();

        assert_eq!(summary.count, 2);
        assert_eq!(summary.dimensions, 2);
        assert_eq!(summary.weight_sum, 4.0);
        assert_eq!(summary.coordinate_stats.len(), 2);
        assert_eq!(summary.coordinate_stats[0].mean, Some(7.5));
        assert_eq!(summary.coordinate_stats[1].mean, Some(5.0));
        assert_eq!(summary.coordinate_stats[0].min, Some(0.0));
        assert_eq!(summary.coordinate_stats[0].max, Some(10.0));
        assert_eq!(summary.bounds.min, vec![0.0, 2.0]);
        assert_eq!(summary.bounds.max, vec![10.0, 6.0]);
        assert_eq!(
            summary.value_stats.as_ref().and_then(|stats| stats.mean),
            Some(17.5)
        );
        assert_eq!(
            summary.value_stats.as_ref().map(|stats| stats.weight_sum),
            Some(4.0)
        );
    }

    #[test]
    fn dataset_summary_wraps_point_summary() {
        let dataset = DenseDataset::from_points([
            point([1.0, 2.0]),
            point([3.0, 4.0]).weighted(2.0).unwrap(),
        ])
        .unwrap();

        let summary = dataset.summary().unwrap();

        assert_eq!(summary.count, 2);
        assert_close(summary.coordinate_stats[0].mean.unwrap(), 7.0 / 3.0);
        assert_close(summary.coordinate_stats[1].mean.unwrap(), 10.0 / 3.0);
    }

    #[test]
    fn dataset_exposes_covariance_and_pca_helpers() {
        let dataset =
            DenseDataset::from_points([point([1.0, 1.0]), point([2.0, 2.0]), point([3.0, 3.0])])
                .unwrap();
        let covariance = dataset.covariance_matrix().unwrap();
        let pca = dataset.principal_components(1).unwrap();
        assert_eq!(covariance.matrix.shape().rows, 2);
        assert_eq!(pca.components().shape().rows, 1);
    }

    #[test]
    fn covariance_uses_point_weights() {
        let dataset = DenseDataset::from_points([
            point([0.0]).weighted(1.0).unwrap(),
            point([10.0]).weighted(3.0).unwrap(),
        ])
        .unwrap();

        let covariance = dataset.covariance_matrix().unwrap();

        assert_eq!(covariance.count, 2);
        assert_eq!(covariance.weight_sum, 4.0);
        assert_eq!(covariance.means, vec![7.5]);
        assert_close(covariance.matrix.values()[0] as f64, 18.75);
    }

    #[test]
    fn dense_summary_omits_value_stats_when_points_have_no_values() {
        let summary = dense_summary(&[point([1.0]), point([3.0])]).unwrap();
        assert!(summary.value_stats.is_none());
    }

    #[test]
    fn dense_summary_rejects_empty_point_sets() {
        assert!(dense_summary(&[]).is_err());
    }

    #[test]
    fn grid_buckets_floor_coordinates_by_dimension() {
        let points = vec![point([0.2, 1.9]), point([1.1, 1.2]), point([-0.1, 0.2])];
        let grid = BucketGrid::uniform(2, 1.0).unwrap();

        let buckets = bucket_points(&points, &grid).unwrap();

        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0].key.indices, vec![-1, 0]);
        assert_eq!(buckets[0].point_indices, vec![2]);
        assert_eq!(buckets[1].key.indices, vec![0, 1]);
        assert_eq!(buckets[2].key.indices, vec![1, 1]);
    }

    #[test]
    fn dataset_rejects_mismatched_dimensions() {
        let mut dataset = DenseDataset::new();
        dataset.push(point([1.0, 2.0])).unwrap();

        let err = dataset.push(point([1.0])).unwrap_err();

        assert!(err.to_string().contains("same dimensions"));
    }

    #[test]
    fn k_means_splits_dense_groups_deterministically() {
        let points = vec![
            point([0.0, 0.0]),
            point([0.2, 0.1]),
            point([9.0, 9.0]),
            point([10.0, 10.0]),
        ];

        let result = k_means(
            &points,
            KMeansConfig {
                clusters: 2,
                max_iterations: 20,
                tolerance: 0.0,
            },
        )
        .unwrap();

        assert_eq!(result.clusters.len(), 2);
        assert_eq!(result.clusters[0].point_indices, vec![0, 1]);
        assert_eq!(result.clusters[1].point_indices, vec![2, 3]);
        assert_eq!(result.clusters[0].centroid, vec![0.1, 0.05]);
        assert_eq!(result.clusters[1].centroid, vec![9.5, 9.5]);
    }

    #[test]
    fn numeric_series_index_sorts_filters_and_bins_points() {
        let index = NumericSeriesIndex::from_points(vec![
            NumericSeriesPoint {
                source_index: 0,
                x: 10.0,
                y: 6.0,
                metrics: BTreeMap::from([("count".to_string(), 1.0)]),
            },
            NumericSeriesPoint {
                source_index: 1,
                x: f64::NAN,
                y: 100.0,
                metrics: BTreeMap::from([("count".to_string(), 100.0)]),
            },
            NumericSeriesPoint {
                source_index: 2,
                x: 0.0,
                y: 2.0,
                metrics: BTreeMap::from([
                    ("count".to_string(), 1.0),
                    ("invalid".to_string(), f64::INFINITY),
                ]),
            },
            NumericSeriesPoint {
                source_index: 3,
                x: 1.0,
                y: 4.0,
                metrics: BTreeMap::from([("count".to_string(), 1.0)]),
            },
        ])
        .unwrap();

        assert_eq!(
            index.bounds(),
            Some(NumericSeriesBounds {
                min_x: 0.0,
                max_x: 10.0,
                min_y: 2.0,
                max_y: 6.0,
            })
        );

        let result = index
            .bin(NumericSeriesBinQuery {
                x_domain: [10.0, 0.0],
                target_bin_count: 2,
                include_empty_bins: true,
            })
            .unwrap();

        assert_eq!(result.bins.len(), 2);
        assert_eq!(result.bins[0].point_count, 2);
        assert_eq!(result.bins[0].first_point_index, Some(2));
        assert_eq!(result.bins[0].last_point_index, Some(3));
        assert_eq!(result.bins[0].metrics.get("count"), Some(&2.0));
        assert!(!result.bins[0].metrics.contains_key("invalid"));
        assert_eq!(result.bins[1].point_count, 1);
        assert_eq!(result.bins[1].first_point_index, Some(0));
    }

    #[test]
    fn numeric_series_index_handles_empty_and_empty_bins() {
        let index = NumericSeriesIndex::from_points(Vec::new()).unwrap();

        assert_eq!(index.bounds(), None);

        let result = index
            .bin(NumericSeriesBinQuery {
                x_domain: [0.0, 0.0],
                target_bin_count: 2,
                include_empty_bins: true,
            })
            .unwrap();

        assert_eq!(result.bins.len(), 2);
        assert_eq!(result.bins[0].point_count, 0);
        assert_eq!(result.bins[0].x0, 0.0);
        assert_eq!(result.bins[0].x1, 1.0);
        assert_eq!(result.bins[1].x0, 1.0);
        assert_eq!(result.bins[1].x1, 0.0);
    }

    #[test]
    fn numeric_series_index_creates_chart_series_histograms_and_heatmaps() {
        let index = NumericSeriesIndex::from_points(vec![
            NumericSeriesPoint {
                source_index: 0,
                x: 0.0,
                y: 2.0,
                metrics: BTreeMap::from([("count".to_string(), 1.0)]),
            },
            NumericSeriesPoint {
                source_index: 1,
                x: 1.0,
                y: 4.0,
                metrics: BTreeMap::from([("count".to_string(), 1.0)]),
            },
            NumericSeriesPoint {
                source_index: 2,
                x: 10.0,
                y: 6.0,
                metrics: BTreeMap::from([("count".to_string(), 1.0)]),
            },
        ])
        .unwrap();

        let series = index
            .get_chart_series(NumericSeriesQuery {
                x_domain: [0.0, 10.0],
                target_bin_count: 2,
                value_mode: NumericValueMode::Sum,
                include_empty_bins: true,
            })
            .unwrap();

        assert_eq!(
            series
                .samples
                .iter()
                .map(|sample| sample.y)
                .collect::<Vec<_>>(),
            vec![Some(6.0), Some(6.0)]
        );
        assert_eq!(series.summary.metrics.get("count"), Some(&3.0));

        let histogram = index
            .get_histogram(NumericHistogramQuery {
                bucket_count: 2,
                include_empty_buckets: true,
                x_domain: Some([0.0, 10.0]),
                value_domain: None,
                value_accessor: NumericValueAccessor::Y,
            })
            .unwrap();

        assert_eq!(
            histogram
                .buckets
                .iter()
                .map(|bucket| bucket.point_count)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );

        let heatmap = index
            .get_heatmap(NumericHeatmapQuery {
                include_empty_cells: true,
                x_bin_count: 2,
                x_domain: [0.0, 10.0],
                y_bin_count: 2,
                y_domain: Some([0.0, 10.0]),
                value_accessor: NumericValueAccessor::Y,
            })
            .unwrap();

        assert_eq!(
            heatmap
                .cells
                .iter()
                .map(|cell| cell.point_count)
                .collect::<Vec<_>>(),
            vec![2, 0, 0, 1]
        );
    }

    #[test]
    fn invalid_points_are_rejected() {
        assert!(DensePoint::new([f64::NAN]).is_err());
        assert!(point([1.0]).weighted(0.0).is_err());
        assert!(point([1.0]).valued(f64::INFINITY).is_err());
    }
}
