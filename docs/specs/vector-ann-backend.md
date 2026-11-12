# Vector ANN backend: exact vs cosine LSH vs external HNSW (#119)

Reuse-first evaluation (PRINCIPLE-007) of an established approximate-nearest-neighbour
index against Foundation's own `CosineLshIndex`, with the exact
`VectorSearchIndex` as the recall baseline.

## Paths compared

| Path | Type | Candidate selection | Ranking |
| --- | --- | --- | --- |
| exact | `VectorSearchIndex` | every record | exact cosine, `(distance, id)` |
| LSH | `approximate::CosineLshIndex` | seeded hyperplane buckets, Hamming probes, hard cap `C` | exact cosine, `(distance, id)` |
| HNSW | `hnsw::CosineHnswIndex` (`hnsw` feature, `hnsw_rs` 0.3.5) | HNSW graph, `ef_search` candidates | exact cosine, `(distance, id)` |

All three return `SearchResult`; `hnsw_rs` types stay private to the adapter.

## Workloads

Both harnesses are deterministic and repeatable; recall/ordering numbers are exact,
wall-clock and heap numbers are indicative only (BENCH-003). Single thread, release
build, x86_64 Linux (WSL2, Intel Core Ultra 7 258V, 8 cores, shared machine with
load average 5-11 during the runs).

1. **Foundation synthetic** — `cargo run --release -p moenarch-vector-analysis-index --features hnsw --example ann_three_way`:
   64 Gaussian clusters, D=128, σ=0.6 around unit-variance centres, seed `0x1190a11e`,
   200 held-out queries, cosine top-10. Input FNV-1a64 hashes: N=2,048
   `0x22fab4e809d6848b`, N=8,192 `0x93ff68f1bc7aaa04`, N=32,768 `0x4b17759c0b859950`.
2. **Consumer (nlp-stack `text-retrieval`)** — `cargo run --release -p moenarch-text-retrieval --features hnsw --example ann_three_way`:
   `RetrievalIndex` chunk vectors from the deterministic hashed-fixture embedder
   (D=128, 48 synthetic topics, one chunk per document, seed `0x1190c0de`),
   200 indexed-chunk queries, related-chunk top-10.

`order` is the fraction of queries whose full top-10 (ids, scores, order) is
identical to exact. `scored` is the mean number of vectors scored exactly per query.
HNSW uses `M=16, efConstruction=200`.

### Foundation synthetic

| N | path | recall@10 | order | build | query | scored | build heap |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 2,048 | exact | 1.000 | 1.000 | 2 ms | 1.27 ms | 2,048 | 1.5 MiB |
| 2,048 | LSH 8 bits r=2 C=2048 | 0.739 | 0.210 | 3 ms | 0.21 ms | 317 | 1.5 MiB |
| 2,048 | HNSW ef=64 | 1.000 | 1.000 | 0.90 s | 0.26 ms | 64 | 5.7 MiB |
| 8,192 | exact | 1.000 | 1.000 | 2 ms | 3.96 ms | 8,192 | 6.0 MiB |
| 8,192 | LSH 8 bits r=2 C=2048 | 0.765 | 0.270 | 9 ms | 0.74 ms | 1,250 | 6.0 MiB |
| 8,192 | HNSW ef=64 | 1.000 | 1.000 | 7.0 s | 0.53 ms | 64 | 22.7 MiB |
| 32,768 | exact | 1.000 | 1.000 | 13 ms | 18.5 ms | 32,768 | 23.9 MiB |
| 32,768 | LSH 12 bits r=2 C=1024 | 0.511 | 0.065 | 85 ms | 0.74 ms | 811 | 24.2 MiB |
| 32,768 | LSH 8 bits r=2 C=2048 | 0.548 | 0.070 | 55 ms | 1.78 ms | 2,048 | 24.1 MiB |
| 32,768 | HNSW ef=32 | 0.977 | 0.820 | 29 s | 0.43 ms | 32 | 89.6 MiB |
| 32,768 | HNSW ef=64 | 0.997 | 0.965 | 29 s | 0.54 ms | 64 | 89.6 MiB |
| 32,768 | HNSW ef=128 | 0.9995 | 0.995 | 29 s | 0.74 ms | 128 | 89.6 MiB |

