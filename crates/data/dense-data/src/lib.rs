#![doc = include_str!("../README.md")]

mod bucket;
mod cluster;
mod numeric;
mod point;
mod summary;
pub mod surface;
mod validation;

pub use bucket::{bucket_points, BucketGrid, BucketKey, DenseBucket};
pub use cluster::{k_means, ClusterResult, DenseCluster, KMeansConfig};
pub use numeric::{
    NumericHeatmapCell, NumericHeatmapQuery, NumericHeatmapResult, NumericHeatmapSummary,
    NumericHistogramBucket, NumericHistogramQuery, NumericHistogramResult, NumericHistogramSummary,
    NumericSeriesBin, NumericSeriesBinQuery, NumericSeriesBinResult, NumericSeriesBounds,
    NumericSeriesIndex, NumericSeriesPoint, NumericSeriesQuery, NumericSeriesResult,
    NumericSeriesSample, NumericSeriesSummary, NumericValueAccessor, NumericValueMode,
};
pub use point::{DenseDataset, DensePoint};
pub use summary::{
    dense_averages, dense_bounds, dense_summary, DenseAverages, DenseBounds, DenseSummary,
};

#[cfg(test)]
mod tests;
