//! Sorted index answering repeated chart viewport queries.

use std::collections::BTreeMap;

use media_core::Result;

use super::accumulators::{
    create_numeric_heatmap_cells, create_numeric_histogram_buckets, numeric_bin_width,
    NumericHistogramAccumulator, NumericSeriesBinAccumulator,
};
use super::types::{
    NumericHeatmapQuery, NumericHeatmapResult, NumericHeatmapSummary, NumericHistogramQuery,
    NumericHistogramResult, NumericHistogramSummary, NumericSeriesBin, NumericSeriesBinQuery,
    NumericSeriesBinResult, NumericSeriesBounds, NumericSeriesPoint, NumericSeriesQuery,
    NumericSeriesResult, NumericSeriesSample, NumericSeriesSummary, NumericValueAccessor,
    NumericValueMode,
};
use crate::validation::invalid_argument;

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

fn collect_numeric_series_metric_keys(points: &[NumericSeriesPoint]) -> Vec<String> {
    let mut keys = BTreeMap::<String, ()>::new();
    for point in points {
        for key in point.metrics.keys() {
            keys.insert(key.clone(), ());
        }
    }
    keys.into_keys().collect()
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