The default LSH configuration (16 bits, r=1, C=256) reached recall 0.10-0.12 at every scale.
Build heap is the peak heap growth while building, including the cloned records (≈1.5 MiB
per 2k records); the HNSW graph adds ≈2 KiB per vector at M=16, including `hnsw_rs`'s own copy of each vector.

### Consumer (hashed-fixture retrieval chunks)

| N | exact query | best LSH recall@10 | HNSW ef=64 recall@10 | HNSW ef=64 query | HNSW build |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1,024 | 0.62 ms | 0.469 | 1.000 | 0.24 ms | 0.35 s |
| 4,096 | 2.63 ms | 0.507 | 1.000 | 0.64 ms | 5.9 s |
| 16,384 | 11.2 ms | 0.310 | 0.9995 | 0.97 ms | 30 s |

### Ordering and ties

All paths re-score candidates exactly and order by `(distance, id)`. HNSW output is
identical to exact whenever its candidate set covers the exact top-K (100% of queries up
to 8k synthetic / 4k consumer at ef=64, 96.5% at 32k synthetic). LSH rarely covers it on
these workloads.

### Determinism

`CosineHnswIndex` inserts serially in record order and `hnsw_rs` draws levels from a fixed
internal seed (397), so rebuilds reproduce identical results on the same build and target:
asserted for every query and `ef` by the synthetic harness and by unit tests. Not claimed
across targets/SIMD paths. `hnsw_rs::parallel_insert` is not exposed because its graph
depends on thread scheduling.

## Decision

- **External HNSW (`hnsw_rs`) is the preferred ANN candidate** for unfiltered cosine top-K
  over long-lived, query-heavy corpora of a few thousand vectors and more. It is wired into
  one real consumer: nlp-stack `text-retrieval` behind its `hnsw` feature, for unfiltered
  semantic hits over ≥4,096 chunks.
- **Build cost is the main trade-off**: serial insertion takes seconds at 4-8k vectors and
  ~30 s at 16-32k on this machine, so a graph amortizes after roughly 900-3,000 queries.
  Short-lived or frequently rebuilt indexes should stay exact.
- **Memory**: about 3.7× the exact index's heap at M=16.
- **Deployment**: optional `hnsw` feature only. `hnsw_rs` pulls `rayon`, `mmap-rs`,
  `parking_lot`, `env_logger`, `cpu-time`, `num_cpus`, `hashbrown`, `indexmap`, `rand`
  and `anndists`; it is pure Rust (no C/C++ toolchain) and builds on native Linux. WASM was
  not attempted (`mmap-rs`, `rayon`, `cpu-time` make it unlikely); the WASM bindings do not
  enable the feature. `hnsw_rs` calls `process::exit` for `max_connections > 256`, which the
  adapter validates first.
- **LSH is retained unchanged** for its dependency-free, WASM-portable niche with an explicit
  scoring bound. It did not win a measured workload on recall (≤0.77 at any tested setting)
  and has no consumer today; removing it is a separate decision, not part of #119.
- **`usearch` not adopted**: it is a C++ library behind a `cxx` build. It needs a C++
  toolchain on every build host, uses native SIMD dispatch, has no WASM path, and would add
  native build and portability cost to CI and consumers. `hnsw_rs` already met the recall
  and latency need, so `usearch` stays a recorded native-deployment option, unmeasured.
- **Fallback**: the exact `VectorSearchIndex` remains the reference and the source of truth.
  Filtered queries, small corpora and builds without the feature use it. Disabling the
  feature restores exact behaviour with no data migration.

## Not verified

Real embedding models (Candle/ONNX) were not measured; the consumer workload uses the
hashed-fixture embedder. Multi-threaded query concurrency, cross-target determinism and
WASM builds were not measured.
