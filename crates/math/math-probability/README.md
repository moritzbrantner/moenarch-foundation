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

Normal, Binomial and Poisson CDF calculations delegate to [statrs 0.19.1](https://docs.rs/statrs/0.19.1/statrs/) (MIT), with default features disabled and only `std` enabled. This avoids importing sampling, RNG, or optional numerical matrix backends. The Foundation contract retains stricter finite validation, stable public types, and special handling of valid degenerate distributions; in particular, `Poisson::new(0.0)` remains supported despite being rejected by statrs.

Normal CDF uses the backend's complementary error function rather than the former local `~1e-7` approximation. Binomial and positive-rate Poisson CDFs use incomplete beta/gamma functions rather than a recurrence proportional to the count argument. Input-dependent convergence work remains within statrs' numerical routines; performance comparisons must be made on representative values. Probability mass/log-mass calculations retain their existing local formulas for now, preserving their established edge and underflow semantics pending separate parity evidence. Existing public return and error types are unchanged.

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
