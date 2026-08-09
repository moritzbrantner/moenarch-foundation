use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vector_analysis_core::{
    cosine_similarity, dot, euclidean_distance, mean_vector, vector_stats, DenseVector,
};

fn vector(dimensions: usize, phase: f32) -> Vec<f32> {
    (0..dimensions)
        .map(|index| {
            let value = index as f32 * 0.017 + phase;
            value.sin() * 0.5 + value.cos() * 0.25
        })
        .collect()
}

fn vectors(count: usize, dimensions: usize) -> Vec<DenseVector> {
    (0..count)
        .map(|index| DenseVector::new(vector(dimensions, index as f32 * 0.01)).unwrap())
        .collect()
}

fn bench_metrics(c: &mut Criterion) {
    let left = vector(768, 0.0);
    let right = vector(768, 0.7);
    let batch = vectors(2_048, 128);

    c.bench_function("dot_768", |b| {
        b.iter(|| dot(black_box(&left), black_box(&right)).unwrap())
    });

    c.bench_function("cosine_similarity_768", |b| {
        b.iter(|| cosine_similarity(black_box(&left), black_box(&right)).unwrap())
    });

    c.bench_function("euclidean_distance_768", |b| {
        b.iter(|| euclidean_distance(black_box(&left), black_box(&right)).unwrap())
    });

    c.bench_function("mean_vector_2k_x_128", |b| {
        b.iter(|| mean_vector(black_box(&batch)).unwrap())
    });

    c.bench_function("vector_stats_2k_x_128", |b| {
        b.iter(|| vector_stats(black_box(&batch)).unwrap())
    });
}

criterion_group!(benches, bench_metrics);
criterion_main!(benches);
