# math-geometry-3d

Domain-neutral, finite 3D points, vectors, rotations, and affine transforms.

## Conventions

- The crate uses a right-handed coordinate system. Axes have no implicit real-world
  unit; callers must establish units at their application boundary.
- `Matrix3*` and `Matrix4*` store rows in row-major serialization order and act on
  column vectors. A transform applies as `M * v`.
- `RigidTransform3*::compose` and `AffineTransform3*::compose` return
  `self after rhs`: applying the result to a value is the same as applying `rhs`
  first and then `self`.
- Quaternions serialize as `x`, `y`, `z`, `w`. `UnitQuaternion*` is the canonical
  rotation type and normalizes input; raw `Quaternion*` is only an interchange
  representation. Euler angles are radians and an explicit `EulerOrder` is
  required at the import/export boundary.
- Every constructor rejects non-finite values. `*d` types are f64; unsuffixed
  types are f32. Conversion from f64 to f32 is checked rather than lossy.

The optional `nalgebra-adapters` feature adds conversions at the crate edge; no
nalgebra type is part of this crate's canonical contract.

## Example

```rust
use math_geometry_3d::{Point3d, RigidTransform3d, UnitQuaterniond, Vector3d};

let rotation = UnitQuaterniond::from_axis_angle(Vector3d::Y, std::f64::consts::FRAC_PI_2)?;
let transform = RigidTransform3d::new(rotation, Vector3d::new(1.0, 0.0, 0.0)?)?;
let point = transform.apply_point(Point3d::ORIGIN)?;
assert!((point.x() - 1.0).abs() < 1.0e-12);
# Ok::<(), math_geometry_3d::Geometry3dError>(())
```
