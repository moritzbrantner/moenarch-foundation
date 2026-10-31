use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use dense_data::{
    bucket_points, dense_summary, k_means, BucketGrid, DensePoint, KMeansConfig,
    NumericHeatmapQuery, NumericHistogramQuery, NumericSeriesIndex, NumericSeriesPoint,
    NumericSeriesQuery, NumericValueAccessor, NumericValueMode,
};
use std::collections::BTreeMap;
use std::hint::black_box;

fn sample_points() -> Vec<DensePoint> {
    (0..2_048)
        .map(|index| {
            let x = (index % 64) as f64 * 0.25;
            let y = (index / 64) as f64 * 0.125;
            DensePoint::new([x, y])
                .unwrap()
                .weighted(1.0 + (index % 5) as f64)
                .unwrap()
                .valued((index % 17) as f64)
                .unwrap()
        })
        .collect()
}

fn sample_series() -> Vec<NumericSeriesPoint> {
    const METRICS: [&str; 6] = [
        "bitrate", "duration", "frames", "keyframes", "motion", "scenes",
    ];
    (0..20_000)
        .map(|index| NumericSeriesPoint {
            source_index: index,
            x: (index * 7_919 % 20_000) as f64 * 0.05,
            y: (index % 257) as f64 - 128.0,
            metrics: METRICS
                .iter()
                .enumerate()
                .map(|(metric, key)| (key.to_string(), (index % (metric + 2)) as f64))
                .collect::<BTreeMap<_, _>>(),
        })
        .collect()
}

fn benchmark_numeric_series(c: &mut Criterion) {
    let points = sample_series();
    let index = NumericSeriesIndex::from_points(points.clone()).unwrap();

    c.bench_function("numeric_series_index_20k", |b| {
        b.iter_batched(
            || points.clone(),
            |points| NumericSeriesIndex::from_points(points).unwrap(),
            BatchSize::LargeInput,
        )
    });

    c.bench_function("numeric_chart_series_20k", |b| {
        b.iter(|| {
            index
                .get_chart_series(black_box(NumericSeriesQuery {
                    x_domain: [0.0, 1_000.0],
                    target_bin_count: 500,
                    value_mode: NumericValueMode::Average,
                    include_empty_bins: false,
                }))
                .unwrap()
        })
    });

    c.bench_function("numeric_histogram_20k", |b| {
        b.iter(|| {
            index
                .get_histogram(black_box(NumericHistogramQuery {
                    bucket_count: 128,
                    include_empty_buckets: true,
                    x_domain: None,
                    value_domain: None,
                    value_accessor: NumericValueAccessor::Y,
                }))
                .unwrap()
        })
    });

    c.bench_function("numeric_heatmap_sparse_20k", |b| {
        b.iter(|| {
            index
                .get_heatmap(black_box(NumericHeatmapQuery {
                    x_bin_count: 256,
                    x_domain: [0.0, 1_000.0],
                    y_bin_count: 128,
                    y_domain: None,
                    include_empty_cells: false,
                    value_accessor: NumericValueAccessor::Y,
                }))
                .unwrap()
        })
    });
}

fn benchmark_pipeline(c: &mut Criterion) {
    let points = sample_points();
    let grid = BucketGrid::uniform(2, 1.0).unwrap();
    let config = KMeansConfig {
        clusters: 8,
        max_iterations: 50,
        tolerance: 0.0001,
    };

    c.bench_function("dense_summary_2k", |b| {
        b.iter(|| dense_summary(black_box(&points)).unwrap())
    });

    c.bench_function("bucket_points_2k", |b| {
        b.iter(|| bucket_points(black_box(&points), black_box(&grid)).unwrap())
    });

    c.bench_function("k_means_2k", |b| {
        b.iter(|| k_means(black_box(&points), black_box(config)).unwrap())
    });
}

criterion_group!(benches, benchmark_pipeline, benchmark_numeric_series);
criterion_main!(benches);
