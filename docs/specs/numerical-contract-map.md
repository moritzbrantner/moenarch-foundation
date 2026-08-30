# Foundation numerical contract and ownership map

Status: authoritative current-state audit for
[F0](https://github.com/moritzbrantner/moenarch-foundation/issues/27). This
document records the source tree at baseline `25fd240` as inspected on
2026-08-30; it makes no API, serialization, package-version, or release change.

## Scope and ownership rule

This is the authoritative ownership map for reusable numerical primitives in
`numbers-core`, `tensor-data`, `vector`, and `math`. A primitive has one
library owner. CLI and server crates are transport adapters only: they expose
their wrapped library's package surface and do not own a second numerical
contract.

| Primitive family | Owner | Current public primitives | Precision and representation |
| --- | --- | --- | --- |
| Scalar contracts, summaries, ranges, exact decimals, and conversion | `moenarch-numbers-core` | `ApproxTolerance`, `is_finite_f32_slice`, `is_finite_f64_slice`, `checked_f64_to_f32`; `RunningStats`, `NumberSummary`, `NumberRange`, `HistogramConfig`, `Histogram`, `HistogramBin`, `QuartileSummary`, summary/quantile/histogram functions; `ExactDecimal`, `RoundingMode`, `DecimalError` | `f32`/`f64` contract helpers; `f64` analytical data; `u64` counts; `usize` bin counts; exact base-10 decimals with scale `0..=28` |
| Checked tensors | `moenarch-tensor-data` | `TensorShape`, `F32Tensor`, `F32TensorView` | contiguous `f32` values and non-zero `usize` dimensions |
| Dense vector metrics | `moenarch-vector-analysis-core` | `DenseVector`, `VectorMetric`, `VectorStats`, dot/norm/similarity/distance/summary functions | non-empty `f32` vectors |
| Exact in-memory vector retrieval | `moenarch-vector-analysis-index` | `VectorRecord`, metadata/filter/search configuration, results, `VectorSearchIndex`, centroid assignment | delegates vector arithmetic to `vector-analysis-core`; string and JSON metadata only |
| Dense matrices, decompositions, kernels, and tensor/vector bridges | `moenarch-math-linear` | `MatrixShape`, `MatrixLayout`, `F32Matrix`/views/row/column views, `F64Matrix`/views/row/column views, LU/QR/SVD/pseudoinverse diagnostics and options, `Kernel1d`, `Kernel2d` | finite row-major `f32` and `f64`; transpose views may be column-major without copying |
| 2D geometry and affine transforms | `moenarch-math-geometry-2d` | points, vectors, integer and float rectangles, bounds, normalized points, `Affine2`, segments, circles, polygons, and broad-phase grid types | validated `f32` coordinates; checked `u32` pixel geometry; `u64` areas where required |
| 3D coordinates, rotations, matrices, and transforms | `moenarch-math-geometry-3d` | `Vector3`/`Vector3d`, `Point3`/`Point3d`, `Quaternion`/`Quaterniond`, `UnitQuaternion`/`UnitQuaterniond`, `EulerOrder`, `Matrix3`/`Matrix3d`, `Matrix4`/`Matrix4d`, `RigidTransform3`/`RigidTransform3d`, `AffineTransform3`/`AffineTransform3d`, `Geometry3dError` | paired finite `f32` and `f64` contracts; row-major matrices acting on column vectors; radians for angles |
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
| `numbers-core` | Shared slice predicates accept only finite values. `ApproxTolerance::new` rejects non-finite limits and comparisons reject non-finite operands. Running summaries count `NaN`/infinite inputs but exclude them from numerical aggregates. Quantiles and histograms filter to finite values and require at least one. Range constructors and weights require finite values. `ExactDecimal` has no non-finite state and rejects non-finite float imports. | `ApproxTolerance::new` requires explicit finite, non-negative absolute and relative limits and comparisons accept either bound; it has no default. Its public fields can bypass construction checks. Degenerate scalar ranges are valid; normalization returns `0.0` and histogram values land in the final bin. Decimal rounding/division require an explicit target scale and `RoundingMode`. | `checked_f64_to_f32` returns `None` for non-finite or out-of-range input. Analytical invalid inputs use `media_core::DetectError::InvalidArgument`. Exact parsing, scale, overflow, division, float, and integer-conversion failures use `DecimalError`; arithmetic is checked. | Summary/range/histogram and tolerance types use derived Serde with their named public fields; deserialization does not call `new` or `validate`. `ExactDecimal` is the exception: it serializes as a JSON string and its custom deserializer parses and validates that string. `RoundingMode` uses named Serde variants. `rust_decimal` remains private. |
| `tensor-data` | Validated shapes are non-empty with non-zero extents; every value accepted by tensor/view constructors or `validate` must be finite. | No tolerance behavior. | Element counts use checked `usize` multiplication; reshape requires identical counts; failures use `DetectError`. | Tensor shape and owned tensor contracts use derived Serde; metadata is an uninterpreted `BTreeMap<String, serde_json::Value>`. Derived deserialization bypasses the constructors, so untrusted values require an explicit public `validate` call. No backend types are exposed. |
| `vector-analysis-core` | Dense vectors and all metric inputs are non-empty and finite. | Cosine similarity and normalization reject norms at or below `f32::EPSILON`; pairwise operations require equal dimensions. | Validation failures use `DetectError`. | Dense vectors and metric/summaries are Serde transfer types. No backend types are exposed. |
| `vector-analysis-index` | Records and queries accepted into an index are finite, non-empty dense vectors; indexed dimensions are fixed. | The selected `VectorMetric` defines ranking; zero-norm rejection comes from the vector core for cosine operations. | Insert/search/filter validation uses `DetectError`; `import_records` rebuilds each serialized vector through `DenseVector::new`. | Record, filter, and serialized-record transfer shapes use derived Serde, including metadata and a raw `Vec<f32>` in `SerializableVectorRecord`; deserialization alone does not validate that vector, while `import_records` does. No index backend type escapes. |
| `math-linear` | Matrix, vector, and kernel inputs and produced decomposition values must be finite. Matrix shapes are non-zero. | Several local policies exist: rank tolerance accepts a finite non-negative caller value or derives `f32::EPSILON * max(rows, cols) * max_column_l2_norm`; LU uses a pivot-derived threshold; QR/Cholesky and normalization reject effectively zero values. | Matrix/tensor element counts and shape bridges are checked; `F32Tensor` conversion is rank-2 only. Failures use `DetectError`. | Core matrix/decomposition structs are Rust library types, not Serde data models; JSON is confined to `surface`. `faer` and `nalgebra` are optional, feature-gated reference/benchmark paths and do not appear in public signatures. |
| `math-geometry-2d` | Float constructors and operation entry points validate the relevant point, vector, rectangle, normalized-point, transform, circle, and bounds coordinates. Integer operations check dimensions and coordinate arithmetic. | Geometric zero/parallel/invertible tests use `f32::EPSILON`; validated normalized points are bounded to `[0, 1]`. | `u32` coordinate/extent arithmetic and `u64` areas are checked. It owns `GeometryError` (including invalid dimensions) and converts it to `DetectError` at integration boundaries. | Geometry value types have named public fields and derived Serde. Direct construction or deserialization can bypass `new`/`validate`; checked operations validate their inputs. No numerical backend types are exposed. |
| `math-geometry-3d` | Validated coordinate, raw-quaternion, matrix, and transform constructors require finite components. Unit-quaternion constructors normalize input and reject zero magnitude. Checked operations reject non-finite results. | Vector/quaternion degeneracy, singular matrices, and homogeneous affine rows use precision-local epsilon checks. Proper f64 rotation-matrix import uses a documented `1e-10` validity bound; verification uses explicit `1e-10` f64 and `2e-5` f32 absolute-or-relative tolerances. | Widening from f32 to f64 is infallible. Every named f64-to-f32 path is explicitly checked through `checked_f64_to_f32`. Failures use `Geometry3dError` variants for non-finite, degenerate, singular, non-affine, and unrepresentable values. | Types serialize as named coordinate/quaternion fields or nested row-major matrix arrays; quaternion component order is `x, y, z, w`. Custom deserializers route coordinates, quaternions, matrices, and nested transforms through their validating constructors, including unit-quaternion normalization. Optional `nalgebra-adapters` expose named edge conversions only; foundation types remain canonical. |
| `math-sparse-data` | Sparse vector/COO/CSR values and dense operands must be finite; indices and offsets are checked. | Cosine/normalization rejects norms at or below `f32::EPSILON`; prune thresholds must be finite and non-negative. | Matrix dimensions and CSR offsets are checked; dense interop returns `math-linear` types. Failures use `DetectError`. | Core sparse types are Rust library types rather than Serde models; JSON is confined to `surface`. No backend types are exposed. |
| `math-statistics` | Scalar series and weights must be finite; multivariate data relies on finite matrix contracts. | Correlation, z-score, regression, and PCA use `f64::EPSILON` or `f32::EPSILON` checks appropriate to their input precision. OLS promotes f32 matrix work to f64 when needed and rejects a non-finite/out-of-range conversion back to f32. | Series dimensions, rolling windows, confidence, weights, and regularization are checked; failures use `DetectError`. | Core result types are Rust library types rather than Serde models; JSON is confined to `surface`. No backend types are exposed. |
| `math-signal-core` | Samples, coefficients, frequency, Q, dB, and scaling inputs are finite where applicable. Sample rates, window length, and frame stride are non-zero. | Zero/near-zero peak normalization rejects at `f32::EPSILON`; frequency is constrained by Nyquist. | Invalid sample rates use `DetectError::InvalidAudioFormat`; other invalid inputs use `DetectError::InvalidArgument`. | Core signal types are Rust library types rather than Serde models; JSON is confined to `surface`. No backend types are exposed. |

These are present contracts, not a claim that every crate shares one tolerance
or error type. `numbers-core` owns only the three proven shared seams described
in [Shared numerical contracts](numerical-contracts.md): finite-slice
predicates, checked `f64`-to-`f32` conversion, and explicit approximate
tolerance values. `f32::EPSILON`, `f64::EPSILON`, and algorithm-derived
thresholds remain selected locally. There is no universal scalar trait, global
epsilon, or implicit collapse of floating-point and exact-number semantics.
Constructor and operation guarantees are deliberately distinguished from wire
input above: for owners that still use derived `Deserialize`, the representation
contract alone is not proof that an invariant was checked.

## Dependency and target-layering map

```text
numeric contracts
  numbers-core: finite predicates, explicit tolerance, checked conversion,
                exact decimal mechanics, ranges, summaries, quantiles
                 |
                 +------------------------------+
                 |                              |
vectors/tensors/matrices/geometry          signal primitives
  tensor-data -> math-linear -> sparse-data     math-signal-core
  vector-analysis-core -> vector-index          |
  math-geometry-2d, math-geometry-3d             |
                 |                              |
                 +--------------+---------------+
                                |
statistics/algorithms
  math-statistics, search, decomposition, sparse algorithms
                                |
domain capabilities
  downstream repositories only
```

The target layering is exactly `numeric contracts ->
vectors/tensors/matrices/geometry -> statistics/algorithms -> domain
capabilities`. Signal primitives are a domain-neutral algorithm input alongside
the linear substrates; they do not create a dependency around that direction.
The diagram expresses ownership and dependency direction, not a mandate to
introduce another intermediate crate. Existing direct dependencies are retained
until an implementation slice proves a smaller shared seam.

## Duplicated, misplaced, and deferred concepts

| Finding | Present locations | Intended destination / follow-up |
| --- | --- | --- |
| Finite-value checks and effectively-zero decisions still appear with local error wording and thresholds. | `numbers-core` supplies shared finite-slice predicates used by vector core and linear; tensor, sparse, statistics, 2D/3D geometry, and signal retain local validation or algorithm thresholds. | Keep shared finite/tolerance/conversion mechanics in `numbers-core`, but preserve owner-specific errors and operation-specific thresholds such as linear rank/LU pivot and geometry invertibility bounds. Adopt another shared seam only when multiple production callers prove it. |
| Normalization is represented by scalar range normalization, dense/sparse L2 normalization, quaternion normalization, and z-score/min-max normalizers. | `numbers-core`, vector core, sparse data, 3D geometry, statistics. | Keep each semantic operation with its current owner. Share validation mechanics, not a generic normalization hierarchy. |
| Dense vectors and matrices are intentionally separate but overlap on dot/norm/row operations. | Vector core and linear. | `vector-analysis-core` remains vector owner; `math-linear` remains matrix/decomposition owner. Reference/law coverage can span both without moving APIs. |
| Reusable 3D value and transform math was duplicated in the legacy spatial owner. | Foundation now owns `math-geometry-3d`; related source remains in `rust-packages/crates/three-d/three-d-processing-core/src/{geometry,math,spatial_math,transform}.rs` as compatibility/provenance material. | `math-geometry-3d` is the canonical owner for domain-neutral points, vectors, quaternions, Euler order, matrices, rigid transforms, and affine transforms. Physical presence of the legacy copy does not confer source or release authority. A later migration may redirect legacy callers. Meshes, point clouds, bounds, camera poses, rays, planes, collision, scene SVG, and reconstruction stay in spatial/domain owners. Similarity/TRS conveniences require a separate demonstrated foundation API need. |
| Dense 3D vectors overlap conceptually with general dense-vector arithmetic. | `math-geometry-3d` owns typed three-component displacement semantics; `vector-analysis-core` owns variable-length analytical vectors. | Retain both owners. Point/vector distinction and rotation/transform composition stay in geometry; retrieval and metric operations stay in vector analysis. |
| Financial/risk-labelled functions use binary floating point. | `math-statistics`: relative/log returns, compounded return, drawdown, and tail risk use `f64`; `numbers-core` scalar totals/weighted means also use `f64`. | Keep numerical analytics floating-point. Consumers requiring monetary or contractual exactness must hold amounts as `ExactDecimal` and cross to lossy analysis explicitly; finance-domain adoption belongs above foundation. |
| Exact decimal mechanics exist, but finance semantics and other exact-number families do not. | `numbers-core::ExactDecimal` owns fixed-scale base-10 parsing, checked arithmetic, conversion, rounding, and string serialization. There is no rational, currency, money, basis-point, day-count, ledger, tax, or payment contract. | Keep domain-neutral decimal mechanics in `numbers-core`. Add another exact-number representation only for a proven domain-neutral need. Currency and finance policy remain in capability repositories. |
| Validated constructors and derived deserialization are inconsistent for several transfer types. | `numbers-core` range/config/tolerance values, `tensor-data`, and 2D geometry still derive `Deserialize`; direct public fields also bypass constructors in several of those owners. The 3D geometry owner now validates every invariant-bearing deserialization path. | Each remaining owner is responsible for any later validating-deserializer or private-field compatibility decision. Until a behavior-authorizing slice changes it, callers of those remaining derived surfaces must use constructors or explicit validation after untrusted deserialization. |

## Durable ownership decisions

1. Preserve the distinct semantics of `f32`, `f64`, integer counts, exact
   values, and decimals. No `Number` or `FoundationScalar` super-trait is
   introduced.
2. Treat optional `nalgebra` and `faer` dependencies as edge adapters and
   reference paths only. Public APIs remain foundation-owned concrete types.
3. Keep only reusable 3D mathematical values and transforms in foundation.
   Domain geometry and spatial pipelines are excluded.
4. Give exact-decimal quantities explicit scale, conversion, overflow, and
   rounding contracts; do not use binary float for finance-grade amounts.
5. Demonstrate numerical behavior with laws, independent reference agreement,
   adversarial inputs, and cross-crate canaries rather than treating unit
   examples or line coverage as sufficient evidence.

## Audit evidence

- `cargo metadata --format-version 1 --no-deps` identified the in-scope library
  and adapter package graph at baseline `25fd240`.
- The public library sources, manifests, READMEs, runtime surfaces, package
  ownership records, and the existing legacy 3D ownership finding were
  inspected. The focused contract notes for shared numerics, linear algebra,
  3D geometry, and exact decimals resolve details summarized here.
- The repository remains source-first: this audit does not alter crate versions,
  release plan, package ownership, manifests, dependency graph, or serialized
  public behavior.
