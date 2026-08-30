# Exact decimal contract

`numbers-core::ExactDecimal` provides domain-neutral, fixed-scale base-10
mechanics for consumers that cannot store monetary-style values in binary
floating point. It does not model currency, money, accounts, tax, interest,
cash flows, day-count conventions, basis points, portfolios, or accounting.
Those meanings remain in finance capability crates above foundation.

## Dependency decision

The crate uses a hybrid built-on-established-storage design. `rust_decimal`
1.42.1 owns the bounded 96-bit coefficient representation, parsing, formatting,
comparison, and Serde interoperability. Foundation owns small exact
coefficient-arithmetic adapters for add/subtract, multiply, and rounded divide;
these prevent the backend's implicit precision reduction and avoid false
overflow caused only by a narrow intermediate. `numbers-core` selects only
`rust_decimal`'s `std` and `serde` features, and its existing WASM binding
compiles for `wasm32-unknown-unknown` with that configuration.

Alternatives considered:

- `fixed` is fixed-point, but its binary radix is not a base-10 monetary
  representation and the current line is alpha.
- `bigdecimal` provides arbitrary precision, but the foundation need is a
  bounded, deterministic contract with checked scale/overflow behavior rather
  than unbounded arithmetic and dependency weight.
- A fully bespoke decimal type would duplicate parsing, storage, formatting,
  comparison, and interchange behavior. The selected hybrid keeps those
  established surfaces while implementing only the exactness checks the
  backend arithmetic does not guarantee.

## Interface and serialization

`ExactDecimal` parses strict base-10 strings and serializes as a JSON string.
The string preserves stored scale and trailing zeroes: `1.2300` serializes as
`"1.2300"`, and JSON numbers are rejected during deserialization. This avoids a
JSON number being parsed through `f64` by another runtime. Its maximum scale is
28; operations return typed errors on invalid scale, overflow, division by
zero, non-finite float input, and non-integral integer conversion.

All rounding requires both a target scale and `RoundingMode`. Addition,
subtraction, and multiplication return `Overflow` when the exact result cannot
fit the 96-bit coefficient and scale-28 representation; they never reduce
precision implicitly. Division applies the requested scale and mode directly to
the exact coefficient ratio, so no intermediate decimal rounding can move a
value across a midpoint.

Addition and subtraction preserve the greater stored operand scale whenever the
exact coefficient fits; they remove trailing zeroes only when required to fit
an otherwise representable exact result. Multiplication returns a normalized
product, and multiplication by zero returns scale-zero `0`. Division always
returns the requested scale. These choices make the resulting stable wire
string deliberate rather than an accidental property of the wrapped crate.

`RoundingMode` uses these stable Serde variant strings:
`MidpointNearestEven`, `MidpointAwayFromZero`, `MidpointTowardZero`,
`TowardZero`, `AwayFromZero`, `TowardNegativeInfinity`, and
`TowardPositiveInfinity`.

Float crossing is intentionally named: `from_f64_retain` imports the float's
represented decimal digits, while `to_f64_lossy` permits export. There is no
implicit `From<f64>` conversion. `to_i128_exact` succeeds only for integral,
representable values.

`math-statistics` continues to own floating-point analytics. A consumer that
needs analysis converts an exact decimal at its own documented boundary using
`to_f64_lossy`; this slice does not generalize statistical algorithms to
decimal arithmetic.

## Verification

Property tests cover parse/scale-preserving Serde round trips, same- and
cross-scale exact add/subtract identities, representable multiplication,
rounding invariants, coefficient boundaries, and lossless conversion ranges.
Golden and boundary
tests cover every rounding mode for positive and negative midpoints, explicit
division around coefficient-sized midpoints, directed rounding of tiny ratios,
maximum coefficient and scale, multiplication underflow, overflow, arithmetic
result scales, JSON string shape, JSON-number rejection, and the stable rounding
mode names. The existing numbers WASM surface is compiled for
`wasm32-unknown-unknown`; no finance-domain type is introduced.
