# dense-data

Deterministic dense point datasets, bucketing, and clustering for `moritzbrantner-video-analysis`.

## Highlights

- Weighted dense point summaries with per-dimension stats
- `moritzbrantner-math-linear` matrix export for dense point coordinates
- `moritzbrantner-math-statistics` covariance and PCA helpers
- Deterministic fixed-grid bucketing
- Deterministic k-means clustering
- Dataset and point-set helpers for tables, charts, and media-derived features

## Example

```rust,no_run
use dense_data::{
    BucketGrid, DenseDataset, DensePoint, KMeansConfig, NumericHeatmapQuery,
    NumericHistogramQuery, NumericSeriesIndex, NumericSeriesPoint, NumericSeriesQuery,
    NumericValueAccessor, NumericValueMode,
};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = DenseDataset::from_points([
        DensePoint::new([0.0, 0.0])?.named("left"),
        DensePoint::new([1.0, 1.0])?
            .named("right")
            .weighted(2.0)?
            .valued(4.0)?,
    ])?;

    let summary = dataset.summary()?;
    assert_eq!(summary.dimensions, 2);
    assert_eq!(summary.coordinate_stats[0].mean, Some(2.0 / 3.0));

    let buckets = dataset.buckets(&BucketGrid::uniform(2, 1.0)?)?;
    assert_eq!(buckets.len(), 2);

    let covariance = dataset.covariance_matrix()?;
    assert_eq!(covariance.matrix.shape().rows, 2);

    let clusters = dataset.k_means(KMeansConfig::new(2)?)?;
    assert_eq!(clusters.clusters.len(), 2);

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
    ])?;

    let series = index.get_chart_series(NumericSeriesQuery {
        x_domain: [10.0, 0.0],
        target_bin_count: 2,
        value_mode: NumericValueMode::Sum,
        include_empty_bins: true,
    })?;
    assert_eq!(
        series.samples.iter().map(|sample| sample.y).collect::<Vec<_>>(),
        vec![Some(6.0), Some(6.0)]
    );

    let histogram = index.get_histogram(NumericHistogramQuery {
        bucket_count: 2,
        include_empty_buckets: true,
        x_domain: Some([0.0, 10.0]),
        value_domain: None,
        value_accessor: NumericValueAccessor::Y,
    })?;
    assert_eq!(
        histogram
            .buckets
            .iter()
            .map(|bucket| bucket.point_count)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );

    let heatmap = index.get_heatmap(NumericHeatmapQuery {
        x_bin_count: 2,
        x_domain: [0.0, 10.0],
        y_bin_count: 2,
        y_domain: Some([0.0, 10.0]),
        include_empty_cells: true,
        value_accessor: NumericValueAccessor::Y,
    })?;
    assert_eq!(heatmap.summary.max_cell_count, 2);

    Ok(())
}
```

## Numeric series behavior

`NumericSeriesIndex` drops non-finite x/y values, drops non-finite metric
values, and sorts remaining points by x coordinate and source index. Query
domains are normalized, so `[10.0, 0.0]` is treated as `[0.0, 10.0]`.

`get_chart_series` returns display-sized samples backed by the same bins as
`bin`. Empty bins are included only when requested. `get_histogram` can derive
its value domain from selected points or use an explicit domain, and empty
buckets are included by default. `get_heatmap` uses deterministic x/y binning,
derives the y domain when omitted, includes empty cells by default, and reports
cell density normalized against the largest cell count.

Dense point weights are finite positive values. They affect coordinate
averages, optional scalar value summaries, covariance, and k-means assignment
updates. K-means is deterministic for the same input: initial centroids are
chosen from the validated point order and clusters are returned in stable index
order.

## Package surface

Primary workflow: `summarizeDensePoints`.

Workflow operations:

- `summarizeDensePoints`: Returns weighted per-dimension stats, bounds, and optional scalar value stats.
- `bucketDensePoints`: Groups dense points into deterministic fixed-grid buckets.
- `clusterDensePoints`: Runs deterministic weighted k-means over dense points.
- `binNumericSeries`: Bins numeric points into display-sized summaries with metric totals.

Debug operations:

- `describe`: inspect package metadata and runtime support.

Runtime support: library, CLI, server, and WASM wrappers expose these operations.

Run the primary workflow through the CLI:

```bash
cargo run -p moritzbrantner-dense-data-cli -- run \
  --operation summarizeDensePoints \
  --json '{"points":[{"coordinates":[0,1],"weight":1},{"coordinates":[2,3],"value":4,"weight":2}]}'
```

Successful responses use the shared package-surface shape with `operation`,
`title`, `message`, `summary`, and `result`. Default surface calls are
deterministic, local-first, and do not download models, write persistent files,
or execute external tools unless an operation explicitly documents native or
external-tool execution.

## Related crates

- `math-linear`
- `math-statistics`
- `numbers-core`
- `vector-analysis-core`
- `video-analysis-data`
