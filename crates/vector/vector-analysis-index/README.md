# vector-analysis-index

Exact and deterministic approximate in-memory vector search for `moritzbrantner-video-analysis`.

The exact `VectorSearchIndex` remains the reference implementation for arbitrary supported metrics. `approximate::CosineLshIndex` adds bounded candidate selection for cosine search using deterministic seeded random-hyperplane LSH; candidates are still ranked with exact cosine similarity.

## Feature flags

- No optional feature flags today.

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
