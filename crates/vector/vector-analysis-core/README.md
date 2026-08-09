# vector-analysis-core

Dense vector validation and metrics for `moritzbrantner-video-analysis`.

## Highlights

- Finite non-empty `f32` vectors
- Dot product, L2 norm, cosine similarity, Euclidean distance, and Manhattan distance
- Batch mean/min/max summaries for same-dimension vector sets
- Metric dispatch helper for ranking-oriented callers

## Feature flags

- No optional feature flags today.

## Example

```rust,no_run
use vector_analysis_core::{cosine_similarity, DenseVector};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = DenseVector::new(vec![1.0, 0.0, 0.0])?;
    let b = DenseVector::new(vec![0.9, 0.1, 0.0])?;

    let similarity = cosine_similarity(a.as_slice(), b.as_slice())?;
    assert!(similarity > 0.99);
    Ok(())
}
```

## Behavior

All public metric functions require non-empty finite input slices. Pairwise
metrics require equal dimensions and return an error for mismatches.

Cosine similarity and L2 normalization reject zero-norm vectors. This avoids
quietly producing `NaN` or arbitrary similarity scores for degenerate input.

`mean_vector` and `vector_stats` require a non-empty batch of `DenseVector`
values with identical dimensions. Statistics are plain arithmetic summaries
with no weighting.

`metric_distance(VectorMetric::Cosine, ...)` returns `1.0 - cosine_similarity`.
`VectorMetric::Dot` returns the negative dot product so callers that sort by
ascending distance can still rank larger dot products first.

## Runtime Surface

The package surface exposes `vector.normalize`, `vector.distance`, and
`vector.summary`. Successful responses preserve vector-specific fields and add
the shared `operation`, `title`, `message`, `summary`, and `result` fields.

Default surface calls are deterministic and in-memory. They reject more than
100,000 vector values and previews larger than 256 values with typed
`runtime_core::SurfaceError` JSON. Unsupported metric names also return typed
surface errors.

## Related crates

- `vector-analysis-index`
- `text-embeddings`
- `video-analysis-recognition`
