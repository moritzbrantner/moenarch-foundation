#![doc = include_str!("../README.md")]

//! The public seam is intentionally limited to finite coordinate values,
//! normalized rotations, and affine transforms. Scene, camera, mesh, and pose
//! semantics belong to capability owners rather than this module.

use numbers_core::checked_f64_to_f32;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when an input cannot satisfy a finite 3D math invariant.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Geometry3dError {
    /// A supplied scalar was NaN or infinite.
    #[error("{0} must be finite")]
    NonFinite(&'static str),
    /// A vector or quaternion had no direction.
    #[error("{0} must have non-zero magnitude")]
    Degenerate(&'static str),
    /// A matrix cannot be inverted.
    #[error("matrix is singular")]
    SingularMatrix,
    /// A homogeneous matrix was not affine.
    #[error("affine matrix must have bottom row [0, 0, 0, 1]")]
    NonAffineMatrix,
    /// A double-precision value does not fit in the paired f32 contract.
    #[error("{0} is not representable as finite f32")]
    NotRepresentableAsF32(&'static str),
}

/// Result type for this crate.
pub type Result<T> = std::result::Result<T, Geometry3dError>;

fn finite64(value: f64, name: &'static str) -> Result<f64> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(Geometry3dError::NonFinite(name))
}

fn finite32(value: f32, name: &'static str) -> Result<f32> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(Geometry3dError::NonFinite(name))
}

fn as_f32(value: f64, name: &'static str) -> Result<f32> {
    checked_f64_to_f32(value).ok_or(Geometry3dError::NotRepresentableAsF32(name))
}

/// Explicit Euler rotation order. Angles always describe rotations around the
/// named world axes, and are composed in the written order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EulerOrder {
    /// X, then Y, then Z.
    Xyz,
    /// X, then Z, then Y.
    Xzy,
    /// Y, then X, then Z.
    Yxz,
    /// Y, then Z, then X.
    Yzx,
    /// Z, then X, then Y.
    Zxy,
    /// Z, then Y, then X.
    Zyx,
}

macro_rules! define_vector_point {
    ($vector:ident, $point:ident, $scalar:ty, $finite:ident, $zero:expr, $one:expr) => {
        /// A finite displacement in three-dimensional space.
        #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
        pub struct $vector {
            x: $scalar,
            y: $scalar,
            z: $scalar,
        }

        impl $vector {
            /// The zero displacement.
            pub const ZERO: Self = Self {
                x: $zero,
                y: $zero,
                z: $zero,
            };
            /// Unit x axis.
            pub const X: Self = Self {
                x: $one,
                y: $zero,
                z: $zero,
            };
            /// Unit y axis.
            pub const Y: Self = Self {
                x: $zero,
                y: $one,
                z: $zero,
            };
            /// Unit z axis.
            pub const Z: Self = Self {
                x: $zero,
                y: $zero,
                z: $one,
            };

            /// Creates a finite vector.
            pub fn new(x: $scalar, y: $scalar, z: $scalar) -> Result<Self> {
                Ok(Self {
                    x: $finite(x, "vector x")?,
                    y: $finite(y, "vector y")?,
                    z: $finite(z, "vector z")?,
                })
            }
            /// x component.
            pub const fn x(self) -> $scalar {
                self.x
            }
            /// y component.
            pub const fn y(self) -> $scalar {
                self.y
            }
            /// z component.
            pub const fn z(self) -> $scalar {
                self.z
            }
            /// Components in x, y, z order.
            pub const fn components(self) -> [$scalar; 3] {
                [self.x, self.y, self.z]
            }
            /// Dot product, rejecting overflow into a non-finite result.
            pub fn dot(self, rhs: Self) -> Result<$scalar> {
                $finite(
                    self.x.mul_add(rhs.x, self.y.mul_add(rhs.y, self.z * rhs.z)),
                    "dot product",
                )
            }
            /// Cross product.
            pub fn cross(self, rhs: Self) -> Result<Self> {
                Self::new(
                    self.y.mul_add(rhs.z, -(self.z * rhs.y)),
                    self.z.mul_add(rhs.x, -(self.x * rhs.z)),
                    self.x.mul_add(rhs.y, -(self.y * rhs.x)),
                )
            }
            /// Euclidean magnitude.
            pub fn magnitude(self) -> Result<$scalar> {
                self.dot(self)
                    .and_then(|value| $finite(value.sqrt(), "vector magnitude"))
            }
            /// Unit-length version of this vector.
            pub fn normalized(self) -> Result<Self> {
                let magnitude = self.magnitude()?;
                if magnitude <= <$scalar>::EPSILON {
                    return Err(Geometry3dError::Degenerate("vector"));
                }
                Self::new(self.x / magnitude, self.y / magnitude, self.z / magnitude)
            }
            /// Adds another displacement.
            pub fn plus(self, rhs: Self) -> Result<Self> {
                Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
            }
            /// Subtracts another displacement.
            pub fn subtract(self, rhs: Self) -> Result<Self> {
                Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
            }
            /// Scales this displacement.
            pub fn scale(self, scalar: $scalar) -> Result<Self> {
                $finite(scalar, "scale")?;
                Self::new(self.x * scalar, self.y * scalar, self.z * scalar)
            }
        }

        /// A finite position in three-dimensional space. Points and vectors are
        /// deliberately distinct so translations cannot be applied by accident.
        #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
        pub struct $point {
            x: $scalar,
            y: $scalar,
            z: $scalar,
        }

        impl $point {
            /// The origin.
            pub const ORIGIN: Self = Self {
                x: $zero,
                y: $zero,
                z: $zero,
            };
            /// Creates a finite point.
            pub fn new(x: $scalar, y: $scalar, z: $scalar) -> Result<Self> {
                Ok(Self {
                    x: $finite(x, "point x")?,
                    y: $finite(y, "point y")?,
                    z: $finite(z, "point z")?,
                })
            }
            /// x coordinate.
            pub const fn x(self) -> $scalar {
                self.x
            }
            /// y coordinate.
            pub const fn y(self) -> $scalar {
                self.y
            }
            /// z coordinate.
            pub const fn z(self) -> $scalar {
                self.z
            }
            /// Coordinates in x, y, z order.
            pub const fn coordinates(self) -> [$scalar; 3] {
                [self.x, self.y, self.z]
            }
            /// Translates this point by a vector.
            pub fn translate(self, vector: $vector) -> Result<Self> {
                Self::new(self.x + vector.x, self.y + vector.y, self.z + vector.z)
            }
            /// Returns the displacement from `rhs` to this point.
            pub fn subtract(self, rhs: Self) -> Result<$vector> {
                $vector::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
            }
        }
    };
}

define_vector_point!(Vector3d, Point3d, f64, finite64, 0.0, 1.0);
define_vector_point!(Vector3, Point3, f32, finite32, 0.0, 1.0);

impl Vector3d {
    /// Converts to f32 only when every component remains finite and representable.
    pub fn to_f32_checked(self) -> Result<Vector3> {
        Vector3::new(
            as_f32(self.x, "vector x")?,
            as_f32(self.y, "vector y")?,
            as_f32(self.z, "vector z")?,
        )
    }
}
impl Point3d {
    /// Converts to f32 only when every coordinate remains finite and representable.
    pub fn to_f32_checked(self) -> Result<Point3> {
        Point3::new(
            as_f32(self.x, "point x")?,
            as_f32(self.y, "point y")?,
            as_f32(self.z, "point z")?,
        )
    }
}
impl From<Vector3> for Vector3d {
    fn from(value: Vector3) -> Self {
        Self {
            x: value.x as f64,
            y: value.y as f64,
            z: value.z as f64,
        }
    }
}
impl From<Point3> for Point3d {
    fn from(value: Point3) -> Self {
        Self {
            x: value.x as f64,
            y: value.y as f64,
            z: value.z as f64,
        }
    }
}

/// A finite quaternion in `x, y, z, w` order. Use `UnitQuaterniond` for rotations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Quaterniond {
    x: f64,
    y: f64,
    z: f64,
    w: f64,
}

impl Quaterniond {
    /// Creates a finite raw quaternion.
    pub fn new(x: f64, y: f64, z: f64, w: f64) -> Result<Self> {
        Ok(Self {
            x: finite64(x, "quaternion x")?,
            y: finite64(y, "quaternion y")?,
            z: finite64(z, "quaternion z")?,
            w: finite64(w, "quaternion w")?,
        })
    }
    /// Components in serialization order: x, y, z, w.
    pub const fn components(self) -> [f64; 4] {
        [self.x, self.y, self.z, self.w]
    }
    /// Produces a normalized rotation quaternion.
    pub fn normalized(self) -> Result<UnitQuaterniond> {
        UnitQuaterniond::from_quaternion(self)
    }
}

/// A normalized quaternion representing a right-handed rotation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UnitQuaterniond {
    x: f64,
    y: f64,
    z: f64,
    w: f64,
}

