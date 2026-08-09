# vector-analysis-index

Exact in-memory vector search for `moritzbrantner-video-analysis`.

## Feature flags

- No optional feature flags today.

## Example

```rust,ignore
use vector_analysis_core::DenseVector;
use vector_analysis_index::VectorIndex;

let mut index = VectorIndex::new(3)?;
index.insert("a", DenseVector::new(vec![1.0, 0.0, 0.0])?)?;

let _ = index.search(DenseVector::new(vec![0.9, 0.1, 0.0])?.as_slice(), 5)?;
```

## Package surface

Primary workflow: `vector.index.search`.

Workflow operations:

- `vector.index.search`: Builds an in-memory index and returns nearest records for a dense query vector.
- `vector.index.centroids`: Assigns each dense vector to the nearest centroid using the selected metric.

Debug operations:

- `describe`: inspect package metadata and runtime support.

Runtime support: library, CLI, server, and WASM wrappers expose these operations.

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
