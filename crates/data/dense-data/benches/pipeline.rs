use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dense_data::{bucket_points, dense_summary, k_means, BucketGrid, DensePoint, KMeansConfig};

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

criterion_group!(benches, benchmark_pipeline);
criterion_main!(benches);