impl UnitQuaterniond {
    /// Identity rotation.
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
    /// Normalizes a finite raw quaternion and rejects zero magnitude.
    pub fn from_quaternion(value: Quaterniond) -> Result<Self> {
        let magnitude = (value.x.mul_add(
            value.x,
            value
                .y
                .mul_add(value.y, value.z.mul_add(value.z, value.w * value.w)),
        ))
        .sqrt();
        if !magnitude.is_finite() {
            return Err(Geometry3dError::NonFinite("quaternion magnitude"));
        }
        if magnitude <= f64::EPSILON {
            return Err(Geometry3dError::Degenerate("quaternion"));
        }
        Ok(Self {
            x: value.x / magnitude,
            y: value.y / magnitude,
            z: value.z / magnitude,
            w: value.w / magnitude,
        })
    }
    /// Components in serialization order: x, y, z, w.
    pub const fn components(self) -> [f64; 4] {
        [self.x, self.y, self.z, self.w]
    }
    /// Builds a rotation from a non-zero axis and a radian angle.
    pub fn from_axis_angle(axis: Vector3d, radians: f64) -> Result<Self> {
        finite64(radians, "axis-angle radians")?;
        let axis = axis.normalized()?;
        let half = radians * 0.5;
        Quaterniond::new(
            axis.x * half.sin(),
            axis.y * half.sin(),
            axis.z * half.sin(),
            half.cos(),
        )?
        .normalized()
    }
    /// Builds a rotation from an explicit Euler import/export representation.
    pub fn from_euler(order: EulerOrder, x: f64, y: f64, z: f64) -> Result<Self> {
        let qx = Self::from_axis_angle(Vector3d::X, x)?;
        let qy = Self::from_axis_angle(Vector3d::Y, y)?;
        let qz = Self::from_axis_angle(Vector3d::Z, z)?;
        match order {
            EulerOrder::Xyz => qz.compose(qy)?.compose(qx),
            EulerOrder::Xzy => qy.compose(qz)?.compose(qx),
            EulerOrder::Yxz => qz.compose(qx)?.compose(qy),
            EulerOrder::Yzx => qx.compose(qz)?.compose(qy),
            EulerOrder::Zxy => qy.compose(qx)?.compose(qz),
            EulerOrder::Zyx => qx.compose(qy)?.compose(qz),
        }
    }
    /// Rotation composition: the returned rotation applies `rhs` first, then `self`.
    pub fn compose(self, rhs: Self) -> Result<Self> {
        Quaterniond::new(
            self.w.mul_add(
                rhs.x,
                self.x.mul_add(rhs.w, self.y * rhs.z - self.z * rhs.y),
            ),
            self.w.mul_add(
                rhs.y,
                -self.x * rhs.z + self.y.mul_add(rhs.w, self.z * rhs.x),
            ),
            self.w
                .mul_add(rhs.z, self.x * rhs.y - self.y * rhs.x + self.z * rhs.w),
            self.w
                .mul_add(rhs.w, -(self.x * rhs.x + self.y * rhs.y + self.z * rhs.z)),
        )?
        .normalized()
    }
    /// The inverse rotation.
    pub const fn inverse(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }
    /// Rotates a displacement without translating it.
    pub fn rotate_vector(self, vector: Vector3d) -> Result<Vector3d> {
        let u = Vector3d::new(self.x, self.y, self.z)?;
        let uv = u.cross(vector)?;
        let uuv = u.cross(uv)?;
        vector.plus(uv.scale(2.0 * self.w)?)?.plus(uuv.scale(2.0)?)
    }
    /// Converts this rotation to a row-major matrix with column-vector semantics.
    pub fn to_matrix3(self) -> Result<Matrix3d> {
        let (x, y, z, w) = (self.x, self.y, self.z, self.w);
        Matrix3d::new([
            [
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y - w * z),
                2.0 * (x * z + w * y),
            ],
            [
                2.0 * (x * y + w * z),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z - w * x),
            ],
            [
                2.0 * (x * z - w * y),
                2.0 * (y * z + w * x),
                1.0 - 2.0 * (x * x + y * y),
            ],
        ])
    }
    /// Returns the axis and non-negative radian angle represented by this rotation.
    pub fn to_axis_angle(self) -> Result<(Vector3d, f64)> {
        let angle = 2.0 * self.w.clamp(-1.0, 1.0).acos();
        let sin_half = (1.0 - self.w * self.w).max(0.0).sqrt();
        if sin_half <= f64::EPSILON {
            return Ok((Vector3d::X, 0.0));
        }
        Ok((
            Vector3d::new(self.x / sin_half, self.y / sin_half, self.z / sin_half)?.normalized()?,
            angle,
        ))
    }
    /// Converts to explicit Euler angles in the requested order. At gimbal lock,
    /// one valid solution is returned with the least-significant angle set to zero.
    pub fn to_euler(self, order: EulerOrder) -> Result<(f64, f64, f64)> {
        euler_from_matrix(order, self.to_matrix3()?.rows)
    }
    /// Spherical interpolation. `t` is finite and is intentionally not clamped.
    pub fn slerp(self, rhs: Self, t: f64) -> Result<Self> {
        finite64(t, "interpolation factor")?;
        let mut rhs = rhs;
        let mut dot = self.x.mul_add(
            rhs.x,
            self.y.mul_add(rhs.y, self.z.mul_add(rhs.z, self.w * rhs.w)),
        );
        if dot < 0.0 {
            rhs = Self {
                x: -rhs.x,
                y: -rhs.y,
                z: -rhs.z,
                w: -rhs.w,
            };
            dot = -dot;
        }
        if dot > 0.9995 {
            return Quaterniond::new(
                self.x + (rhs.x - self.x) * t,
                self.y + (rhs.y - self.y) * t,
                self.z + (rhs.z - self.z) * t,
                self.w + (rhs.w - self.w) * t,
            )?
            .normalized();
        }
        let theta0 = dot.clamp(-1.0, 1.0).acos();
        let theta = theta0 * t;
        let s0 = theta.cos() - dot * theta.sin() / theta0.sin();
        let s1 = theta.sin() / theta0.sin();
        Quaterniond::new(
            self.x * s0 + rhs.x * s1,
            self.y * s0 + rhs.y * s1,
            self.z * s0 + rhs.z * s1,
            self.w * s0 + rhs.w * s1,
        )?
        .normalized()
    }
    /// Checked conversion to the paired f32 rotation contract.
    pub fn to_f32_checked(self) -> Result<UnitQuaternion> {
        UnitQuaternion::from_components(
            as_f32(self.x, "quaternion x")?,
            as_f32(self.y, "quaternion y")?,
            as_f32(self.z, "quaternion z")?,
            as_f32(self.w, "quaternion w")?,
        )
    }
}

