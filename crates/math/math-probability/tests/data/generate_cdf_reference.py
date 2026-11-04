#!/usr/bin/env python3
"""Regenerate `cdf_reference.csv`: high-precision CDF reference values.

Offline fixture generator only; the Rust tests are the oracle. Requires
`mpmath` (tested with 1.4.1). Run from this directory:

    python3 generate_cdf_reference.py > cdf_reference.csv

Every value is evaluated at the exact binary64 arguments Foundation passes to
its backend: Poisson uses Q(f64(k) + 1, lambda); Binomial uses
I_{1-p}(f64(n - k), f64(k) + 1) with the exact real 1 - p. Methods:

* normal: `mpmath.ncdf`;
* Poisson, lambda <= 1e5: `mpmath.gammainc(regularized=True)`;
* Poisson, lambda > 1e5: adaptive Gauss-Legendre quadrature of the gamma
  density around its peak (independent of any asymptotic expansion);
* Binomial, a + b < 2e4: `mpmath.betainc(regularized=True)`;
* Binomial, otherwise: the same quadrature approach for the beta density.
"""

import math

import mpmath as mp

mp.mp.dps = 45


def _pieces(lo, hi, anchor_is_hi, scale):
    if anchor_is_hi:
        start = max(lo, hi - 80 * scale)
        return [start + (hi - start) * mp.mpf(i) / 64 for i in range(65)]
    end = min(hi, lo + 80 * scale)
    return [lo + (end - lo) * mp.mpf(i) / 64 for i in range(65)]


def gamma_q(a, x):
    a, x = mp.mpf(a), mp.mpf(x)
    if x <= 1e5:
        return mp.gammainc(a, x, mp.inf, regularized=True)
    log_gamma = mp.loggamma(a)
    density = lambda t: mp.exp((a - 1) * mp.log(t) - t - log_gamma)
    slope = abs((a - 1) / x - 1)
    scale = min(mp.sqrt(a), 1 / slope) if slope > 0 else mp.sqrt(a)
    if x <= a - 1:
        return 1 - mp.quad(density, _pieces(mp.mpf(0), x, True, scale))
    return mp.quad(density, _pieces(x, mp.inf, False, scale))


def beta_i(a, b, x):
    a, b, x = mp.mpf(a), mp.mpf(b), mp.mpf(x)
    if a + b < 2e4:
        return mp.betainc(a, b, 0, x, regularized=True)
    log_beta = mp.loggamma(a) + mp.loggamma(b) - mp.loggamma(a + b)
    density = lambda t: mp.exp((a - 1) * mp.log(t) + (b - 1) * mp.log1p(-t) - log_beta)
    total = a + b
    mode = (a - 1) / (total - 2)
    width = mp.sqrt(mode * (1 - mode) / total)
    slope = abs((a - 1) / x - (b - 1) / (1 - x))
    if width == 0:  # mode on the support boundary (b = 1 or a = 1)
        scale = 1 / slope
    else:
        scale = min(width, 1 / slope) if slope > 0 else width
    if x <= mode:
        return mp.quad(density, _pieces(mp.mpf(0), x, True, scale))
    return 1 - mp.quad(density, _pieces(x, mp.mpf(1), False, scale))


def cases():
    for z in [-38.5, -37.0, -30.0, -20.0, -10.0, -5.0, -3.0, -1.0, -1e-8, 0.0,
              1e-8, 0.5, 1.0, 2.0, 3.0, 5.0, 8.0, 8.3, 9.0]:
        yield ("normal", repr(z), "", "", mp.ncdf(mp.mpf(z)))
    for lam in [1e-300, 1e-8, 0.1, 1.0, 3.0, 19.5, 30.0, 100.0, 1e3, 1e4, 1e5,
                1e6, 1e7, 1e9, 1e12, 1e15, 1e18]:
        sigma = math.sqrt(lam)
        ks = {0, 1, 2, 10, 100, 2**64 - 1}
        for z in [-38, -10, -3, -1, 0, 1, 3, 10, 38]:
            k = int(lam + z * sigma)
            if 0 <= k < 1.8e19:
                ks.add(k)
        for k in sorted(ks):
            yield ("poisson", repr(lam), "", str(k), gamma_q(float(k) + 1.0, lam))
    for n, p in [(1, 0.3), (10, 0.5), (40, 0.15), (100, 0.95), (1000, 1e-3),
                 (10**4, 0.5), (10**5, 0.01), (10**6, 0.5), (10**8, 0.5),
                 (10**8, 1e-6), (10**12, 0.25), (10**15, 0.999), (10**18, 1e-17),
                 (10**18, 0.5)]:
        sigma = math.sqrt(n * p * (1 - p))
        ks = {k for k in (0, 1, n - 1) if k < n}
        for z in [-38, -10, -3, -1, 0, 1, 3, 10, 38]:
            k = int(n * p + z * sigma)
            if 0 <= k < n:
                ks.add(k)
        for k in sorted(ks):
            value = beta_i(float(n - k), float(k) + 1.0, 1 - mp.mpf(p))
            yield ("binomial", str(n), repr(p), str(k), value)


def main():
    print("kind,parameter,probability,k,reference")
    for kind, parameter, probability, k, value in cases():
        print(f"{kind},{parameter},{probability},{k},{float(value)!r}", flush=True)


if __name__ == "__main__":
    main()
