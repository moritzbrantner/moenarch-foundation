//! Deterministic comparative CDF workloads.
//!
//! * `foundation` is the production path (`math_probability`, delegating to
//!   cdflib's incomplete gamma/beta ratios and `cumnor`).
//! * `statrs` is the previously selected external candidate (dev-only).
//! * `legacy` is the original first-party log-space recurrence, which is
//!   O(k) and is therefore only measured where it completes quickly.
//!
//! Inputs are constructed outside the measured body and pass through
//! `black_box`. Workloads on which statrs takes more than ~1 ms per call (or
//! does not terminate) are measured for the production path only; see the
//! crate README for the recorded numbers and environment fingerprint.
//!
//! Run: cargo bench --locked -p moenarch-math-probability --bench cdf_reference
//! Compare controlled-run timings; shared CI wall-clock numbers are not a gate.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use math_probability::{Binomial, Normal, Poisson};
use statrs::distribution::{ContinuousCDF, DiscreteCDF};
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

fn legacy_binomial_lower_cdf(trials: u64, probability: f64, k: u64) -> f64 {
    let mut log_term = trials as f64 * (-probability).ln_1p();
    let mut log_sum = log_term;
    for successes in 0..k {
        log_term += ((trials - successes) as f64).ln() - ((successes + 1) as f64).ln()
            + probability.ln()
            - (-probability).ln_1p();
        let maximum = log_sum.max(log_term);
        log_sum = maximum + ((log_sum - maximum).exp() + (log_term - maximum).exp()).ln();
    }
    log_sum.exp().clamp(0.0, 1.0)
}

/// `(workload, lambda, k, measure statrs, measure legacy)`.
const POISSON: &[(&str, f64, u64, bool, bool)] = &[
    ("lambda_3_k_2", 3.0, 2, true, true),
    ("lambda_30_k_30", 30.0, 30, true, true),
    ("lambda_100_k_100", 100.0, 100, true, true),
    ("lambda_100_k_10000", 100.0, 10_000, true, true),
    ("lambda_1e4_k_1e4", 1.0e4, 10_000, true, true),
    ("lambda_1e6_k_1e6", 1.0e6, 1_000_000, true, false),
    ("lambda_1e9_k_1e9", 1.0e9, 1_000_000_000, true, false),
    (
        "lambda_1e15_k_1e15",
        1.0e15,
        1_000_000_000_000_000,
        false,
        false,
    ),
    (
        "lambda_1e18_k_1e18",
        1.0e18,
        1_000_000_000_000_000_000,
        false,
        false,
    ),
    ("lambda_3_k_u64max", 3.0, u64::MAX, true, false),
];

/// `(workload, trials, probability, k, measure legacy)`.
const BINOMIAL: &[(&str, u64, f64, u64, bool)] = &[
    ("n_10_p_0.5_k_3", 10, 0.5, 3, true),
    ("n_1000_p_0.001_k_2", 1_000, 0.001, 2, true),
    ("n_1e5_p_0.25_k_25000", 100_000, 0.25, 25_000, true),
    ("n_1e8_p_0.5_k_5e7", 100_000_000, 0.5, 50_000_000, false),
    (
        "n_1e12_p_0.25_k_2.5e11",
        1_000_000_000_000,
        0.25,
        250_000_000_000,
        false,
    ),
    (
        "n_1e18_p_1e-17_k_10",
        1_000_000_000_000_000_000,
        1.0e-17,
        10,
        false,
    ),
];

const NORMAL_Z: &[(&str, f64)] = &[("z_-1", -1.0), ("z_3", 3.0), ("z_-20", -20.0)];

fn cdf_reference(c: &mut Criterion) {
    let mut poisson = c.benchmark_group("poisson_cdf");
    for &(workload, lambda, k, with_statrs, with_legacy) in POISSON {
        let foundation = Poisson::new(lambda).unwrap();
        poisson.bench_with_input(BenchmarkId::new("foundation", workload), &k, |b, &k| {
            b.iter(|| black_box(foundation.cdf(black_box(k))))
        });
        if with_statrs {
            let statrs = statrs::distribution::Poisson::new(lambda).unwrap();
            poisson.bench_with_input(BenchmarkId::new("statrs", workload), &k, |b, &k| {
                b.iter(|| black_box(statrs.cdf(black_box(k))))
            });
        }
        if with_legacy {
            poisson.bench_with_input(BenchmarkId::new("legacy", workload), &k, |b, &k| {
                b.iter(|| black_box(legacy_poisson_cdf(black_box(lambda), black_box(k))))
            });
        }
    }
    poisson.finish();

    let mut binomial = c.benchmark_group("binomial_cdf");
    for &(workload, trials, probability, k, with_legacy) in BINOMIAL {
        let foundation = Binomial::new(trials, probability).unwrap();
        binomial.bench_with_input(BenchmarkId::new("foundation", workload), &k, |b, &k| {
            b.iter(|| black_box(foundation.cdf(black_box(k))))
        });
        let statrs = statrs::distribution::Binomial::new(probability, trials).unwrap();
        binomial.bench_with_input(BenchmarkId::new("statrs", workload), &k, |b, &k| {
            b.iter(|| black_box(statrs.cdf(black_box(k))))
        });
        if with_legacy {
            binomial.bench_with_input(BenchmarkId::new("legacy", workload), &k, |b, &k| {
                b.iter(|| {
                    black_box(legacy_binomial_lower_cdf(
                        black_box(trials),
                        black_box(probability),
                        black_box(k),
                    ))
                })
            });
        }
    }
    binomial.finish();

    let mut normal = c.benchmark_group("normal_cdf");
    let foundation = Normal::new(0.0, 1.0).unwrap();
    let statrs = statrs::distribution::Normal::new(0.0, 1.0).unwrap();
    for &(workload, z) in NORMAL_Z {
        normal.bench_with_input(BenchmarkId::new("foundation", workload), &z, |b, &z| {
            b.iter(|| black_box(foundation.cdf(black_box(z)).unwrap()))
        });
        normal.bench_with_input(BenchmarkId::new("statrs", workload), &z, |b, &z| {
            b.iter(|| black_box(statrs.cdf(black_box(z))))
        });
    }
    normal.finish();
}

criterion_group!(benches, cdf_reference);
criterion_main!(benches);
