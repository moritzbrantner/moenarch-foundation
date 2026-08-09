use criterion::{black_box, criterion_group, criterion_main, Criterion};
use numbers_core::{histogram, quartiles, summarize_numbers, HistogramConfig, RunningStats};

fn benchmark_stats(c: &mut Criterion) {
    let values = (0..10_000)
        .map(|index| ((index % 97) as f64 * 0.25) - 8.0)
        .collect::<Vec<_>>();

    c.bench_function("summarize_numbers_10k", |b| {
        b.iter(|| summarize_numbers(black_box(&values)))
    });

    c.bench_function("quartiles_10k", |b| {
        b.iter(|| quartiles(black_box(&values)).unwrap())
    });

    let config = HistogramConfig::new(64).unwrap();
    c.bench_function("histogram_10k", |b| {
        b.iter(|| histogram(black_box(&values), black_box(config)).unwrap())
    });

    c.bench_function("running_stats_push_weighted_10k", |b| {
        b.iter(|| {
            let mut stats = RunningStats::new();
            for value in black_box(&values) {
                stats.push_weighted(*value, 1.5).unwrap();
            }
            stats.summary()
        })
    });
}

criterion_group!(benches, benchmark_stats);
criterion_main!(benches);