/// A finite f32 raw quaternion in `x, y, z, w` order.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Quaternion {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}
impl Quaternion {
    /// Creates a finite raw quaternion.
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Result<Self> {
        Ok(Self {
            x: finite32(x, "quaternion x")?,
            y: finite32(y, "quaternion y")?,
            z: finite32(z, "quaternion z")?,
            w: finite32(w, "quaternion w")?,
        })
    }
    /// Components in serialization order: x, y, z, w.
    pub const fn components(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }
    /// Produces a normalized rotation quaternion.
    pub fn normalized(self) -> Result<UnitQuaternion> {
        UnitQuaternion::from_components(self.x, self.y, self.z, self.w)
    }
}

/// A normalized f32 quaternion representing a right-handed rotation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UnitQuaternion {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}
impl UnitQuaternion {
    /// Identity rotation.
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
    /// Normalizes finite f32 components and rejects zero magnitude.
    pub fn from_components(x: f32, y: f32, z: f32, w: f32) -> Result<Self> {
        let source = Quaterniond::new(x as f64, y as f64, z as f64, w as f64)?.normalized()?;
        Ok(Self {
            x: as_f32(source.x, "quaternion x")?,
            y: as_f32(source.y, "quaternion y")?,
            z: as_f32(source.z, "quaternion z")?,
            w: as_f32(source.w, "quaternion w")?,
        })
    }
    /// Components in serialization order: x, y, z, w.
    pub const fn components(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }
    /// Builds a rotation from a non-zero axis and a radian angle.
    pub fn from_axis_angle(axis: Vector3, radians: f32) -> Result<Self> {
        UnitQuaterniond::from_axis_angle(
            axis.into(),
            finite32(radians, "axis-angle radians")? as f64,
        )?
        .to_f32_checked()
    }
    /// Builds a rotation from explicit Euler import/export angles in radians.
    pub fn from_euler(order: EulerOrder, x: f32, y: f32, z: f32) -> Result<Self> {
        UnitQuaterniond::from_euler(order, x as f64, y as f64, z as f64)?.to_f32_checked()
    }
    /// Rotation composition: the returned rotation applies `rhs` first, then `self`.
    pub fn compose(self, rhs: Self) -> Result<Self> {
        self.to_f64().compose(rhs.to_f64())?.to_f32_checked()
    }
    /// The inverse rotation.
    pub const fn inverse(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }
    /// Rotates a displacement.
    pub fn rotate_vector(self, vector: Vector3) -> Result<Vector3> {
        self.to_f64().rotate_vector(vector.into())?.to_f32_checked()
    }
    /// Widens to f64 and renormalizes the f32 representation.
    ///
    /// The f32 components were normalized before their checked narrowing, but
    /// that narrowing can change the f64 norm by a few ulps. Renormalizing here
    /// preserves the `UnitQuaterniond` invariant rather than only re-labeling
    /// the rounded components.
    pub fn to_f64(self) -> UnitQuaterniond {
        let (x, y, z, w) = (self.x as f64, self.y as f64, self.z as f64, self.w as f64);
        let magnitude = (x.mul_add(x, y.mul_add(y, z.mul_add(z, w * w)))).sqrt();
        UnitQuaterniond {
            x: x / magnitude,
            y: y / magnitude,
            z: z / magnitude,
            w: w / magnitude,
        }
    }
    /// Converts to explicit Euler angles in the requested order.
    pub fn to_euler(self, order: EulerOrder) -> Result<(f32, f32, f32)> {
        let (x, y, z) = self.to_f64().to_euler(order)?;
        Ok((
            as_f32(x, "Euler x")?,
            as_f32(y, "Euler y")?,
            as_f32(z, "Euler z")?,
        ))
    }
    /// Spherical interpolation. `t` is finite and intentionally not clamped.
    pub fn slerp(self, rhs: Self, t: f32) -> Result<Self> {
        UnitQuaterniond::slerp(self.to_f64(), rhs.to_f64(), t as f64)?.to_f32_checked()
    }
}

