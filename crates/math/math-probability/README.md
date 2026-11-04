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

Normal, Binomial and Poisson CDF calculations delegate to [cdflib 0.4.4](https://docs.rs/cdflib/0.4.4/cdflib/) (MIT, pure Rust, no default features, single dependency `thiserror`), a line-by-line port of CDFLIB's `cumnor` and of the DiDonato–Morris incomplete gamma (ACM TOMS 654) and incomplete beta (ACM TOMS 708) ratios. Poisson uses `Q(k + 1, lambda)`, Binomial uses `I_{1-p}(n - k, k + 1)` with `p` passed as the exact complement, and Normal uses `cumnor((x - mean) / std_dev)`. Each routine dispatches between bounded series, continued fractions, and Temme-style asymptotic expansions, so CDF work does not grow with `k`, `n`, or `lambda` (see the backend decision below). The Foundation contract retains its stricter finite validation, stable public types, and local handling of valid degenerate distributions: `Poisson::new(0.0)`, zero trials, `p = 0` / `p = 1`, `k >= n`, and standardizations that overflow (clamped to `|z| <= 40`, where the CDF is already exactly `0` or `1`). Counts above 2^53 are evaluated at their nearest `f64`, as before.

Probability mass/log-mass calculations retain their existing local formulas, preserving their established edge and underflow semantics. Existing public return and error types are unchanged.

### CDF accuracy policy

For scale `s` (`1` for Normal, `lambda` for Poisson, `n` for Binomial) a CDF value must be within `8 ε √max(1, s)` absolute and, when the true value is at least `1e-300`, within `64 ε √max(1, s)` relative of a 45-digit reference. The `√s` factor is the binary64 conditioning of the ratio `x / a` that every incomplete-function algorithm forms; it reaches about `2e-8` absolute only at `lambda ≈ 1e18`. True values below `1e-300` may underflow to `0`. `tests/cdf_differential.rs` enforces this policy on 363 committed reference cases in `tests/data/cdf_reference.csv` (generated offline with mpmath by `tests/data/generate_cdf_reference.py`), covering degenerate parameters, extreme tails, underflow, `u64::MAX` counts, and rates and trial counts up to `1e18`.

### Backend decision (issue #117)

Decision: **delegate to cdflib; keep statrs as a dev-only differential oracle.** statrs 0.19.1 was the first external candidate (production since #121) but does not satisfy the Foundation contract on valid large arguments:

| Evidence (reference = 45-digit mpmath) | cdflib 0.4.4 | statrs 0.19.1 |
| --- | --- | --- |
| Normal CDF, max abs error on `z ∈ [-38.5, 9]` | `1.1e-16` | `1.4e-11` |
| Poisson, `lambda ≤ 1e3`, max abs error | `2.6e-15` | `7.0e-14` |
| Poisson `lambda = k = 1e9` / `1e12` | `3.4e-13` / `8.8e-12` | `1.3e-6` / `9.3e-4` |
| Poisson `lambda ≥ 1e14` near the mode | `≤ 2e-8` (at `1e18`) | O(√lambda) series: 0.5 s per call at `1e15`, no result within 20 s at `1e16` |
| Binomial `n ≤ 1e5`, max abs error | `5.6e-16` | `4.5e-11` |
| Binomial `n = 1e6`, `p = 0.5`, median | `< 1e-16` | `1.2e-3` |
| Binomial `n = 1e8`, `p = 0.5`, median | `< 1e-16` | returns `-0.54` (clamped to `0`) |
| Binomial `n = 1e18`, `p = 1e-17`, `k = 10` | `3.3e-16` | returns `1.0` (true `0.583`) |

statrs' incomplete beta continued fraction stops after 140 iterations and its prefactor cancels catastrophically for large `n`; its incomplete gamma has no iteration bound. These mismatches are recorded as assertions in `statrs_defects_on_large_arguments_are_recorded` so a future statrs fix is noticed.

Other criteria:

- **Bounded work.** No O(k) loop remains on a public path. A sweep of 2,248 Poisson (`lambda` from `1e-300` to `f64::MAX`, `k` up to `u64::MAX`) and 20,021 Binomial (`n` up to `u64::MAX`, `p` from `1e-300` to `1 - 1e-12`) evaluations, including the regime boundaries, finished with a worst single call of 2.8 µs and 31 µs respectively; the Criterion workloads below show flat cost from `lambda = 3` to `1e18`. cdflib reports `Indeterminate` only for `a > 6.6e28`, unreachable from `u64` counts.
- **Dependencies, license, MSRV, portability.** cdflib adds one package (`thiserror` was already in the graph); statrs pulled `approx` and `num-traits` into the runtime graph and now remains a dev-dependency only. cdflib is MIT (compatible with this crate's MIT OR Apache-2.0), MSRV 1.71 (workspace 1.95), pure safe Rust with no target-specific code, and is checked upstream against Fortran 90 reference tables bit for bit plus Miri.
- **Maintenance.** cdflib is young (first release 2026-05, single maintainer) while statrs is widely used; this risk is mitigated by the exact lockfile pin, the committed reference table, and the statrs differential test, which together detect any behavioral change on upgrade. The production seam stays private: no backend type or selector appears in the public API.

Benchmark (`cargo bench --locked -p moenarch-math-probability --bench cdf_reference -- --warm-up-time 1 --measurement-time 3`), Criterion median per call. `foundation` is the production path, `legacy` the original O(k) first-party recurrence; `—` marks a workload that is not measured: the legacy recurrence or the statrs series is O(k) or O(√lambda) there (statrs takes 0.5 s per call at `1e15`), and the former local normal approximation is not retained.

| Workload | foundation (cdflib) | statrs | legacy |
| --- | --- | --- | --- |
| `poisson_cdf/lambda_3_k_2` | 13.6 ns | 76.8 ns | 70.5 ns |
| `poisson_cdf/lambda_30_k_30` | 69.7 ns | 171.6 ns | 1.07 µs |
| `poisson_cdf/lambda_100_k_100` | 70.5 ns | 254.2 ns | 3.28 µs |
| `poisson_cdf/lambda_100_k_10000` | 13.2 ns | 31.5 ns | 241.02 µs |
| `poisson_cdf/lambda_1e4_k_1e4` | 58.5 ns | 2.09 µs | 341.96 µs |
| `poisson_cdf/lambda_1e6_k_1e6` | 60.8 ns | 18.85 µs | — |
| `poisson_cdf/lambda_1e9_k_1e9` | 44.2 ns | 564.83 µs | — |
| `poisson_cdf/lambda_1e15_k_1e15` | 44.8 ns | — | — |
| `poisson_cdf/lambda_1e18_k_1e18` | 42.7 ns | — | — |
| `poisson_cdf/lambda_3_k_u64max` | 12.9 ns | 31.3 ns | — |
| `binomial_cdf/n_10_p_0.5_k_3` | 129.7 ns | 156.9 ns | 124.7 ns |
| `binomial_cdf/n_1000_p_0.001_k_2` | 115.1 ns | 181.3 ns | 94.4 ns |
| `binomial_cdf/n_1e5_p_0.25_k_25000` | 211.4 ns | 3.11 µs | 1.01 ms |
| `binomial_cdf/n_1e8_p_0.5_k_5e7` | 143.0 ns | 2.93 µs | — |
| `binomial_cdf/n_1e12_p_0.25_k_2.5e11` | 143.2 ns | 2.77 µs | — |
| `binomial_cdf/n_1e18_p_1e-17_k_10` | 162.5 ns | 21.0 ns | — |
| `normal_cdf/z_-1` | 24.3 ns | 15.5 ns | — |
| `normal_cdf/z_3` | 26.0 ns | 17.6 ns | — |
| `normal_cdf/z_-20` | 24.6 ns | 17.5 ns | — |

Performance trade-off: the production path is faster than statrs on every Poisson workload and on every Binomial workload except `n_1e18_p_1e-17_k_10`, where statrs is faster only because it returns a wrong value; normal CDF costs about 8 ns more than statrs per call in exchange for ~1e5 times smaller error.

Environment fingerprint: source `004da7f` plus this change, `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1`, `RUSTFLAGS` unset, release profile, Linux 6.18.44 x86_64 on a shared 4-vCPU Intel Xeon @ 2.10GHz VM (other workloads running; timings are indicative, not a gate). The Cargo.lock SHA-256 is recorded in the commit that adds this report.

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
