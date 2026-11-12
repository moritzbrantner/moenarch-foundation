# vector-analysis-index

Exact and deterministic approximate in-memory vector search for `moritzbrantner-video-analysis`.

The exact `VectorSearchIndex` remains the reference implementation for arbitrary supported metrics. `approximate::CosineLshIndex` adds bounded candidate selection for cosine search using deterministic seeded random-hyperplane LSH; candidates are still ranked with exact cosine similarity. The optional `hnsw` feature adds `hnsw::CosineHnswIndex`, which selects candidates with the external [`hnsw_rs`](https://docs.rs/hnsw_rs) graph and ranks them the same exact way.

## Ownership boundary

This crate owns vector records, metric/filter semantics, exact and approximate index policy, result types, and deterministic ID tie-breaking. Generic bounded top-k retention is delegated to `search-kernels` in `moritzbrantner/rust-kernels`; that kernel does not own vector scoring or filtering semantics.

## Feature flags

- `work-counters`: test-only deterministic work counters (`work` module) for the complexity contracts in `.performance/contract.json` (#153); every counter call is a no-op without it.
- `hnsw`: enables `hnsw::CosineHnswIndex` over the external `hnsw_rs` crate (#119). Off by default; it pulls a native-only dependency graph (`rayon`, `mmap-rs`, `parking_lot`, `env_logger`, `cpu-time`, `num_cpus`, ...) and is not part of the WASM bindings.

## Exact search

```rust,ignore
use vector_analysis_core::{DenseVector, VectorMetric};
use vector_analysis_index::{SearchConfig, VectorRecord, VectorSearchIndex};

let index = VectorSearchIndex::from_records([
    VectorRecord::new("a", DenseVector::new(vec![1.0, 0.0, 0.0])?),
])?;
let results = index.search(
    &DenseVector::new(vec![0.9, 0.1, 0.0])?,
    SearchConfig { metric: VectorMetric::Cosine, limit: 5 },
)?;
```

## Approximate cosine search

```rust,ignore
use vector_analysis_core::DenseVector;
use vector_analysis_index::{
    approximate::{CosineLshConfig, CosineLshIndex, CosineLshSearchConfig},
    VectorRecord,
};

let index = CosineLshIndex::from_records(
    CosineLshConfig::default(),
    [VectorRecord::new("a", DenseVector::new(vec![1.0, 0.0, 0.0])?)],
)?;
let report = index.search(
    &DenseVector::new(vec![0.9, 0.1, 0.0])?,
    CosineLshSearchConfig {
        limit: 5,
        probe_radius: 1,
        max_candidates: 256,
    },
)?;

assert!(report.candidate_count <= 256);
```

`hash_bits` controls bucket width. `probe_radius` controls how many nearby signatures are visited, while `max_candidates` is a hard bound on the number of vectors scored exactly. Search reports expose both candidate and bucket counts so approximation effort is evidence rather than hidden policy.

The query cost model has distinct dimensions: vector dimensions `D`, signature bits `B` (at most 32), Hamming radius `R` (at most 3), and exact-candidate budget `C`. Constructing the query signature takes `O(B * D)` arithmetic; at most `sum(combinations(B, i), i = 0..R)` bucket signatures are visited (5,489 at `B=32, R=3`), and no more than `C` record vectors are ranked exactly. Probe signatures are generated lazily and are not allocated as one whole list when the candidate cap terminates a query early. Bucket lookups, result selection, and legitimate dense buckets have their own costs; this is a bounded-scoring guarantee, **not** a claim of guaranteed sublinear search time or exact nearest-neighbor recall.

## External HNSW cosine search (`hnsw` feature)

```rust,ignore
use vector_analysis_index::hnsw::{CosineHnswConfig, CosineHnswIndex, CosineHnswSearchConfig};

let index = CosineHnswIndex::from_records(CosineHnswConfig::default(), records)?;
let report = index.search(&query, CosineHnswSearchConfig { limit: 10, ef_search: 64 })?;
assert!(report.candidate_count <= 64);
```

`hnsw_rs` types stay private. The graph only proposes `ef_search` candidates; each is re-scored with exact cosine similarity and ordered by `(distance, id)`, so whenever the candidate set contains the exact top-K the output is identical to `VectorSearchIndex` (scores, order and ties). The index is immutable and built serially in record order; `hnsw_rs` seeds node levels with a fixed internal seed, so rebuilding from the same records and configuration reproduces the same results on the same build and target (checked by the unit tests and the parity example). Parallel insertion is not exposed because it depends on thread scheduling, and no cross-target bit-identity is claimed. Zero-norm records and queries are rejected because cosine is undefined for them. Metadata filters are not supported here; filtered search stays on the exact index.

The three-way exact/LSH/HNSW parity and cost report is repeatable:

```bash
cargo run --release -p moenarch-vector-analysis-index --features hnsw --example ann_three_way [corpus sizes...]
```

Measured results, the keep/external decision, deployment costs and the fallback are recorded in [`docs/specs/vector-ann-backend.md`](../../../docs/specs/vector-ann-backend.md).

## Package surface

Primary workflow: `vector.index.search`.

Workflow operations:

- `vector.index.search`: Builds the exact in-memory index and returns nearest records for a dense query vector.
- `vector.index.centroids`: Assigns each dense vector to the nearest centroid using the selected metric.

The first LSH slice is library-first; package-surface exposure can be added after the API is calibrated against exact search.

Debug operations:

- `describe`: inspect package metadata and runtime support.

Runtime support: library, CLI, server, and WASM wrappers expose the existing exact operations.

Run the primary workflow through the CLI:

```bash
cargo run -p moritzbrantner-vector-analysis-index-cli -- run \
  --operation vector.index.search \
  --json '{"limit":1,"metric":"cosine","query":[1.0,0.0],"records":[{"id":"a","vector":[1.0,0.0]}]}'
```

Successful responses use the shared package-surface shape with `operation`,
`title`, `message`, `summary`, and `result`. Default surface calls are
deterministic, local-first, and do not download models, write persistent files,
or execute external tools unless an operation explicitly documents native or
external-tool execution.

## Related crates

- `vector-analysis-core`
- `text-embeddings`
