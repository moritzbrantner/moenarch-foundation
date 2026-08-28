# Foundation numerical contract and ownership map

Status: current-state audit for [F0](https://github.com/moritzbrantner/moenarch-foundation/issues/27). This document records the source tree as inspected on 2026-08-28; it makes no API, serialization, package-version, or release change.

## Scope and ownership rule

This is the authoritative ownership map for reusable numerical primitives in
`numbers-core`, `tensor-data`, `vector`, and `math`. A primitive has one
library owner. CLI and server crates are transport adapters only: they expose
their wrapped library's package surface and do not own a second numerical
contract.

| Primitive family | Owner | Current public primitives | Precision and representation |
| --- | --- | --- | --- |
| Scalar summaries, ranges, quantiles, histograms, and normalization | `moenarch-numbers-core` | `RunningStats`, `NumberSummary`, `NumberRange`, `HistogramConfig`, `Histogram`, `HistogramBin`, `QuartileSummary`, `summarize_numbers`, `quantile`, `quartiles`, `histogram` | `f64` scalar data; `u64` counts; `usize` bin counts |
| Checked tensors | `moenarch-tensor-data` | `TensorShape`, `F32Tensor`, `F32TensorView` | contiguous `f32` values and non-zero `usize` dimensions |
| Dense vector metrics | `moenarch-vector-analysis-core` | `DenseVector`, `VectorMetric`, `VectorStats`, dot/norm/similarity/distance/summary functions | non-empty `f32` vectors |
| Exact in-memory vector retrieval | `moenarch-vector-analysis-index` | `VectorRecord`, metadata/filter/search configuration, results, `VectorSearchIndex`, centroid assignment | delegates vector arithmetic to `vector-analysis-core`; string and JSON metadata only |
| Dense matrices, decompositions, kernels, and tensor/vector bridges | `moenarch-math-linear` | `MatrixShape`, `MatrixLayout`, `F32Matrix`/views/row/column views, `F64Matrix`/views/row/column views, LU/QR/SVD/pseudoinverse diagnostics and options, `Kernel1d`, `Kernel2d` | finite row-major `f32` and `f64`; transpose views may be column-major without copying |
| 2D geometry and affine transforms | `moenarch-math-geometry-2d` | points, vectors, integer and float rectangles, bounds, normalized points, `Affine2`, segments, circles, polygons, and broad-phase grid types | validated `f32` coordinates; checked `u32` pixel geometry; `u64` areas where required |
| Sparse vectors and matrices | `moenarch-math-sparse-data` | `SparseVector`, `CooMatrix`, `CsrMatrix`, `SparseRow`, `SparseMatrixSummary`, sparse similarity | finite `f32` values with `usize` dimensions and indices; dense interop uses `math-linear` |
| Descriptive, streaming, multivariate, regression, and risk statistics | `moenarch-math-statistics` | variance/change/rank modes; series/pairwise/risk/regression summaries; `WeightedObservation`, `RunningCovariance`, normalizers, PCA; scalar, rolling, and regression functions | scalar statistics use finite `f64`; matrix and multivariate workflows use finite `f32` matrices |
| Signal-domain numerical operations | `moenarch-math-signal-core` | sample rates, resampling/interpolation, windows, frames, biquads, FIR kernels, level, dB, and FFT-bin helpers | integer sample-rate identity; finite `f32` sample/coefficient data; `f64` source positions and resampling ratios |

The in-scope adapter crates are deliberately not owners:

- `*-cli` in `crates/math/**` and `crates/vector/**` forwards JSON package
  operations to its wrapped library.
- `*-server` in those directories forwards the same package surface over HTTP.

Their wrapped libraries remain the sole owners until a separate usage and
semver decision permits adapter removal.

## Current numerical contracts

| Owner | Finite/non-finite policy | Tolerance and degeneracy policy | Checked conversion and errors | Serialization and backend exposure |
| --- | --- | --- | --- | --- |
| `numbers-core` | Running summaries count `NaN`/infinite inputs but exclude them from numerical aggregates. Quantiles and histograms filter to finite values and require at least one. Ranges and weights must be finite. | No approximate-equality API. Degenerate ranges are valid; normalization returns `0.0` and histogram values land in the final bin. | Invalid inputs return `media_core::DetectError::InvalidArgument`. | Summary/range/histogram transfer types derive Serde. No backend types are exposed. |
| `tensor-data` | Shapes are non-empty with non-zero extents; every stored or viewed `f32` value must be finite. | No tolerance behavior. | Element counts use checked `usize` multiplication; reshape requires identical counts; failures use `DetectError`. | Tensor shape and owned/view tensor contracts derive Serde; metadata is an uninterpreted `BTreeMap<String, serde_json::Value>`. No backend types are exposed. |
| `vector-analysis-core` | Dense vectors and all metric inputs are non-empty and finite. | Cosine similarity and normalization reject norms at or below `f32::EPSILON`; pairwise operations require equal dimensions. | Validation failures use `DetectError`. | Dense vectors and metric/summaries are Serde transfer types. No backend types are exposed. |
| `vector-analysis-index` | Records and queries are finite, non-empty dense vectors; indexed dimensions are fixed. | The selected `VectorMetric` defines ranking; zero-norm rejection comes from the vector core for cosine operations. | Insert/search/filter validation uses `DetectError`. | Record, filter, config, and result shapes are Serde contracts, including JSON metadata. No index backend type escapes. |
| `math-linear` | Matrix, vector, and kernel inputs and produced decomposition values must be finite. Matrix shapes are non-zero. | Several local policies exist: rank tolerance accepts a finite non-negative caller value or derives `f32::EPSILON * max(rows, cols) * max_column_l2_norm`; LU uses a pivot-derived threshold; QR/Cholesky and normalization reject effectively zero values. | Matrix/tensor element counts and shape bridges are checked; `F32Tensor` conversion is rank-2 only. Failures use `DetectError`. | Core matrix/decomposition structs are Rust library types, not Serde data models; JSON is confined to `surface`. `faer` and `nalgebra` are optional, feature-gated reference/benchmark paths and do not appear in public signatures. |
| `math-geometry-2d` | Float points, vectors, rectangles, normalized points, transforms, circles, and bounds require finite coordinates. Integer geometry checks dimensions and coordinate arithmetic. | Geometric zero/parallel/invertible tests use `f32::EPSILON`; normalized points are bounded to `[0, 1]`. | `u32` coordinate/extent arithmetic and `u64` areas are checked. It owns `GeometryError` (including invalid dimensions) and converts it to `DetectError` at integration boundaries. | Geometry value types derive Serde, preserving named fields and enum variants. No numerical backend types are exposed. |
| `math-sparse-data` | Sparse vector/COO/CSR values and dense operands must be finite; indices and offsets are checked. | Cosine/normalization rejects norms at or below `f32::EPSILON`; prune thresholds must be finite and non-negative. | Matrix dimensions and CSR offsets are checked; dense interop returns `math-linear` types. Failures use `DetectError`. | Core sparse types are Rust library types rather than Serde models; JSON is confined to `surface`. No backend types are exposed. |
| `math-statistics` | Scalar series and weights must be finite; multivariate data relies on finite matrix contracts. | Correlation, z-score, regression, and PCA use `f64::EPSILON` or `f32::EPSILON` checks appropriate to their input precision. OLS promotes f32 matrix work to f64 when needed and rejects a non-finite/out-of-range conversion back to f32. | Series dimensions, rolling windows, confidence, weights, and regularization are checked; failures use `DetectError`. | Core result types are Rust library types rather than Serde models; JSON is confined to `surface`. No backend types are exposed. |
| `math-signal-core` | Samples, coefficients, frequency, Q, dB, and scaling inputs are finite where applicable. Sample rates, window length, and frame stride are non-zero. | Zero/near-zero peak normalization rejects at `f32::EPSILON`; frequency is constrained by Nyquist. | Invalid sample rates use `DetectError::InvalidAudioFormat`; other invalid inputs use `DetectError::InvalidArgument`. | Core signal types are Rust library types rather than Serde models; JSON is confined to `surface`. No backend types are exposed. |

These are present contracts, not a claim that every crate already shares one
tolerance or error type. In particular, `f32::EPSILON`, `f64::EPSILON`, and
algorithm-derived thresholds are presently selected locally. F2 must make only
the policies demonstrably shared by multiple owners explicit; it must not add a
universal scalar trait or retroactively collapse precision distinctions.

## Dependency and target-layering map

```text
numeric contracts
  numbers-core: scalar validity, ranges, summaries, quantiles
  (F2: only proven common finite/tolerance/conversion contracts)
                 |
                 +------------------------------+
                 |                              |
vectors/tensors/matrices/geometry          signal primitives
  tensor-data -> math-linear -> sparse-data     math-signal-core
  vector-analysis-core -> vector-index          |
  math-geometry-2d                              |
  (F4: extracted neutral 3D primitives)         |
                 |                              |
                 +--------------+---------------+
                                |
statistics/algorithms
  math-statistics, search, decomposition, sparse algorithms
                                |
domain capabilities
  downstream repositories only
```

The diagram expresses the intended layering, not a mandate to introduce an
intermediate crate. Existing direct dependencies are retained until an
implementation slice proves a smaller shared seam.

## Duplicated, misplaced, and deferred concepts

| Finding | Present locations | Intended destination / follow-up |
| --- | --- | --- |
| Finite-value and effectively-zero checks are repeated with local wording and thresholds. | Tensor, vector, linear, sparse, statistics, geometry, and signal owners. | F2 evaluates a deliberately small shared contract layer. Preserve operation-specific threshold derivation such as linear algebra rank and LU pivot tolerances. |
| Normalization is represented by scalar range normalization, dense/sparse L2 normalization, and z-score/min-max normalizers. | `numbers-core`, vector core, sparse data, statistics. | Keep semantic owners separate; F2 may share only validation or tolerance primitives supported by more than one caller. |
| Dense vectors and matrices are intentionally separate but overlap on dot/norm/row operations. | Vector core and linear. | `vector-analysis-core` remains vector owner; `math-linear` remains matrix/decomposition owner. F3 can add reference/law coverage without moving APIs. |
| Reusable 3D value and transform math remains outside foundation. | `rust-packages/crates/three-d/three-d-processing-core/src/{geometry,math,spatial_math,transform}.rs`. | F4 should extract the domain-neutral subset: `Vector3`/`Vector3d`, `Point3`/`Point3d`, `Quaternion`/`Quaterniond`, Euler order, `Matrix3`/`Matrix4`, rigid/similarity/TRS/affine transforms, and their checked f32/f64 conversions. Meshes, point clouds, bounds, camera poses, rays, planes, collision, scene SVG, and reconstruction stay in spatial/domain owners. |
| Rotation convention is encoded by the existing external 3D implementation, not yet documented at the foundation boundary. | `three-d-processing-core` uses quaternion fields ordered `x, y, z, w` and explicit Euler orders. | F4 defines the foundation API; F5 records composition, handedness, vector application, Euler, matrix-layout, and serialization conventions and tests their laws. |
| Financial/risk-labelled functions use binary floating point. | `math-statistics`: relative/log returns, compounded return, drawdown, and tail risk use `f64`; `numbers-core` scalar totals/weighted means also use `f64`. | `numbers-core::ExactDecimal` supplies decimal mechanics for monetary-style inputs. Existing analytics remain floating-point and require an explicit consumer conversion boundary. |
| Exact decimal mechanics lacked a foundation owner. | `numbers-core::ExactDecimal` now provides fixed-scale decimal values, named rounding, checked arithmetic, and stable string serialization. | Currency, ledger, tax, and payment domain rules remain above foundation. |

## Decisions for the next slices

1. Preserve the distinct semantics of `f32`, `f64`, integer counts, exact
   values, and decimals. No `Number` or `FoundationScalar` super-trait is
   introduced.
2. Treat optional `nalgebra` and `faer` dependencies as implementation
   adapters and reference paths only. Public APIs remain foundation-owned
   concrete types.
3. Extract only reusable 3D mathematical values and transforms in F4. Domain
   geometry and spatial pipelines are excluded.
4. Give exact/decimal quantities explicit scale, conversion, overflow, and
   rounding contracts in F6; do not use binary float for finance-grade amounts.
5. Use F1/F3/F5 verification to demonstrate laws, independent reference
   agreement, adversarial-input behavior, and cross-crate canaries rather than
   treating current unit tests or line coverage as sufficient evidence.

## Audit evidence

- `cargo metadata --format-version 1 --no-deps` identified the in-scope library
  and adapter package graph.
- The public library sources, manifests, READMEs, runtime surfaces, and the
  adjacent source-owned `three-d-processing-core` were inspected.
- The repository remains source-first: this audit does not alter crate versions,
  release plan, package ownership, manifests, dependency graph, or serialized
  public behavior.
