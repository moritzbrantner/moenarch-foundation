use std::collections::BTreeMap;

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
    let dataset =
        DenseDataset::from_points([point([1.0, 2.0]), point([3.0, 4.0]).weighted(2.0).unwrap()])
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
