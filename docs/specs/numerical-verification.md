# Numerical verification tiers

This policy is introduced by F1. It defines test support and evidence tiers;
it does not define a production tolerance policy or alter public APIs.

## Shared support

`crates/test-support/numerical.rs` is the shared source module for numerical
integration tests. It provides finite and non-finite scalar generators, small
vector and rectangular-matrix generators, singular and ill-conditioned matrix
generators, deterministic Proptest configuration, and diagnostics over the
production `numbers_core::ApproxTolerance` contract.

It is intentionally not a Cargo package. The repository's ownership audit and
boundary check require every workspace package to be represented in the
canonical ownership inventory, so callers include the module with `#[path]`.
This keeps test-only machinery outside every production crate's public
interface and avoids creating a package boundary solely for one initial
consumer. The module becomes more valuable as F3 and F5 add their independent
consumers.

Property tests use 64 cases with the fixed seed `0x4d4f_454e_4152_4348`. A
failure is therefore locally reproducible with the same test command. Change
the seed only deliberately when extending coverage; do not hide a regression by
rotating it.

Approximate comparisons always name both an absolute and a relative tolerance.
They pass when either criterion is satisfied and print both limits and the
offending matrix coordinate when applicable. These helpers do not choose a
tolerance for production operations.

## Tiers

| Tier | Purpose | Default status |
| --- | --- | --- |
| Fast crate tests | Unit tests and deterministic properties for one changed crate. | Required during local iteration. |
| Property/reference tests | Algebraic laws, adversarial inputs, and agreement with an independent calculation or library. | Run for the affected crate before handoff. |
| Workspace and repository checks | Metadata, boundary, release-plan, and script tests. | Required handoff evidence. |
| Benchmarks | Named representative Criterion scenarios; compare only against a recorded compatible baseline. | Optional; never inferred from test timing. |
| Mutation and coverage diagnostics | Find insensitive tests and missing paths. | Optional slow evidence; never a line-coverage threshold or a release gate by itself. |

The `math-linear` integration test demonstrates this end-to-end: transpose
involution, finite-input rejection, and matrix multiplication against an
independent straightforward reference implementation. No reference type enters
the `math-linear` public interface.

Run the focused fast tier with:

```bash
cargo test -p moenarch-math-linear --no-default-features --test numerical_verification
```

## Reference implementations

Use a reference only in test code and convert it to the foundation owner's
concrete result before comparing. `math-linear` continues to keep its optional
`nalgebra` and `faer` dependencies behind features; they are suitable for
additional differential cases but must not appear in public function
signatures. A direct, separately implemented reference is preferred for small
operations because it reduces correlated implementation errors.

The `nalgebra-backend` feature runs the same generated matrix multiplication
cases against `nalgebra::DMatrix` and converts its result to row-major values
before using the shared comparison helper:

```bash
cargo test -p moenarch-math-linear --no-default-features \
  --features nalgebra-backend --test numerical_verification
```

## Optional mutation path

`cargo-mutants` (currently 27.1.0) is suitable for a deliberately targeted,
slow diagnostic run, for example:

```bash
cargo mutants -p moenarch-math-linear --in-diff 5dcaac4
```

Run it only after ordinary affected tests are green, record its version and
selected scope with the evidence, and investigate surviving mutants. Do not add
it to the fast gate and do not turn its output into an arbitrary percentage
target.
