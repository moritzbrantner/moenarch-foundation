# Performance verification contract

Performance evidence complements correctness verification; it does not replace it.

## Evidence layers

- Existing Criterion suites remain the broad developer-facing benchmark matrix. They are not migrated solely for framework uniformity and are not interpreted as stable wall-clock gates on ordinary shared CI runners.
- Performance-sensitive leaves may expose a deliberately small Iai-Callgrind suite through the `benchmark:smoke` semantic capability.
- The first sentinel is `moenarch-vector-analysis-core`: dot product at 768 dimensions, cosine similarity at 768 dimensions, and mean-vector aggregation over 256 vectors of 128 dimensions.
- `moenarch-numbers-core` covers ordinary and opposite-sign extreme relative comparisons, valid/reversed range clamping, and valid/reversed JSON deserialization.
- `moenarch-math-geometry-3d` covers rounded f32 rotation import, dense matrix inversion at scales `1`, `1e-200`, and `1e200`, and ordinary/tiny axis-angle export. Fixtures are prepared outside the measured region; inputs and outputs pass through `black_box`.
- Callgrind instruction reads are treated as a deterministic regression proxy, not as equivalent to latency.

## Regression boundary

The smoke suite uses a 5% relative instruction-read threshold. Pull requests compare the candidate with the pull-request base when that base already contains the compatible benchmark. The initial adoption seeds the contract instead of inventing a historical comparison.

Every suite runs in the blocking `performance-smoke` CI job. Baseline failures,
candidate regressions, invalid baseline revisions, and changed workload source
fail the driver; they cannot silently become successful seed runs. Workload
changes require a reviewed baseline transition, for example a newly named suite
with its own first-adoption record. Timing is not a blocking metric, and no
absolute latency budget is inferred from these instruction counts.

The numerical correctness tests separately reject the six historical failures.
An optimization cannot pass by returning an error or dropping validation where
the public contract promises a valid result. After packaging, CI runs
`python3 scripts/check_numerical_package_tests.py` to execute tests from the
three extracted numerical crate archives. Missing helper files, external helper
links, and archives from a stale or dirty revision fail before compilation.
Only disposable archive lockfiles are adapted for exact local source patches;
the repository lockfile remains unchanged.

Run the bounded gate with:

```sh
PERF_BASE_SHA=<base-commit> bash scripts/benchmark-smoke.sh
```

Run the broader existing Criterion evidence with:

```sh
cargo bench --locked -p moenarch-vector-analysis-core --bench metrics
```

## Reproducibility

Smoke evidence is written to `.artifacts/performance-smoke/` together with a fingerprint containing candidate and baseline revisions, Rust/Cargo versions, `RUSTFLAGS`, Cargo lock hash, Valgrind and Iai-Callgrind runner versions, and host architecture.

Measurements from incompatible fingerprints are not directly comparable. Deliberate compiler, target, algorithm, dependency, or workload changes may require a reviewed baseline transition rather than weakening the threshold.

Per-suite logs and workload SHA-256 hashes are retained alongside the shared
fingerprint in the uploaded CI artifact. The measured unit is Callgrind `Ir`
(instruction reads), lower is better, and each fixed scenario is measured once
under the same instrumentation for the baseline and candidate.

Baseline and candidate compilation use separate Cargo target directories keyed
by source revision. Only Callgrind measurements are shared between them, avoiding
stale dependency reuse when Git worktree timestamps differ from source history.

Reference or benchmark-only dependencies stay outside public APIs and production runtime selection.