/// A finite 3×3 matrix, serialized row-major and applied to column vectors.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Matrix3d {
    rows: [[f64; 3]; 3],
}
impl Matrix3d {
    /// Identity matrix.
    pub const IDENTITY: Self = Self {
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
    /// Creates a finite row-major matrix.
    pub fn new(rows: [[f64; 3]; 3]) -> Result<Self> {
        for row in rows {
            for value in row {
                finite64(value, "matrix value")?;
            }
        }
        Ok(Self { rows })
    }
    /// Rows in row-major serialization order.
    pub const fn rows(self) -> [[f64; 3]; 3] {
        self.rows
    }
    /// Multiplies a column vector.
    pub fn apply_vector(self, vector: Vector3d) -> Result<Vector3d> {
        Vector3d::new(
            self.rows[0][0].mul_add(
                vector.x,
                self.rows[0][1].mul_add(vector.y, self.rows[0][2] * vector.z),
            ),
            self.rows[1][0].mul_add(
                vector.x,
                self.rows[1][1].mul_add(vector.y, self.rows[1][2] * vector.z),
            ),
            self.rows[2][0].mul_add(
                vector.x,
                self.rows[2][1].mul_add(vector.y, self.rows[2][2] * vector.z),
            ),
        )
    }
    /// Matrix composition: the result applies `rhs` first, then `self`.
    pub fn compose(self, rhs: Self) -> Result<Self> {
        let mut out = [[0.0; 3]; 3];
        for (row, target) in out.iter_mut().enumerate() {
            for (column, value) in target.iter_mut().enumerate() {
                *value = self.rows[row][0].mul_add(
                    rhs.rows[0][column],
                    self.rows[row][1]
                        .mul_add(rhs.rows[1][column], self.rows[row][2] * rhs.rows[2][column]),
                );
            }
        }
        Self::new(out)
    }
    /// Determinant.
    pub fn determinant(self) -> Result<f64> {
        finite64(
            self.rows[0][0]
                * (self.rows[1][1] * self.rows[2][2] - self.rows[1][2] * self.rows[2][1])
                - self.rows[0][1]
                    * (self.rows[1][0] * self.rows[2][2] - self.rows[1][2] * self.rows[2][0])
                + self.rows[0][2]
                    * (self.rows[1][0] * self.rows[2][1] - self.rows[1][1] * self.rows[2][0]),
            "matrix determinant",
        )
    }
    /// Inverse, rejecting singular matrices.
    pub fn inverse(self) -> Result<Self> {
        let d = self.determinant()?;
        if d.abs() <= f64::EPSILON {
            return Err(Geometry3dError::SingularMatrix);
        }
        let m = self.rows;
        Self::new([
            [
                (m[1][1] * m[2][2] - m[1][2] * m[2][1]) / d,
                (m[0][2] * m[2][1] - m[0][1] * m[2][2]) / d,
                (m[0][1] * m[1][2] - m[0][2] * m[1][1]) / d,
            ],
            [
                (m[1][2] * m[2][0] - m[1][0] * m[2][2]) / d,
                (m[0][0] * m[2][2] - m[0][2] * m[2][0]) / d,
                (m[0][2] * m[1][0] - m[0][0] * m[1][2]) / d,
            ],
            [
                (m[1][0] * m[2][1] - m[1][1] * m[2][0]) / d,
                (m[0][1] * m[2][0] - m[0][0] * m[2][1]) / d,
                (m[0][0] * m[1][1] - m[0][1] * m[1][0]) / d,
            ],
        ])
    }
    /// Checked conversion to f32.
    pub fn to_f32_checked(self) -> Result<Matrix3> {
        let mut rows = [[0.0_f32; 3]; 3];
        for (row_index, row) in rows.iter_mut().enumerate() {
            for (column_index, value) in row.iter_mut().enumerate() {
                *value = as_f32(self.rows[row_index][column_index], "matrix value")?;
            }
        }
        Matrix3::new(rows)
    }
}

/// A finite f32 3×3 matrix, serialized row-major and applied to column vectors.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Matrix3 {
    rows: [[f32; 3]; 3],
}
impl Matrix3 {
    /// Identity matrix.
    pub const IDENTITY: Self = Self {
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
    /// Creates a finite row-major matrix.
    pub fn new(rows: [[f32; 3]; 3]) -> Result<Self> {
        for row in rows {
            for value in row {
                finite32(value, "matrix value")?;
            }
        }
        Ok(Self { rows })
    }
    /// Rows in row-major serialization order.
    pub const fn rows(self) -> [[f32; 3]; 3] {
        self.rows
    }
    /// Converts to f64 without loss.
    pub fn to_f64(self) -> Matrix3d {
        Matrix3d {
            rows: self.rows.map(|row| row.map(f64::from)),
        }
    }
    /// Multiplies a column vector.
    pub fn apply_vector(self, vector: Vector3) -> Result<Vector3> {
        self.to_f64().apply_vector(vector.into())?.to_f32_checked()
    }
    /// Matrix composition: the result applies `rhs` first, then `self`.
    pub fn compose(self, rhs: Self) -> Result<Self> {
        self.to_f64().compose(rhs.to_f64())?.to_f32_checked()
    }
    /// Inverse, rejecting singular matrices.
    pub fn inverse(self) -> Result<Self> {
        self.to_f64().inverse()?.to_f32_checked()
    }
}

/// A finite 4×4 matrix, serialized row-major and applied to column vectors.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Matrix4d {
    rows: [[f64; 4]; 4],
}
impl Matrix4d {
    /// Identity matrix.
    pub const IDENTITY: Self = Self {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    /// Creates a finite row-major matrix.
    pub fn new(rows: [[f64; 4]; 4]) -> Result<Self> {
        for row in rows {
            for value in row {
                finite64(value, "matrix value")?;
            }
        }
        Ok(Self { rows })
    }
    /// Rows in row-major serialization order.
    pub const fn rows(self) -> [[f64; 4]; 4] {
        self.rows
    }
    /// Matrix composition: the result applies `rhs` first, then `self`.
    pub fn compose(self, rhs: Self) -> Result<Self> {
        let mut out = [[0.0; 4]; 4];
        for (r, target) in out.iter_mut().enumerate() {
            for (c, value) in target.iter_mut().enumerate() {
                *value = self.rows[r][0].mul_add(
                    rhs.rows[0][c],
                    self.rows[r][1].mul_add(
                        rhs.rows[1][c],
                        self.rows[r][2].mul_add(rhs.rows[2][c], self.rows[r][3] * rhs.rows[3][c]),
                    ),
                );
            }
        }
        Self::new(out)
    }
    /// Checked conversion to f32.
    pub fn to_f32_checked(self) -> Result<Matrix4> {
        let mut rows = [[0.0_f32; 4]; 4];
        for (r, row) in rows.iter_mut().enumerate() {
            for (c, value) in row.iter_mut().enumerate() {
                *value = as_f32(self.rows[r][c], "matrix value")?;
            }
        }
        Matrix4::new(rows)
    }
}

/// A finite f32 4×4 matrix, serialized row-major and applied to column vectors.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Matrix4 {
    rows: [[f32; 4]; 4],
}
impl Matrix4 {
    /// Identity matrix.
    pub const IDENTITY: Self = Self {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    /// Creates a finite row-major matrix.
    pub fn new(rows: [[f32; 4]; 4]) -> Result<Self> {
        for row in rows {
            for value in row {
                finite32(value, "matrix value")?;
            }
        }
        Ok(Self { rows })
    }
    /// Rows in row-major serialization order.
    pub const fn rows(self) -> [[f32; 4]; 4] {
        self.rows
    }
    /// Converts to f64 without loss.
    pub fn to_f64(self) -> Matrix4d {
        Matrix4d {
            rows: self.rows.map(|row| row.map(f64::from)),
        }
    }
    /// Matrix composition: the result applies `rhs` first, then `self`.
    pub fn compose(self, rhs: Self) -> Result<Self> {
        self.to_f64().compose(rhs.to_f64())?.to_f32_checked()
    }
}

/// A rotation and translation transform. Points are rotated then translated;
/// vectors are only rotated.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RigidTransform3d {
    rotation: UnitQuaterniond,
    translation: Vector3d,
}
impl RigidTransform3d {
    /// Identity transform.
    pub const IDENTITY: Self = Self {
        rotation: UnitQuaterniond::IDENTITY,
        translation: Vector3d::ZERO,
    };
    /// Creates a rigid transform.
    pub const fn new(rotation: UnitQuaterniond, translation: Vector3d) -> Result<Self> {
        Ok(Self {
            rotation,
            translation,
        })
    }
    /// Rotation component.
    pub const fn rotation(self) -> UnitQuaterniond {
        self.rotation
    }
    /// Translation component.
    pub const fn translation(self) -> Vector3d {
        self.translation
    }
    /// Applies this transform to a point.
    pub fn apply_point(self, point: Point3d) -> Result<Point3d> {
        let rotated = self
            .rotation
            .rotate_vector(Vector3d::new(point.x, point.y, point.z)?)?;
        Point3d::ORIGIN
            .translate(rotated)?
            .translate(self.translation)
    }
    /// Applies this transform to a vector.
    pub fn apply_vector(self, vector: Vector3d) -> Result<Vector3d> {
        self.rotation.rotate_vector(vector)
    }
    /// Composition: the result applies `rhs` first, then `self`.
    pub fn compose(self, rhs: Self) -> Result<Self> {
        Self::new(
            self.rotation.compose(rhs.rotation)?,
            self.rotation
                .rotate_vector(rhs.translation)?
                .plus(self.translation)?,
        )
    }
    /// Inverse transform.
    pub fn inverse(self) -> Result<Self> {
        let rotation = self.rotation.inverse();
        Self::new(
            rotation,
            rotation.rotate_vector(self.translation.scale(-1.0)?)?,
        )
    }
    /// Converts to an affine matrix representation.
    pub fn to_affine(self) -> Result<AffineTransform3d> {
        AffineTransform3d::new(self.rotation.to_matrix3()?, self.translation)
    }
    /// Checked conversion to f32.
    pub fn to_f32_checked(self) -> Result<RigidTransform3> {
        RigidTransform3::new(
            self.rotation.to_f32_checked()?,
            self.translation.to_f32_checked()?,
        )
    }
}

