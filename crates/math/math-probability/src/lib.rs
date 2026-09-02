#![doc = include_str!("../README.md")]

use media_core::{DetectError, Result};
use std::f64::consts::{PI, SQRT_2};

fn invalid_argument(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}

fn validate_probability(value: f64, name: &str) -> Result<f64> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(invalid_argument(format!(
            "{name} must be finite and between 0 and 1 inclusive"
        )));
    }
    Ok(value)
}

fn validate_finite(value: f64, name: &str) -> Result<f64> {
    if !value.is_finite() {
        return Err(invalid_argument(format!("{name} must be finite")));
    }
    Ok(value)
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Bernoulli distribution over outcomes `0` and `1`.
pub struct Bernoulli {
    probability: f64,
}

impl Bernoulli {
    /// Creates a Bernoulli distribution with success probability in `[0, 1]`.
    pub fn new(probability: f64) -> Result<Self> {
        Ok(Self {
            probability: validate_probability(probability, "Bernoulli probability")?,
        })
    }

    /// Returns the success probability.
    #[must_use]
    pub const fn probability(self) -> f64 {
        self.probability
    }

    /// Returns probability mass for integer outcome `k`.
    #[must_use]
    pub fn pmf(self, k: u64) -> f64 {
        match k {
            0 => 1.0 - self.probability,
            1 => self.probability,
            _ => 0.0,
        }
    }

    /// Returns log probability mass. Zero mass maps to negative infinity.
    #[must_use]
    pub fn log_pmf(self, k: u64) -> f64 {
        self.pmf(k).ln()
    }

    /// Returns cumulative probability through integer outcome `k`.
    #[must_use]
    pub fn cdf(self, k: u64) -> f64 {
        if k == 0 {
            1.0 - self.probability
        } else {
            1.0
        }
    }

    /// Returns the distribution mean.
    #[must_use]
    pub const fn mean(self) -> f64 {
        self.probability
    }

    /// Returns the distribution variance.
    #[must_use]
    pub fn variance(self) -> f64 {
        self.probability * (1.0 - self.probability)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Continuous uniform distribution over the closed interval `[min, max]`.
pub struct Uniform {
    min: f64,
    max: f64,
    width: f64,
}

impl Uniform {
    /// Creates a finite non-degenerate uniform distribution.
    pub fn new(min: f64, max: f64) -> Result<Self> {
        validate_finite(min, "uniform minimum")?;
        validate_finite(max, "uniform maximum")?;
        if max <= min {
            return Err(invalid_argument(
                "uniform maximum must be greater than minimum",
            ));
        }
        let width = max - min;
        if !width.is_finite() {
            return Err(invalid_argument("uniform interval width must be finite"));
        }
        Ok(Self { min, max, width })
    }

    /// Returns the lower support bound.
    #[must_use]
    pub const fn min(self) -> f64 {
        self.min
    }

    /// Returns the upper support bound.
    #[must_use]
    pub const fn max(self) -> f64 {
        self.max
    }

    /// Returns probability density at finite `x`.
    pub fn pdf(self, x: f64) -> Result<f64> {
        validate_finite(x, "uniform evaluation point")?;
        Ok(if x < self.min || x > self.max {
            0.0
        } else {
            1.0 / self.width
        })
    }

    /// Returns log density at finite `x`. Zero density maps to negative infinity.
    pub fn log_pdf(self, x: f64) -> Result<f64> {
        validate_finite(x, "uniform evaluation point")?;
        Ok(if x < self.min || x > self.max {
            f64::NEG_INFINITY
        } else {
            -self.width.ln()
        })
    }

    /// Returns cumulative probability at finite `x`.
    pub fn cdf(self, x: f64) -> Result<f64> {
        validate_finite(x, "uniform evaluation point")?;
        Ok(if x <= self.min {
            0.0
        } else if x >= self.max {
            1.0
        } else {
            ((x - self.min) / self.width).clamp(0.0, 1.0)
        })
    }

    /// Returns the distribution mean.
    #[must_use]
    pub fn mean(self) -> f64 {
        self.min + self.width * 0.5
    }

    /// Returns the distribution variance.
    #[must_use]
    pub fn variance(self) -> f64 {
        self.width * self.width / 12.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Normal distribution with finite mean and positive standard deviation.
pub struct Normal {
    mean: f64,
    std_dev: f64,
}

impl Normal {
    /// Creates a normal distribution.
    pub fn new(mean: f64, std_dev: f64) -> Result<Self> {
        validate_finite(mean, "normal mean")?;
        validate_finite(std_dev, "normal standard deviation")?;
        if std_dev <= 0.0 || !(std_dev * std_dev).is_finite() {
            return Err(invalid_argument(
                "normal standard deviation must be positive with finite variance",
            ));
        }
        Ok(Self { mean, std_dev })
    }

    /// Returns probability density at finite `x`.
    pub fn pdf(self, x: f64) -> Result<f64> {
        Ok(self.log_pdf(x)?.exp())
    }

    /// Returns log probability density at finite `x`.
    pub fn log_pdf(self, x: f64) -> Result<f64> {
        validate_finite(x, "normal evaluation point")?;
        let z = (x - self.mean) / self.std_dev;
        Ok(-0.5 * z * z - self.std_dev.ln() - 0.5 * (2.0 * PI).ln())
    }

    /// Returns cumulative probability using a deterministic erf approximation.
    pub fn cdf(self, x: f64) -> Result<f64> {
        validate_finite(x, "normal evaluation point")?;
        let z = (x - self.mean) / (self.std_dev * SQRT_2);
        Ok((0.5 * (1.0 + erf_approx(z))).clamp(0.0, 1.0))
    }

    /// Returns the distribution mean.
    #[must_use]
    pub const fn mean(self) -> f64 {
        self.mean
    }

    /// Returns the distribution variance.
    #[must_use]
    pub fn variance(self) -> f64 {
        self.std_dev * self.std_dev
    }

    /// Returns the standard deviation.
    #[must_use]
    pub const fn std_dev(self) -> f64 {
        self.std_dev
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Binomial distribution for a fixed number of Bernoulli trials.
pub struct Binomial {
    trials: u64,
    probability: f64,
}

impl Binomial {
    /// Creates a binomial distribution. Zero trials are a valid degenerate case.
    pub fn new(trials: u64, probability: f64) -> Result<Self> {
        Ok(Self {
            trials,
            probability: validate_probability(probability, "binomial probability")?,
        })
    }

    /// Returns the number of trials.
    #[must_use]
    pub const fn trials(self) -> u64 {
        self.trials
    }

    /// Returns success probability.
    #[must_use]
    pub const fn probability(self) -> f64 {
        self.probability
    }

    /// Returns probability mass for `k` successes.
    #[must_use]
    pub fn pmf(self, k: u64) -> f64 {
        self.log_pmf(k).exp()
    }

    /// Returns log probability mass without factorial arithmetic.
    #[must_use]
    pub fn log_pmf(self, k: u64) -> f64 {
        if k > self.trials {
            return f64::NEG_INFINITY;
        }
        if self.probability == 0.0 {
            return if k == 0 { 0.0 } else { f64::NEG_INFINITY };
        }
        if self.probability == 1.0 {
            return if k == self.trials {
                0.0
            } else {
                f64::NEG_INFINITY
            };
        }
        log_binomial_coefficient(self.trials, k)
            + k as f64 * self.probability.ln()
            + (self.trials - k) as f64 * (-self.probability).ln_1p()
    }

    /// Returns cumulative probability through `k` successes.
    #[must_use]
    pub fn cdf(self, k: u64) -> f64 {
        if k >= self.trials {
            return 1.0;
        }
        if self.probability == 0.0 {
            return 1.0;
        }
        if self.probability == 1.0 {
            return 0.0;
        }

        if k <= self.trials / 2 {
            let mut log_term = self.trials as f64 * (-self.probability).ln_1p();
            let mut log_sum = log_term;
            for successes in 0..k {
                log_term += ((self.trials - successes) as f64).ln() - ((successes + 1) as f64).ln()
                    + self.probability.ln()
                    - (-self.probability).ln_1p();
                log_sum = log_add_exp(log_sum, log_term);
            }
            log_sum.exp().clamp(0.0, 1.0)
        } else {
            let mut log_term = self.trials as f64 * self.probability.ln();
            let mut log_tail = f64::NEG_INFINITY;
            for successes in (k + 1..=self.trials).rev() {
                log_tail = log_add_exp(log_tail, log_term);
                if successes > k + 1 {
                    log_term += (successes as f64).ln()
                        - ((self.trials - successes + 1) as f64).ln()
                        + (-self.probability).ln_1p()
                        - self.probability.ln();
                }
            }
            (1.0 - log_tail.exp()).clamp(0.0, 1.0)
        }
    }

    /// Returns the distribution mean.
    #[must_use]
    pub fn mean(self) -> f64 {
        self.trials as f64 * self.probability
    }

    /// Returns the distribution variance.
    #[must_use]
    pub fn variance(self) -> f64 {
        self.trials as f64 * self.probability * (1.0 - self.probability)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Poisson distribution with a finite non-negative event rate.
pub struct Poisson {
    lambda: f64,
}

impl Poisson {
    /// Creates a Poisson distribution. `lambda = 0` is a valid degenerate case.
    pub fn new(lambda: f64) -> Result<Self> {
        validate_finite(lambda, "Poisson lambda")?;
        if lambda < 0.0 {
            return Err(invalid_argument("Poisson lambda must be non-negative"));
        }
        Ok(Self { lambda })
    }

    /// Returns the event rate.
    #[must_use]
    pub const fn lambda(self) -> f64 {
        self.lambda
    }

    /// Returns probability mass for event count `k`.
    #[must_use]
    pub fn pmf(self, k: u64) -> f64 {
        self.log_pmf(k).exp()
    }

    /// Returns log probability mass without factorial arithmetic.
    #[must_use]
    pub fn log_pmf(self, k: u64) -> f64 {
        if self.lambda == 0.0 {
            return if k == 0 { 0.0 } else { f64::NEG_INFINITY };
        }
        -self.lambda + k as f64 * self.lambda.ln() - log_factorial(k)
    }

    /// Returns cumulative probability through event count `k` using log-space recurrence.
    #[must_use]
    pub fn cdf(self, k: u64) -> f64 {
        if self.lambda == 0.0 {
            return 1.0;
        }
        let mut log_term = -self.lambda;
        let mut log_sum = log_term;
        for count in 1..=k {
            log_term += self.lambda.ln() - (count as f64).ln();
            log_sum = log_add_exp(log_sum, log_term);
        }
        log_sum.exp().clamp(0.0, 1.0)
    }

    /// Returns the distribution mean.
    #[must_use]
    pub const fn mean(self) -> f64 {
        self.lambda
    }

    /// Returns the distribution variance.
    #[must_use]
    pub const fn variance(self) -> f64 {
        self.lambda
    }
}

fn erf_approx(value: f64) -> f64 {
    if value.is_infinite() {
        return value.signum();
    }
    let absolute = value.abs();
    let t = 1.0 / (1.0 + 0.5 * absolute);
    let polynomial = 1.000_023_68
        + t * (0.374_091_96
            + t * (0.096_784_18
                + t * (-0.186_288_06
                    + t * (0.278_868_07
                        + t * (-1.135_203_98
                            + t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77)))))));
    let tau = t * (-absolute * absolute - 1.265_512_23 + t * polynomial).exp();
    if value >= 0.0 {
        1.0 - tau
    } else {
        tau - 1.0
    }
}

fn log_add_exp(left: f64, right: f64) -> f64 {
    if left == f64::NEG_INFINITY {
        return right;
    }
    if right == f64::NEG_INFINITY {
        return left;
    }
    let maximum = left.max(right);
    maximum + ((left - maximum).exp() + (right - maximum).exp()).ln()
}

fn log_factorial(value: u64) -> f64 {
    if value < 2 {
        return 0.0;
    }
    if value <= 256 {
        return (2..=value).map(|term| (term as f64).ln()).sum();
    }
    let n = value as f64;
    let inverse = 1.0 / n;
    n * n.ln() - n + 0.5 * (2.0 * PI * n).ln() + inverse / 12.0 - inverse.powi(3) / 360.0
        + inverse.powi(5) / 1260.0
}

fn log_binomial_coefficient(trials: u64, successes: u64) -> f64 {
    let reduced = successes.min(trials - successes);
    if reduced <= 256 {
        return (1..=reduced)
            .map(|index| ((trials - reduced + index) as f64).ln() - (index as f64).ln())
            .sum();
    }
    log_factorial(trials) - log_factorial(successes) - log_factorial(trials - successes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual} with tolerance {tolerance}"
        );
    }

    #[test]
    fn bernoulli_support_and_moments_are_exact() {
        let distribution = Bernoulli::new(0.25).unwrap();
        assert_eq!(distribution.pmf(0), 0.75);
        assert_eq!(distribution.pmf(1), 0.25);
        assert_eq!(distribution.pmf(2), 0.0);
        assert_eq!(distribution.cdf(0), 0.75);
        assert_eq!(distribution.cdf(1), 1.0);
        assert_eq!(distribution.mean(), 0.25);
        assert_eq!(distribution.variance(), 0.1875);
    }

    #[test]
    fn uniform_support_density_cdf_and_moments_are_explicit() {
        let distribution = Uniform::new(-2.0, 2.0).unwrap();
        assert_eq!(distribution.pdf(-3.0).unwrap(), 0.0);
        assert_eq!(distribution.pdf(0.0).unwrap(), 0.25);
        assert_eq!(distribution.cdf(-2.0).unwrap(), 0.0);
        assert_eq!(distribution.cdf(0.0).unwrap(), 0.5);
        assert_eq!(distribution.cdf(2.0).unwrap(), 1.0);
        assert_eq!(distribution.mean(), 0.0);
        assert_close(distribution.variance(), 4.0 / 3.0, 1.0e-15);
    }

    #[test]
    fn standard_normal_matches_reference_fixtures() {
        let distribution = Normal::new(0.0, 1.0).unwrap();
        assert_close(
            distribution.pdf(0.0).unwrap(),
            0.398_942_280_401_432_7,
            1.0e-15,
        );
        assert_close(distribution.cdf(0.0).unwrap(), 0.5, 2.0e-7);
        assert_close(
            distribution.cdf(1.0).unwrap(),
            0.841_344_746_068_542_9,
            2.0e-7,
        );
        assert_close(
            distribution.cdf(-1.0).unwrap(),
            0.158_655_253_931_457_07,
            2.0e-7,
        );
        assert_close(
            distribution.cdf(3.0).unwrap(),
            0.998_650_101_968_369_9,
            2.0e-7,
        );
        assert_eq!(distribution.mean(), 0.0);
        assert_eq!(distribution.variance(), 1.0);
    }

    #[test]
    fn binomial_uses_stable_mass_and_cdf_fixtures() {
        let distribution = Binomial::new(10, 0.5).unwrap();
        assert_close(distribution.pmf(5), 0.246_093_75, 1.0e-15);
        assert_close(distribution.cdf(3), 0.171_875, 1.0e-14);
        assert_close(distribution.cdf(5), 0.623_046_875, 1.0e-14);
        assert_eq!(distribution.mean(), 5.0);
        assert_eq!(distribution.variance(), 2.5);
    }

    #[test]
    fn poisson_matches_reference_fixtures() {
        let distribution = Poisson::new(3.0).unwrap();
        assert_close(distribution.pmf(2), 0.224_041_807_655_387_75, 1.0e-14);
        assert_close(distribution.cdf(2), 0.423_190_081_126_843_53, 1.0e-14);
        assert_eq!(distribution.mean(), 3.0);
        assert_eq!(distribution.variance(), 3.0);
    }

    #[test]
    fn degenerate_discrete_parameters_are_well_defined() {
        let zero_trials = Binomial::new(0, 0.7).unwrap();
        assert_eq!(zero_trials.pmf(0), 1.0);
        assert_eq!(zero_trials.cdf(0), 1.0);

        let impossible = Binomial::new(5, 0.0).unwrap();
        assert_eq!(impossible.pmf(0), 1.0);
        assert_eq!(impossible.pmf(1), 0.0);
        assert_eq!(impossible.log_pmf(1), f64::NEG_INFINITY);

        let poisson = Poisson::new(0.0).unwrap();
        assert_eq!(poisson.pmf(0), 1.0);
        assert_eq!(poisson.pmf(1), 0.0);
        assert_eq!(poisson.cdf(100), 1.0);
    }

    #[test]
    fn invalid_parameters_and_non_finite_points_are_rejected() {
        assert!(Bernoulli::new(-0.1).is_err());
        assert!(Bernoulli::new(f64::NAN).is_err());
        assert!(Uniform::new(1.0, 1.0).is_err());
        assert!(Uniform::new(-f64::MAX, f64::MAX).is_err());
        assert!(Normal::new(0.0, 0.0).is_err());
        assert!(Normal::new(0.0, f64::MAX).is_err());
        assert!(Poisson::new(-1.0).is_err());
        assert!(Normal::new(0.0, 1.0).unwrap().cdf(f64::NAN).is_err());
    }

    #[test]
    fn bounded_discrete_mass_normalizes() {
        let binomial = Binomial::new(25, 0.37).unwrap();
        let total = (0..=25).map(|k| binomial.pmf(k)).sum::<f64>();
        assert_close(total, 1.0, 1.0e-12);

        let poisson = Poisson::new(4.0).unwrap();
        let total = (0..=40).map(|k| poisson.pmf(k)).sum::<f64>();
        assert_close(total, 1.0, 1.0e-12);
    }

    proptest! {
        #[test]
        fn cdfs_are_monotone_and_bounded(
            probability in 0.0f64..=1.0,
            lambda in 0.0f64..20.0,
            left in -20.0f64..20.0,
            width in 0.01f64..20.0,
        ) {
            let bernoulli = Bernoulli::new(probability).unwrap();
            prop_assert!((0.0..=1.0).contains(&bernoulli.cdf(0)));
            prop_assert!(bernoulli.cdf(0) <= bernoulli.cdf(1));

            let binomial = Binomial::new(20, probability).unwrap();
            let mut previous = 0.0;
            for k in 0..=20 {
                let value = binomial.cdf(k);
                prop_assert!((0.0..=1.0).contains(&value));
                prop_assert!(value + 1.0e-14 >= previous);
                previous = value;
            }

            let poisson = Poisson::new(lambda).unwrap();
            let mut previous = 0.0;
            for k in 0..=40 {
                let value = poisson.cdf(k);
                prop_assert!((0.0..=1.0).contains(&value));
                prop_assert!(value + 1.0e-14 >= previous);
                previous = value;
            }

            let uniform = Uniform::new(left, left + width).unwrap();
            let lower = uniform.cdf(left).unwrap();
            let middle = uniform.cdf(left + width * 0.5).unwrap();
            let upper = uniform.cdf(left + width).unwrap();
            prop_assert!(lower <= middle && middle <= upper);

            let normal = Normal::new(left, width).unwrap();
            let lower = normal.cdf(left - width).unwrap();
            let middle = normal.cdf(left).unwrap();
            let upper = normal.cdf(left + width).unwrap();
            prop_assert!(lower <= middle && middle <= upper);
        }
    }
}
