//! Sorted numeric series indexing and chart-sized aggregation.

mod accumulators;
mod index;
mod types;

pub use index::NumericSeriesIndex;
pub use types::{
    NumericHeatmapCell, NumericHeatmapQuery, NumericHeatmapResult, NumericHeatmapSummary,
    NumericHistogramBucket, NumericHistogramQuery, NumericHistogramResult, NumericHistogramSummary,
    NumericSeriesBin, NumericSeriesBinQuery, NumericSeriesBinResult, NumericSeriesBounds,
    NumericSeriesPoint, NumericSeriesQuery, NumericSeriesResult, NumericSeriesSample,
    NumericSeriesSummary, NumericValueAccessor, NumericValueMode,
};