/// A rotation and translation f32 transform.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RigidTransform3 {
    rotation: UnitQuaternion,
    translation: Vector3,
}
impl RigidTransform3 {
    /// Identity transform.
    pub const IDENTITY: Self = Self {
        rotation: UnitQuaternion::IDENTITY,
        translation: Vector3::ZERO,
    };
    /// Creates a rigid transform.
    pub const fn new(rotation: UnitQuaternion, translation: Vector3) -> Result<Self> {
        Ok(Self {
            rotation,
            translation,
        })
    }
    /// Rotation component.
    pub const fn rotation(self) -> UnitQuaternion {
        self.rotation
    }
    /// Translation component.
    pub const fn translation(self) -> Vector3 {
        self.translation
    }
    /// Applies this transform to a point.
    pub fn apply_point(self, point: Point3) -> Result<Point3> {
        self.to_f64().apply_point(point.into())?.to_f32_checked()
    }
    /// Applies this transform to a vector.
    pub fn apply_vector(self, vector: Vector3) -> Result<Vector3> {
        self.to_f64().apply_vector(vector.into())?.to_f32_checked()
    }
    /// Composition: the result applies `rhs` first, then `self`.
    pub fn compose(self, rhs: Self) -> Result<Self> {
        self.to_f64().compose(rhs.to_f64())?.to_f32_checked()
    }
    /// Inverse transform.
    pub fn inverse(self) -> Result<Self> {
        self.to_f64().inverse()?.to_f32_checked()
    }
    /// Converts to f64 without loss.
    pub fn to_f64(self) -> RigidTransform3d {
        RigidTransform3d {
            rotation: self.rotation.to_f64(),
            translation: self.translation.into(),
        }
    }
}

