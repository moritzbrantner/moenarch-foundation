//! Differential and reference evidence for the Foundation CDF contract.
//!
//! * `data/cdf_reference.csv` holds high-precision values generated offline by
//!   `data/generate_cdf_reference.py` (mpmath, 45 digits) at the exact binary64
//!   arguments the production path evaluates.
//! * The production path delegates to cdflib; statrs (dev-only) is the
//!   differential candidate. Where statrs disagrees with the reference the
//!   mismatch is recorded below instead of loosening the Foundation tolerance.
//! * The original first-party recurrences remain as bounded independent
//!   references for small arguments.
//!
//! Tolerance policy (documented in the crate README): for a case with scale
//! `s` (1 for the normal distribution, `lambda` for Poisson, `n` for
//! Binomial) the absolute error must not exceed `8 ε √max(1, s)` and, when
//! the reference is at least `1e-300`, the relative error must not exceed
//! `64 ε √max(1, s)`. The `√s` factor is the conditioning of forming the
//! incomplete-function ratio `x / a` in binary64 for large arguments.

use math_probability::{Bernoulli, Binomial, Normal, Poisson, Uniform};
use statrs::distribution::{ContinuousCDF, DiscreteCDF};

const REFERENCE: &str = include_str!("data/cdf_reference.csv");

#[derive(Debug, Clone, Copy)]
enum Case {
    Normal {
        z: f64,
    },
    Poisson {
        lambda: f64,
        k: u64,
    },
    Binomial {
        trials: u64,
        probability: f64,
        k: u64,
    },
}

impl Case {
    fn scale(self) -> f64 {
        match self {
            Self::Normal { .. } => 1.0,
            Self::Poisson { lambda, .. } => lambda.max(1.0),
            Self::Binomial { trials, .. } => (trials as f64).max(1.0),
        }
    }

    fn foundation(self) -> f64 {
        match self {
            Self::Normal { z } => Normal::new(0.0, 1.0).unwrap().cdf(z).unwrap(),
            Self::Poisson { lambda, k } => Poisson::new(lambda).unwrap().cdf(k),
            Self::Binomial {
                trials,
                probability,
                k,
            } => Binomial::new(trials, probability).unwrap().cdf(k),
        }
    }

    fn statrs(self) -> f64 {
        let value = match self {
            Self::Normal { z } => statrs::distribution::Normal::new(0.0, 1.0).unwrap().cdf(z),
            Self::Poisson { lambda, k } => {
                statrs::distribution::Poisson::new(lambda).unwrap().cdf(k)
            }
            Self::Binomial {
                trials,
                probability,
                k,
            } => statrs::distribution::Binomial::new(probability, trials)
                .unwrap()
                .cdf(k),
        };
        // Foundation clamps final rounding drift; compare on the same footing.
        value.clamp(0.0, 1.0)
    }
}

fn reference_cases() -> Vec<(Case, f64)> {
    REFERENCE
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split(',').collect();
            assert_eq!(fields.len(), 5, "malformed reference row {line}");
            let reference: f64 = fields[4].parse().unwrap();
            let case = match fields[0] {
                "normal" => Case::Normal {
                    z: fields[1].parse().unwrap(),
                },
                "poisson" => Case::Poisson {
                    lambda: fields[1].parse().unwrap(),
                    k: fields[3].parse().unwrap(),
                },
                "binomial" => Case::Binomial {
                    trials: fields[1].parse().unwrap(),
                    probability: fields[2].parse().unwrap(),
                    k: fields[3].parse().unwrap(),
                },
                other => panic!("unknown reference kind {other}"),
            };
            (case, reference)
        })
        .collect()
}

fn within_policy(actual: f64, reference: f64, scale: f64) -> bool {
    let conditioning = f64::EPSILON * scale.max(1.0).sqrt();
    let absolute = (actual - reference).abs();
    actual.is_finite()
        && absolute <= 8.0 * conditioning
        && (reference < 1.0e-300 || absolute <= 64.0 * conditioning * reference)
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        actual.is_finite() && expected.is_finite() && (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual} (absolute tolerance {tolerance})"
    );
}

