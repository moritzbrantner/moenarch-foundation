# math-probability

Deterministic probability-distribution evaluation for domain-neutral analytical workflows.

This crate owns probability semantics, parameter validation, moments, and probability evaluation. It deliberately does **not** own random-number generation, seeding, sampling policy, fitting, Bayesian inference, stochastic processes, or Monte Carlo orchestration.

## Initial distributions

- `Bernoulli`: probability of success in `[0, 1]`
- `Uniform`: finite continuous interval with strictly positive width
- `Normal`: finite mean and strictly positive finite standard deviation
- `Binomial`: finite Bernoulli trial count plus success probability in `[0, 1]`
- `Poisson`: finite non-negative event rate

Continuous distributions expose `pdf`, `log_pdf`, and `cdf`. Discrete distributions expose `pmf`, `log_pmf`, and `cdf`. Every distribution exposes deterministic mean and variance.

## Numerical behavior

All parameters and continuous evaluation points must be finite. Invalid parameters return the foundation's typed invalid-argument error instead of creating `NaN` state.

Zero probability is represented as `0.0`; its logarithm is mathematically `-∞`, so `log_pmf`/`log_pdf` may return negative infinity only when the corresponding probability is exactly zero. Valid CDF results are clamped to `[0, 1]` against final rounding drift.

Normal CDF uses a compact deterministic error-function approximation with absolute error on the order of `1e-7` for ordinary finite inputs. It is suitable for local analytics and referenceable package behavior, not arbitrary-precision tail computation.

Binomial and Poisson probability mass use logarithmic formulations rather than factorial arithmetic. Their CDFs use log-space recurrence and log-addition, avoiding factorial overflow and preserving small terms substantially longer than naïve probability-space accumulation.

## Examples

```rust,no_run
use math_probability::{Binomial, Normal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let normal = Normal::new(0.0, 1.0)?;
    assert!((normal.cdf(0.0)? - 0.5).abs() < 1e-7);

    let binomial = Binomial::new(10, 0.5)?;
    assert!((binomial.pmf(5) - 0.24609375).abs() < 1e-12);
    Ok(())
}
```

## Boundary

This crate is intentionally evaluation-only. A future sampling layer may consume these parameter contracts, but introducing an RNG dependency here would mix mathematical distribution semantics with execution policy and make deterministic consumers pay for behavior they do not need.
