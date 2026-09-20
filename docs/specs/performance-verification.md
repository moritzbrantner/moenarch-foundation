# Performance verification contract

Performance evidence complements correctness verification; it does not replace it.

## Evidence layers

- Existing Criterion suites remain the broad developer-facing benchmark matrix. They are not migrated solely for framework uniformity and are not interpreted as stable wall-clock gates on ordinary shared CI runners.
- Performance-sensitive leaves may expose a deliberately small Iai-Callgrind suite through the `benchmark:smoke` semantic capability.
- The first sentinel is `moenarch-vector-analysis-core`: dot product at 768 dimensions, cosine similarity at 768 dimensions, and mean-vector aggregation over 256 vectors of 128 dimensions.
- Callgrind instruction reads are treated as a deterministic regression proxy, not as equivalent to latency.

## Regression boundary

The smoke suite uses a 5% relative instruction-read threshold. Pull requests compare the candidate with the pull-request base when that base already contains the compatible benchmark. The initial adoption seeds the contract instead of inventing a historical comparison.

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

Reference or benchmark-only dependencies stay outside public APIs and production runtime selection.
