# 3D geometry contract

`moenarch-math-geometry-3d` owns reusable, domain-neutral 3D coordinate and
transform mathematics. It is intentionally independent of the legacy spatial
processing package and does not define cameras, rays, meshes, scene graphs,
reconstruction, SFM/MVS, Gaussian splatting, or application pose pipelines.

## Interface

- `Point3`/`Point3d` are positions; `Vector3`/`Vector3d` are displacements.
  Their separate types make translating a point and transforming a vector
  distinct operations.
- Every public constructor validates finiteness. `UnitQuaternion` and
  `UnitQuaterniond` additionally normalize raw quaternion input and reject a
  zero magnitude.
- Unsuffixed types use f32 and `*d` types use f64. Widening is lossless;
  narrowing, including raw quaternions, uses explicit `to_f32_checked` methods
  and rejects overflow, infinity, and NaN.
- Raw quaternions are finite interchange values. Unit quaternions are the
  primary rotation representation. Axis-angle and `EulerOrder` conversions are
  import/export or UI seams, and all Euler angles are radians.
- `RigidTransform3*` is rotation plus translation. `AffineTransform3*` permits
  a general finite 3×3 linear term plus translation, so it can represent scale
  and shear without admitting perspective.

## Conventions

The coordinate basis is right-handed and has no implied physical unit.
Matrices store rows in row-major order, serialize in that same nested-row order,
and act on column vectors. A homogeneous affine matrix has bottom row
`[0, 0, 0, 1]`; translation occupies the final column.

`compose` has one meaning across rotations, matrices, rigid transforms, and
affine transforms: `left.compose(right)` applies `right` first and `left`
second. Quaternion serialized component order is `x`, `y`, `z`, `w`.

The `nalgebra-adapters` feature exposes only edge conversion methods. The
crate’s own types remain the public contract, so consumers can use nalgebra
without making it a foundation-wide backend choice.

## Verification tolerances

The law suite uses deterministic generated cases. f64 rotation, matrix, and
transform identities use an absolute-or-relative tolerance of `1e-10`; f32
canaries use `2e-5` to account for the deliberate narrow representation.
SLERP evidence covers endpoints, midpoint and shortest-path behavior,
unit-length preservation, and invalid interpolation factors in both precision
families.
Matrix import accepts a proper f64 rotation matrix only when each axis norm,
axis dot product, and determinant differ from the ideal by no more than
`1e-10`. These are verification and input-validity contracts, not a hidden
global tolerance for callers' application algorithms.

Raw-quaternion deserialization validates finite components. Unit-quaternion
deserialization additionally normalizes the wire components and rejects a zero
magnitude, so Serde cannot bypass the rotation invariant used by nested rigid
transforms.

## Compatibility and release boundary

The historical counterpart remains in `rust-packages` for compatibility and
provenance; F4 created the canonical foundation owner without deleting or
redirecting that legacy source. Compatibility facades or migration removal
require separate authorization. Physical presence of the legacy source does
not confer source or release authority. The package is recorded as one approved
post-extraction addition: the original 60-record source-inventory digest and
the single addition digest are validated separately by the boundary checker.
