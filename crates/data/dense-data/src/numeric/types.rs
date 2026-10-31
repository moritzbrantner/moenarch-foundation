//! Public query and result types for numeric series aggregation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    pub(super) fn as_str(self) -> &'static str {
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

fn default_true() -> bool {
    true
}
