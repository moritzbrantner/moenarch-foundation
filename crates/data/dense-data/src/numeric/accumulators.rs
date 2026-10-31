//! Per-bin running totals behind the numeric series aggregations.

use std::collections::BTreeMap;

use super::types::{
    NumericHeatmapCell, NumericHistogramBucket, NumericSeriesBin, NumericSeriesPoint,
};

pub(super) fn numeric_bin_width(domain: [f64; 2], bin_count: usize) -> f64 {
    let span = domain[1] - domain[0];

    if span > 0.0 {
        span / bin_count as f64
    } else {
        1.0
    }
}

#[derive(Debug, Clone)]
pub(super) struct NumericSeriesBinAccumulator {
    average_y: Option<f64>,
    first_point_index: Option<usize>,
    index: usize,
    last_point_index: Option<usize>,
    max_y: Option<f64>,
    metrics: BTreeMap<String, f64>,
    min_y: Option<f64>,
    pub(super) point_count: u64,
    sum_y: f64,
    x0: f64,
    x1: f64,
}

impl NumericSeriesBinAccumulator {
    pub(super) fn new(
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

    pub(super) fn push(&mut self, point: &NumericSeriesPoint, metric_keys: &[String]) {
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

    pub(super) fn finish(self) -> NumericSeriesBin {
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
pub(super) struct NumericHistogramAccumulator {
    average_value: Option<f64>,
    first_point_index: Option<usize>,
    index: usize,
    last_point_index: Option<usize>,
    max_value: Option<f64>,
    metrics: BTreeMap<String, f64>,
    min_value: Option<f64>,
    pub(super) point_count: u64,
    sum_value: f64,
    value: f64,
    value0: f64,
    value1: f64,
}

impl NumericHistogramAccumulator {
    pub(super) fn push(&mut self, point: &NumericSeriesPoint, value: f64, metric_keys: &[String]) {
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

    pub(super) fn finish(self) -> NumericHistogramBucket {
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

pub(super) fn create_numeric_histogram_buckets(
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
pub(super) struct NumericHeatmapAccumulator {
    average_value: Option<f64>,
    first_point_index: Option<usize>,
    index: usize,
    last_point_index: Option<usize>,
    metrics: BTreeMap<String, f64>,
    pub(super) point_count: u64,
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
    pub(super) fn push(&mut self, point: &NumericSeriesPoint, value: f64, metric_keys: &[String]) {
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

    pub(super) fn finish(self, max_cell_count: u64) -> NumericHeatmapCell {
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

pub(super) fn create_numeric_heatmap_cells(
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
