//! Deterministic comparative workload: candidate regularized gamma CDF against
//! the original log-space Poisson sum. Inputs are kept outside the measured body.
//! Run: cargo bench --locked -p moenarch-math-probability --bench cdf_reference
//! Compare controlled-run timings; shared CI wall-clock numbers are not a gate.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use math_probability::{Binomial, Normal, Poisson};
use std::hint::black_box;

fn legacy_poisson_cdf(lambda: f64, k: u64) -> f64 {
    let mut log_term = -lambda;
    let mut log_sum = log_term;
    for count in 1..=k {
        log_term += lambda.ln() - (count as f64).ln();
        let maximum = log_sum.max(log_term);
        log_sum = maximum + ((log_sum - maximum).exp() + (log_term - maximum).exp()).ln();
    }
    log_sum.exp().clamp(0.0, 1.0)
}

fn cdf_reference(c: &mut Criterion) {
    let mut poisson = c.benchmark_group("poisson_cdf");
    for (lambda, k) in [(3.0, 2), (30.0, 30), (100.0, 100), (100.0, 10_000)] {
        let foundation = Poisson::new(lambda).unwrap();
        let workload = format!("lambda_{lambda}_k_{k}");
        poisson.bench_with_input(BenchmarkId::new("statrs", &workload), &k, |b, &k| {
            b.iter(|| black_box(foundation.cdf(black_box(k))))
        });
        poisson.bench_with_input(BenchmarkId::new("legacy", &workload), &k, |b, &k| {
            b.iter(|| black_box(legacy_poisson_cdf(black_box(lambda), black_box(k))))
        });
    }
    let very_large = Poisson::new(3.0).unwrap();
    poisson.bench_function("statrs/lambda_3_k_u64max", |b| {
        b.iter(|| black_box(very_large.cdf(black_box(u64::MAX))))
    });
    poisson.finish();

    let binomial = Binomial::new(100_000, 0.25).unwrap();
    c.bench_function("binomial_cdf/n_100000_p_025_k_25000", |b| {
        b.iter(|| black_box(binomial.cdf(black_box(25_000))))
    });
    let normal = Normal::new(0.0, 1.0).unwrap();
    c.bench_function("normal_cdf/mu_0_sd_1_x_3", |b| {
        b.iter(|| black_box(normal.cdf(black_box(3.0)).unwrap()))
    });
}

criterion_group!(benches, cdf_reference);
criterion_main!(benches);
