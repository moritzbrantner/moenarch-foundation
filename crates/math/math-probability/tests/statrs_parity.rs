//! Independently exercise the Foundation public CDF contract against bounded
//! legacy reference recurrences and established analytical values.

use math_probability::{Binomial, Normal, Poisson};
use statrs::distribution::{ContinuousCDF, DiscreteCDF};

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        actual.is_finite() && expected.is_finite() && (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual} (absolute tolerance {tolerance})"
    );
}

fn legacy_poisson_cdf(lambda: f64, k: u64) -> f64 {
    // Bounded reference only. This was the previous algorithm and must never be
    // invoked for large k; the production implementation no longer uses it.
    assert!(k <= 256);
    let mut term = -lambda;
    let mut total = term;
    for count in 1..=k {
        term += lambda.ln() - (count as f64).ln();
        let max = total.max(term);
        total = max + ((total - max).exp() + (term - max).exp()).ln();
    }
    total.exp().clamp(0.0, 1.0)
}

#[test]
fn normal_cdf_matches_analytic_and_previous_precision_contracts() {
    let normal = Normal::new(0.0, 1.0).unwrap();
    for (x, expected) in [
        (-3.0, 0.001_349_898_031_630_093_3),
        (-1.0, 0.158_655_253_931_457_07),
        (0.0, 0.5),
        (1.0, 0.841_344_746_068_542_9),
        (3.0, 0.998_650_101_968_369_9),
    ] {
        assert_close(normal.cdf(x).unwrap(), expected, 5.0e-11);
    }
    // Foundation validation is stricter than statrs: infinite variance is rejected.
    assert!(Normal::new(0.0, f64::MAX).is_err());
    assert!(normal.cdf(f64::NAN).is_err());
    assert_eq!(normal.cdf(-100.0).unwrap(), 0.0);
    assert_eq!(normal.cdf(100.0).unwrap(), 1.0);
}

#[test]
fn binomial_cdf_matches_independent_finite_mass_sums() {
    for (trials, probability) in [
        (0, 0.7),
        (5, 0.0),
        (5, 1.0),
        (10, 0.5),
        (40, 0.15),
        (100, 0.95),
    ] {
        let binomial = Binomial::new(trials, probability).unwrap();
        for k in [0, 1, trials / 4, trials / 2, trials, trials + 1] {
            let expected: f64 = (0..=k.min(trials)).map(|j| binomial.pmf(j)).sum();
            assert_close(binomial.cdf(k), expected, 1.0e-10);
        }
    }
    assert_eq!(Binomial::new(u64::MAX, 0.5).unwrap().cdf(u64::MAX), 1.0);
}

#[test]
fn poisson_cdf_matches_bounded_legacy_recurrence() {
    for lambda in [0.1, 1.0, 3.0, 30.0, 100.0] {
        let poisson = Poisson::new(lambda).unwrap();
        for k in [0, 1, 2, 10, 40, 100, 200] {
            assert_close(poisson.cdf(k), legacy_poisson_cdf(lambda, k), 1.0e-11);
        }
    }
    // statrs rejects a zero rate; Foundation explicitly supports it.
    let zero = Poisson::new(0.0).unwrap();
    assert_eq!(zero.pmf(0), 1.0);
    assert_eq!(zero.log_pmf(1), f64::NEG_INFINITY);
    assert_eq!(zero.cdf(u64::MAX), 1.0);
    // These counts used to trigger effectively unbounded iterations.
    assert_eq!(Poisson::new(3.0).unwrap().cdf(1_000_000_000), 1.0);
    assert_eq!(Poisson::new(3.0).unwrap().cdf(u64::MAX), 1.0);
}

#[test]
fn probability_boundary_does_not_expose_upstream_backend_types() {
    let gaussian = Normal::new(4.0, 2.0).unwrap();
    let statrs_gaussian = statrs::distribution::Normal::new(4.0, 2.0).unwrap();
    for x in [-10.0, 1.0, 4.0, 5.0, 20.0] {
        assert_close(gaussian.cdf(x).unwrap(), statrs_gaussian.cdf(x), 1.0e-14);
    }
    let poisson = Poisson::new(4.0).unwrap();
    let statrs_poisson = statrs::distribution::Poisson::new(4.0).unwrap();
    for k in [0, 1, 5, 10, 100] {
        assert_close(poisson.cdf(k), statrs_poisson.cdf(k), 1.0e-14);
    }
}
