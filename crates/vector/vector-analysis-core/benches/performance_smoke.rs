use iai_callgrind::{
    library_benchmark, library_benchmark_group, main, Callgrind, EventKind,
    LibraryBenchmarkConfig,
};
use std::hint::black_box;
use vector_analysis_core::{cosine_similarity, dot, mean_vector, DenseVector};

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
        .map(|index| {
            DenseVector::new(vector(dimensions, index as f32 * 0.01))
                .expect("deterministic benchmark vectors are finite and non-empty")
        })
        .collect()
}

#[library_benchmark]
#[bench::dot_768(args = (vector(768, 0.0), vector(768, 0.7)))]
fn bench_dot(left: Vec<f32>, right: Vec<f32>) -> f32 {
    black_box(dot(&left, &right).expect("benchmark vectors have matching dimensions"))
}

#[library_benchmark]
#[bench::cosine_768(args = (vector(768, 0.0), vector(768, 0.7)))]
fn bench_cosine_similarity(left: Vec<f32>, right: Vec<f32>) -> f32 {
    black_box(
        cosine_similarity(&left, &right)
            .expect("benchmark vectors are finite, non-zero, and have matching dimensions"),
    )
}

#[library_benchmark]
#[bench::mean_256_x_128(vectors(256, 128))]
fn bench_mean_vector(batch: Vec<DenseVector>) -> DenseVector {
    black_box(mean_vector(&batch).expect("benchmark vectors share one finite shape"))
}

library_benchmark_group!(
    name = vector_metrics_smoke;
    benchmarks = bench_dot, bench_cosine_similarity, bench_mean_vector
);

fn benchmark_config() -> LibraryBenchmarkConfig {
    let mut callgrind = Callgrind::default();
    callgrind
        .soft_limits([(EventKind::Ir, 5.0)])
        .fail_fast(true);
    LibraryBenchmarkConfig::default().tool(callgrind)
}

main!(
    config = benchmark_config();
    library_benchmark_groups = vector_metrics_smoke
);
