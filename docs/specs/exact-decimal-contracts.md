# Exact decimal contract

`numbers-core::ExactDecimal` provides domain-neutral, fixed-scale base-10
mechanics for consumers that cannot store monetary-style values in binary
floating point. It does not model currency, money, accounts, tax, interest,
cash flows, day-count conventions, basis points, portfolios, or accounting.
Those meanings remain in finance capability crates above foundation.

## Dependency decision

The crate wraps `rust_decimal` 1.42.1 rather than implementing decimal
arithmetic locally. It is a maintained pure-Rust decimal implementation with
Rust 1.67 compatibility, checked arithmetic, explicit rounding strategies,
serde support, and a WASM feature path. `numbers-core` selects only its `std`
and `serde` features; its existing WASM binding compiles for
`wasm32-unknown-unknown` with that configuration.

Alternatives considered:

- `fixed` is fixed-point, but its binary radix is not a base-10 monetary
  representation and the current line is alpha.
- `bigdecimal` provides arbitrary precision, but the foundation need is a
  bounded, deterministic contract with checked scale/overflow behavior rather
  than unbounded arithmetic and dependency weight.
- A bespoke decimal implementation would duplicate well-tested arithmetic and
  rounding behavior without adding a domain-specific advantage.

## Interface and serialization

`ExactDecimal` parses strict base-10 strings and serializes as a JSON string.
This avoids a JSON number being parsed through `f64` by another runtime. Its
maximum scale is 28; operations return typed errors on invalid scale, overflow,
division by zero, non-finite float input, and non-integral integer conversion.

All rounding requires both a target scale and `RoundingMode`. Addition,
subtraction, and multiplication are checked. Division requires scale and mode;
repeating fractions are bounded by the underlying 28-digit representation
before that explicitly requested final rounding step.

Float crossing is intentionally named: `from_f64_retain` imports the float's
represented decimal digits, while `to_f64_lossy` permits export. There is no
implicit `From<f64>` conversion. `to_i128_exact` succeeds only for integral,
representable values.

`math-statistics` continues to own floating-point analytics. A consumer that
needs analysis converts an exact decimal at its own documented boundary using
`to_f64_lossy`; this slice does not generalize statistical algorithms to
decimal arithmetic.