/// An affine linear transform and translation. The linear term may include
/// scale or shear; perspective matrices are deliberately excluded.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AffineTransform3d {
    linear: Matrix3d,
    translation: Vector3d,
}
impl AffineTransform3d {
    /// Identity transform.
    pub const IDENTITY: Self = Self {
        linear: Matrix3d::IDENTITY,
        translation: Vector3d::ZERO,
    };
    /// Creates an affine transform from its linear and translation terms.
    pub const fn new(linear: Matrix3d, translation: Vector3d) -> Result<Self> {
        Ok(Self {
            linear,
            translation,
        })
    }
    /// Builds from a row-major homogeneous affine matrix.
    pub fn from_matrix4(matrix: Matrix4d) -> Result<Self> {
        let r = matrix.rows;
        if r[3][0].abs() > f64::EPSILON
            || r[3][1].abs() > f64::EPSILON
            || r[3][2].abs() > f64::EPSILON
            || (r[3][3] - 1.0).abs() > f64::EPSILON
        {
            return Err(Geometry3dError::NonAffineMatrix);
        }
        Self::new(
            Matrix3d::new([
                [r[0][0], r[0][1], r[0][2]],
                [r[1][0], r[1][1], r[1][2]],
                [r[2][0], r[2][1], r[2][2]],
            ])?,
            Vector3d::new(r[0][3], r[1][3], r[2][3])?,
        )
    }
    /// Linear term.
    pub const fn linear(self) -> Matrix3d {
        self.linear
    }
    /// Translation term.
    pub const fn translation(self) -> Vector3d {
        self.translation
    }
    /// Applies this transform to a point.
    pub fn apply_point(self, point: Point3d) -> Result<Point3d> {
        let result = self
            .linear
            .apply_vector(Vector3d::new(point.x, point.y, point.z)?)?
            .plus(self.translation)?;
        Point3d::new(result.x, result.y, result.z)
    }
    /// Applies only the linear term to a vector.
    pub fn apply_vector(self, vector: Vector3d) -> Result<Vector3d> {
        self.linear.apply_vector(vector)
    }
    /// Composition: the result applies `rhs` first, then `self`.
    pub fn compose(self, rhs: Self) -> Result<Self> {
        Self::new(
            self.linear.compose(rhs.linear)?,
            self.linear
                .apply_vector(rhs.translation)?
                .plus(self.translation)?,
        )
    }
    /// Inverse transform, rejecting singular linear terms.
    pub fn inverse(self) -> Result<Self> {
        let linear = self.linear.inverse()?;
        Self::new(linear, linear.apply_vector(self.translation.scale(-1.0)?)?)
    }
    /// Homogeneous row-major matrix representation.
    pub fn to_matrix4(self) -> Matrix4d {
        let m = self.linear.rows;
        Matrix4d {
            rows: [
                [m[0][0], m[0][1], m[0][2], self.translation.x],
                [m[1][0], m[1][1], m[1][2], self.translation.y],
                [m[2][0], m[2][1], m[2][2], self.translation.z],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
    /// Checked conversion to f32.
    pub fn to_f32_checked(self) -> Result<AffineTransform3> {
        AffineTransform3::new(
            self.linear.to_f32_checked()?,
            self.translation.to_f32_checked()?,
        )
    }
}

/// An f32 affine linear transform and translation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AffineTransform3 {
    linear: Matrix3,
    translation: Vector3,
}
impl AffineTransform3 {
    /// Identity transform.
    pub const IDENTITY: Self = Self {
        linear: Matrix3::IDENTITY,
        translation: Vector3::ZERO,
    };
    /// Creates an affine transform from its linear and translation terms.
    pub const fn new(linear: Matrix3, translation: Vector3) -> Result<Self> {
        Ok(Self {
            linear,
            translation,
        })
    }
    /// Builds from a row-major homogeneous affine matrix.
    pub fn from_matrix4(matrix: Matrix4) -> Result<Self> {
        AffineTransform3d::from_matrix4(matrix.to_f64())?.to_f32_checked()
    }
    /// Applies this transform to a point.
    pub fn apply_point(self, point: Point3) -> Result<Point3> {
        self.to_f64().apply_point(point.into())?.to_f32_checked()
    }
    /// Applies only the linear term to a vector.
    pub fn apply_vector(self, vector: Vector3) -> Result<Vector3> {
        self.to_f64().apply_vector(vector.into())?.to_f32_checked()
    }
    /// Composition: the result applies `rhs` first, then `self`.
    pub fn compose(self, rhs: Self) -> Result<Self> {
        self.to_f64().compose(rhs.to_f64())?.to_f32_checked()
    }
    /// Inverse transform, rejecting singular linear terms.
    pub fn inverse(self) -> Result<Self> {
        self.to_f64().inverse()?.to_f32_checked()
    }
    /// Converts to f64 without loss.
    pub fn to_f64(self) -> AffineTransform3d {
        AffineTransform3d {
            linear: self.linear.to_f64(),
            translation: self.translation.into(),
        }
    }
}

#[cfg(feature = "nalgebra-adapters")]
impl Vector3d {
    /// Converts at the optional nalgebra adapter seam.
    pub fn to_nalgebra(self) -> nalgebra::Vector3<f64> {
        nalgebra::Vector3::new(self.x, self.y, self.z)
    }
    /// Validates a nalgebra value at the adapter seam.
    pub fn from_nalgebra(value: nalgebra::Vector3<f64>) -> Result<Self> {
        Self::new(value.x, value.y, value.z)
    }
}
#[cfg(feature = "nalgebra-adapters")]
impl Point3d {
    /// Converts at the optional nalgebra adapter seam.
    pub fn to_nalgebra(self) -> nalgebra::Point3<f64> {
        nalgebra::Point3::new(self.x, self.y, self.z)
    }
    /// Validates a nalgebra value at the adapter seam.
    pub fn from_nalgebra(value: nalgebra::Point3<f64>) -> Result<Self> {
        Self::new(value.x, value.y, value.z)
    }
}
#[cfg(feature = "nalgebra-adapters")]
impl UnitQuaterniond {
    /// Converts at the optional nalgebra adapter seam.
    pub fn to_nalgebra(self) -> nalgebra::UnitQuaternion<f64> {
        nalgebra::UnitQuaternion::from_quaternion(nalgebra::Quaternion::new(
            self.w, self.x, self.y, self.z,
        ))
    }
    /// Validates a nalgebra rotation at the adapter seam.
    pub fn from_nalgebra(value: nalgebra::UnitQuaternion<f64>) -> Result<Self> {
        let q = value.quaternion();
        Quaterniond::new(q.i, q.j, q.k, q.w)?.normalized()
    }
}

fn euler_from_matrix(order: EulerOrder, rows: [[f64; 3]; 3]) -> Result<(f64, f64, f64)> {
    let epsilon = 1.0e-12;
    let angles = match order {
        EulerOrder::Xyz => {
            let y = (-rows[2][0]).clamp(-1.0, 1.0).asin();
            if y.cos().abs() > epsilon {
                (
                    rows[2][1].atan2(rows[2][2]),
                    y,
                    rows[1][0].atan2(rows[0][0]),
                )
            } else {
                (0.0, y, (-rows[0][1]).atan2(rows[1][1]))
            }
        }
        EulerOrder::Xzy => {
            let z = rows[1][0].clamp(-1.0, 1.0).asin();
            if z.cos().abs() > epsilon {
                (
                    (-rows[1][2]).atan2(rows[1][1]),
                    (-rows[2][0]).atan2(rows[0][0]),
                    z,
                )
            } else {
                (rows[2][1].atan2(rows[2][2]), 0.0, z)
            }
        }
        EulerOrder::Yxz => {
            let x = rows[2][1].clamp(-1.0, 1.0).asin();
            if x.cos().abs() > epsilon {
                (
                    x,
                    (-rows[2][0]).atan2(rows[2][2]),
                    (-rows[0][1]).atan2(rows[1][1]),
                )
            } else {
                (x, rows[0][2].atan2(rows[0][0]), 0.0)
            }
        }
        EulerOrder::Yzx => {
            let z = (-rows[0][1]).clamp(-1.0, 1.0).asin();
            if z.cos().abs() > epsilon {
                (
                    rows[2][1].atan2(rows[1][1]),
                    rows[0][2].atan2(rows[0][0]),
                    z,
                )
            } else {
                (0.0, (-rows[2][0]).atan2(rows[2][2]), z)
            }
        }
        EulerOrder::Zxy => {
            let x = (-rows[1][2]).clamp(-1.0, 1.0).asin();
            if x.cos().abs() > epsilon {
                (
                    x,
                    rows[0][2].atan2(rows[2][2]),
                    rows[1][0].atan2(rows[1][1]),
                )
            } else {
                (x, 0.0, (-rows[0][1]).atan2(rows[0][0]))
            }
        }
        EulerOrder::Zyx => {
            let y = rows[0][2].clamp(-1.0, 1.0).asin();
            if y.cos().abs() > epsilon {
                (
                    (-rows[1][2]).atan2(rows[2][2]),
                    y,
                    (-rows[0][1]).atan2(rows[0][0]),
                )
            } else {
                (rows[2][1].atan2(rows[1][1]), y, 0.0)
            }
        }
    };
    (angles.0.is_finite() && angles.1.is_finite() && angles.2.is_finite())
        .then_some(angles)
        .ok_or(Geometry3dError::NonFinite("Euler angle"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn close(left: f64, right: f64) {
        assert!((left - right).abs() < 1.0e-10, "{left} != {right}");
    }

    #[test]
    fn points_vectors_and_non_finite_values_have_distinct_invariants() {
        let point = Point3d::new(1.0, 2.0, 3.0).unwrap();
        let vector = Vector3d::new(2.0, 0.0, -1.0).unwrap();
        assert_eq!(
            point.translate(vector).unwrap().coordinates(),
            [3.0, 2.0, 2.0]
        );
        assert!(Vector3d::new(f64::NAN, 0.0, 0.0).is_err());
        assert!(Point3::new(f32::INFINITY, 0.0, 0.0).is_err());
    }

    #[test]
    fn rotations_and_affine_transforms_preserve_documented_composition() {
        let quarter =
            UnitQuaterniond::from_axis_angle(Vector3d::Z, std::f64::consts::FRAC_PI_2).unwrap();
        let rotated = quarter.rotate_vector(Vector3d::X).unwrap();
        close(rotated.y(), 1.0);
        let transform =
            RigidTransform3d::new(quarter, Vector3d::new(2.0, 0.0, 0.0).unwrap()).unwrap();
        let point = transform
            .apply_point(Point3d::new(1.0, 0.0, 0.0).unwrap())
            .unwrap();
        close(point.x(), 2.0);
        close(point.y(), 1.0);
        let inverse = transform.inverse().unwrap();
        let recovered = inverse.apply_point(point).unwrap();
        close(recovered.x(), 1.0);
        close(recovered.y(), 0.0);
    }

    #[test]
    fn euler_round_trips_each_explicit_order() {
        for order in [
            EulerOrder::Xyz,
            EulerOrder::Xzy,
            EulerOrder::Yxz,
            EulerOrder::Yzx,
            EulerOrder::Zxy,
            EulerOrder::Zyx,
        ] {
            let source = UnitQuaterniond::from_euler(order, 0.2, -0.3, 0.4).unwrap();
            let (x, y, z) = source.to_euler(order).unwrap();
            let recovered = UnitQuaterniond::from_euler(order, x, y, z).unwrap();
            let dot = source
                .components()
                .iter()
                .zip(recovered.components())
                .map(|(a, b)| a * b)
                .sum::<f64>();
            assert!(
                dot.abs() > 1.0 - 1.0e-10,
                "{order:?}: {dot}; recovered = {x}, {y}, {z}"
            );
        }
    }

    proptest! {
        #[test]
        fn f64_to_f32_conversion_never_admits_non_finite(x in any::<f64>(), y in any::<f64>(), z in any::<f64>()) {
            let result=Vector3d::new(x,y,z).and_then(Vector3d::to_f32_checked);
            if let Ok(vector)=result { prop_assert!(vector.x().is_finite() && vector.y().is_finite() && vector.z().is_finite()); }
        }
    }
}