#[test]
fn foundation_cdfs_match_high_precision_reference() {
    let cases = reference_cases();
    assert!(cases.len() > 300, "reference table unexpectedly small");
    let mut failures = Vec::new();
    for (case, reference) in cases {
        let actual = case.foundation();
        if !within_policy(actual, reference, case.scale()) {
            failures.push(format!("{case:?}: reference {reference:e}, got {actual:e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn statrs_agrees_where_its_algorithms_converge() {
    // statrs' normal CDF carries ~1.4e-11 absolute error (vs ~1e-16 for the
    // production backend); its discrete CDFs agree tightly for small scales.
    for (case, reference) in reference_cases() {
        let tolerance = match case {
            Case::Normal { .. } => 5.0e-11,
            Case::Poisson { lambda, .. } if lambda <= 1.0e3 => 1.0e-13,
            Case::Poisson { lambda, .. } if lambda <= 1.0e6 => 1.0e-9,
            Case::Binomial { trials, .. } if trials <= 100_000 => 1.0e-10,
            _ => continue,
        };
        assert_close(case.statrs(), reference, tolerance);
    }
}

/// Recorded statrs 0.19.1 mismatches: each case is Foundation-valid, the
/// production path meets the tolerance policy, and statrs is wrong by at
/// least `minimum_error`. statrs is not evaluated for Poisson rates of 1e14
/// and above: its incomplete-gamma series is O(√lambda) (0.5 s per call at
/// 1e15) and did not terminate within 20 s at 1e16.
#[test]
fn statrs_defects_on_large_arguments_are_recorded() {
    let recorded = [
        // (case, reference, minimum statrs absolute error)
        (
            Case::Poisson {
                lambda: 1.0e9,
                k: 1_000_000_000,
            },
            0.500_008_410_441_739,
            1.0e-6,
        ),
        (
            Case::Poisson {
                lambda: 1.0e12,
                k: 1_000_000_000_000,
            },
            0.500_000_265_961_520_3,
            1.0e-4,
        ),
        (
            Case::Binomial {
                trials: 1_000_000,
                probability: 0.5,
                k: 500_000,
            },
            0.500_398_942_180_665_9,
            1.0e-3,
        ),
        (
            // statrs returns -0.54 (clamped to 0) at the median.
            Case::Binomial {
                trials: 100_000_000,
                probability: 0.5,
                k: 50_000_000,
            },
            0.500_039_894_227_940_4,
            0.4,
        ),
        (
            // statrs returns 1 instead of the Poisson(10)-like 0.583.
            Case::Binomial {
                trials: 1_000_000_000_000_000_000,
                probability: 1.0e-17,
                k: 10,
            },
            0.583_039_750_192_985,
            0.4,
        ),
    ];
    for (case, reference, minimum_error) in recorded {
        assert!(
            within_policy(case.foundation(), reference, case.scale()),
            "{case:?}: Foundation {} vs reference {reference}",
            case.foundation()
        );
        let statrs_error = (case.statrs() - reference).abs();
        assert!(
            statrs_error >= minimum_error,
            "{case:?}: statrs error {statrs_error:e} is smaller than recorded; \
             re-evaluate the backend decision"
        );
    }
}

fn legacy_poisson_cdf(lambda: f64, k: u64) -> f64 {
    // Bounded reference only: the original O(k) recurrence.
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
fn poisson_cdf_matches_bounded_legacy_recurrence() {
    for lambda in [0.1, 1.0, 3.0, 30.0, 100.0] {
        let poisson = Poisson::new(lambda).unwrap();
        for k in [0, 1, 2, 10, 40, 100, 200] {
            assert_close(poisson.cdf(k), legacy_poisson_cdf(lambda, k), 1.0e-11);
        }
    }
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
}

#[test]
fn degenerate_and_extreme_valid_inputs_keep_foundation_semantics() {
    // Zero trials and k >= n are exactly 1; p = 0 / p = 1 are point masses.
    assert_eq!(Binomial::new(0, 0.7).unwrap().cdf(0), 1.0);
    assert_eq!(Binomial::new(u64::MAX, 0.5).unwrap().cdf(u64::MAX), 1.0);
    assert_eq!(Binomial::new(5, 0.0).unwrap().cdf(0), 1.0);
    assert_eq!(Binomial::new(5, 1.0).unwrap().cdf(4), 0.0);
    assert_eq!(Binomial::new(5, 1.0).unwrap().cdf(5), 1.0);
    // Tiny and near-one success probabilities (statrs rounds the first to 1).
    assert_eq!(
        Binomial::new(10, 1.0e-17).unwrap().cdf(0),
        0.999_999_999_999_999_9
    );
    for probability in [f64::from_bits(1), 1.0e-310, 1.0e-300] {
        assert_eq!(Binomial::new(10, probability).unwrap().cdf(0), 1.0);
    }
    let near_one = Binomial::new(10, 1.0 - f64::EPSILON).unwrap();
    let statrs_near_one = statrs::distribution::Binomial::new(1.0 - f64::EPSILON, 10).unwrap();
    for k in [0, 5, 9] {
        let expected = statrs_near_one.cdf(k);
        assert!((near_one.cdf(k) - expected).abs() <= 1.0e-14 * expected);
    }

    // statrs rejects a zero rate; Foundation explicitly supports it.
    let zero = Poisson::new(0.0).unwrap();
    assert_eq!(zero.pmf(0), 1.0);
    assert_eq!(zero.log_pmf(1), f64::NEG_INFINITY);
    assert_eq!(zero.cdf(0), 1.0);
    assert_eq!(zero.cdf(u64::MAX), 1.0);
    assert!(statrs::distribution::Poisson::new(0.0).is_err());
    // Subnormal and maximal finite rates.
    assert_eq!(Poisson::new(f64::from_bits(1)).unwrap().cdf(0), 1.0);
    assert_eq!(Poisson::new(f64::MAX).unwrap().cdf(0), 0.0);
    assert_eq!(Poisson::new(f64::MAX).unwrap().cdf(u64::MAX), 0.0);
    // Counts that used to drive an O(k) loop, and huge rates near the mode
    // where statrs' series does not terminate in practice.
    assert_eq!(Poisson::new(3.0).unwrap().cdf(1_000_000_000), 1.0);
    assert_eq!(Poisson::new(3.0).unwrap().cdf(u64::MAX), 1.0);
    let huge = Poisson::new(1.8e19)
        .unwrap()
        .cdf(18_000_000_000_000_000_000);
    assert!((huge - 0.5).abs() < 1.0e-6, "{huge}");

    // Normal: validation is stricter than statrs, tails saturate exactly, and
    // an overflowing standardization still yields the exact limit.
    assert!(Normal::new(0.0, f64::MAX).is_err());
    let standard = Normal::new(0.0, 1.0).unwrap();
    assert!(standard.cdf(f64::NAN).is_err());
    assert_eq!(standard.cdf(-100.0).unwrap(), 0.0);
    assert_eq!(standard.cdf(100.0).unwrap(), 1.0);
    let shifted = Normal::new(f64::MAX, 1.0).unwrap();
    assert_eq!(shifted.cdf(-f64::MAX).unwrap(), 0.0);
    assert_eq!(shifted.cdf(f64::MAX).unwrap(), 0.5);
    let narrow = Normal::new(0.0, 1.0e-300).unwrap();
    assert_close(
        narrow.cdf(-1.0e-300).unwrap(),
        0.158_655_253_931_457_05,
        1.0e-16,
    );
}

#[test]
fn bernoulli_and_uniform_match_statrs() {
    for probability in [0.0, 0.25, 0.5, 1.0] {
        let foundation = Bernoulli::new(probability).unwrap();
        let statrs = statrs::distribution::Bernoulli::new(probability).unwrap();
        for k in [0, 1, 2] {
            assert_eq!(foundation.cdf(k), statrs.cdf(k));
            assert_eq!(
                foundation.pmf(k),
                statrs::distribution::Discrete::pmf(&statrs, k)
            );
        }
    }
    let foundation = Uniform::new(-2.0, 3.0).unwrap();
    let statrs = statrs::distribution::Uniform::new(-2.0, 3.0).unwrap();
    for x in [-5.0, -2.0, -1.5, 0.0, 2.999, 3.0, 7.0] {
        assert_close(foundation.cdf(x).unwrap(), statrs.cdf(x), 1.0e-15);
    }
}
