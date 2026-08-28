# Shared numerical contracts

F2 makes only three cross-crate semantics common. The layer lives in
`moenarch-numbers-core` because it is already the scalar numerical owner; no
new package or universal numeric hierarchy is introduced.

| Contract | Consumers | Semantics |
| --- | --- | --- |
| `is_finite_f32_slice` / `is_finite_f64_slice` | `vector-analysis-core`, `math-linear` | A predicate over primitive floating-point slices. Each consumer retains its established error type and message. |
| `checked_f64_to_f32` | `math-linear`, `math-statistics` | Returns `Some(f32)` only for finite values within the inclusive `f32` range; otherwise returns `None`. Consumers map rejection to their existing errors. |
| `ApproxTolerance` | F1 test support now; F3 linear law/reference boundaries and F5 rotation/transform law boundaries next | An explicit absolute-and-relative comparison value. It has no default epsilon and returns `false` for non-finite operands. |

`ApproxTolerance::new` requires finite non-negative limits. `allows_f32` and
`allows_f64` accept a comparison when either its absolute error or relative
error is within the supplied limit. The caller selects the value at an
algorithm or test boundary; algorithm-derived rank, pivot, and convergence
thresholds remain local to their owners.

## Explicit exclusions

- No `Number`, `Scalar`, algebraic typeclass, or common arithmetic trait.
- No implicit default tolerance or one global epsilon.
- No conversion between floats and exact/decimal representations; F6 owns that
  contract because rounding and scale must be explicit.
- No third-party backend type in a foundation public signature.
- No shared error type: existing owners preserve their documented errors and
  serialization behavior.
