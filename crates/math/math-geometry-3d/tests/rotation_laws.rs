#[path = "../../../test-support/numerical.rs"]
mod numerical;

use math_geometry_3d::{
    AffineTransform3d, EulerOrder, Geometry3dError, Matrix3d, Matrix4d, Point3d, Quaternion,
    Quaterniond, RigidTransform3d, UnitQuaternion, UnitQuaterniond, Vector3, Vector3d,
};
use numerical::{assert_approx_eq_f64, deterministic_config, ApproxTolerance};
use proptest::prelude::*;

fn tolerance() -> ApproxTolerance {
    ApproxTolerance::new(1.0e-10, 1.0e-10).unwrap()
}

fn assert_vector_close(left: Vector3d, right: Vector3d) {
    let tolerance = tolerance();
    assert_approx_eq_f64(left.x(), right.x(), tolerance);
    assert_approx_eq_f64(left.y(), right.y(), tolerance);
    assert_approx_eq_f64(left.z(), right.z(), tolerance);
}

fn assert_vector3_close(left: Vector3, right: Vector3) {
    let tolerance = ApproxTolerance::new(2.0e-5, 2.0e-5).unwrap();
    numerical::assert_approx_eq_f32(left.x(), right.x(), tolerance);
    numerical::assert_approx_eq_f32(left.y(), right.y(), tolerance);
    numerical::assert_approx_eq_f32(left.z(), right.z(), tolerance);
}

fn assert_point_close(left: Point3d, right: Point3d) {
    let tolerance = tolerance();
    assert_approx_eq_f64(left.x(), right.x(), tolerance);
    assert_approx_eq_f64(left.y(), right.y(), tolerance);
    assert_approx_eq_f64(left.z(), right.z(), tolerance);
}

fn quaternion_dot(left: UnitQuaterniond, right: UnitQuaterniond) -> f64 {
    left.components()
        .into_iter()
        .zip(right.components())
        .map(|(left, right)| left * right)
        .sum()
}

fn arbitrary_rotation() -> impl Strategy<Value = (Vector3d, f64)> {
    (
        -1.0_f64..=1.0_f64,
        -1.0_f64..=1.0_f64,
        -1.0_f64..=1.0_f64,
        -std::f64::consts::PI..=std::f64::consts::PI,
    )
        .prop_filter_map("axis must be non-zero", |(x, y, z, angle)| {
            Vector3d::new(x, y, z)
                .ok()
                .filter(|axis| axis.magnitude().is_ok_and(|length| length > 1.0e-5))
                .map(|axis| (axis, angle))
        })
}

proptest! {
    #![proptest_config(deterministic_config())]

    #[test]
    fn quaternion_identity_inverse_composition_and_matrix_laws(
        (axis, angle) in arbitrary_rotation(),
        (second_axis, second_angle) in arbitrary_rotation(),
        (third_axis, third_angle) in arbitrary_rotation(),
        vx in -100.0_f64..=100.0_f64,
        vy in -100.0_f64..=100.0_f64,
        vz in -100.0_f64..=100.0_f64,
    ) {
        let quaternion = UnitQuaterniond::from_axis_angle(axis, angle).unwrap();
        let second = UnitQuaterniond::from_axis_angle(second_axis, second_angle).unwrap();
        let third = UnitQuaterniond::from_axis_angle(third_axis, third_angle).unwrap();
        let vector = Vector3d::new(vx, vy, vz).unwrap();

        let identity = quaternion.compose(quaternion.inverse()).unwrap();
        assert_vector_close(identity.rotate_vector(vector).unwrap(), vector);
        assert_vector_close(
            quaternion.inverse().rotate_vector(quaternion.rotate_vector(vector).unwrap()).unwrap(),
            vector,
        );
        assert_approx_eq_f64(vector.magnitude().unwrap(), quaternion.rotate_vector(vector).unwrap().magnitude().unwrap(), tolerance());

        let [x, y, z, w] = quaternion.components();
        let negated = Quaterniond::new(-x, -y, -z, -w).unwrap().normalized().unwrap();
        assert_vector_close(quaternion.rotate_vector(vector).unwrap(), negated.rotate_vector(vector).unwrap());

        let matrix = quaternion.to_matrix3().unwrap();
        matrix.validate_rotation().unwrap();
        assert_approx_eq_f64(matrix.determinant().unwrap(), 1.0, tolerance());
        let recovered = UnitQuaterniond::from_matrix3(matrix).unwrap();
        prop_assert!(quaternion_dot(quaternion, recovered).abs() > 1.0 - 1.0e-10);

        assert_vector_close(
            quaternion.compose(second).unwrap().rotate_vector(vector).unwrap(),
            quaternion.rotate_vector(second.rotate_vector(vector).unwrap()).unwrap(),
        );
        assert_vector_close(
            quaternion.compose(second).unwrap().compose(third).unwrap().rotate_vector(vector).unwrap(),
            quaternion.compose(second.compose(third).unwrap()).unwrap().rotate_vector(vector).unwrap(),
        );
    }

    #[test]
    fn axis_angle_euler_and_transform_laws(
        (axis, angle) in arbitrary_rotation(),
        x in -0.8_f64..=0.8_f64,
        y in -0.8_f64..=0.8_f64,
        z in -0.8_f64..=0.8_f64,
        px in -100.0_f64..=100.0_f64,
        py in -100.0_f64..=100.0_f64,
        pz in -100.0_f64..=100.0_f64,
    ) {
        let quaternion = UnitQuaterniond::from_axis_angle(axis, angle).unwrap();
        let (recovered_axis, recovered_angle) = quaternion.to_axis_angle().unwrap();
        let axis_angle_roundtrip = UnitQuaterniond::from_axis_angle(recovered_axis, recovered_angle).unwrap();
        prop_assert!(quaternion_dot(quaternion, axis_angle_roundtrip).abs() > 1.0 - 1.0e-10);

        for order in [EulerOrder::Xyz, EulerOrder::Xzy, EulerOrder::Yxz, EulerOrder::Yzx, EulerOrder::Zxy, EulerOrder::Zyx] {
            let source = UnitQuaterniond::from_euler(order, x, y, z).unwrap();
            let (rx, ry, rz) = source.to_euler(order).unwrap();
            let recovered = UnitQuaterniond::from_euler(order, rx, ry, rz).unwrap();
            prop_assert!(quaternion_dot(source, recovered).abs() > 1.0 - 1.0e-10, "{order:?}");
        }

        let translation = Vector3d::new(3.0, -2.0, 1.0).unwrap();
        let transform = RigidTransform3d::new(quaternion, translation).unwrap();
        let point = Point3d::new(px, py, pz).unwrap();
        assert_point_close(transform.inverse().unwrap().apply_point(transform.apply_point(point).unwrap()).unwrap(), point);
        let vector = Vector3d::new(px, py, pz).unwrap();
        assert_vector_close(transform.apply_vector(vector).unwrap(), quaternion.rotate_vector(vector).unwrap());
        assert_point_close(transform.apply_point(Point3d::ORIGIN).unwrap(), Point3d::ORIGIN.translate(translation).unwrap());
    }

    #[test]
    fn rigid_and_affine_transform_inverse_and_composition_laws(
        (first_axis, first_angle) in arbitrary_rotation(),
        (second_axis, second_angle) in arbitrary_rotation(),
        tx1 in -20.0_f64..=20.0_f64,
        ty1 in -20.0_f64..=20.0_f64,
        tz1 in -20.0_f64..=20.0_f64,
        tx2 in -20.0_f64..=20.0_f64,
        ty2 in -20.0_f64..=20.0_f64,
        tz2 in -20.0_f64..=20.0_f64,
        x in -100.0_f64..=100.0_f64,
        y in -100.0_f64..=100.0_f64,
        z in -100.0_f64..=100.0_f64,
    ) {
        let first = RigidTransform3d::new(
            UnitQuaterniond::from_axis_angle(first_axis, first_angle).unwrap(),
            Vector3d::new(tx1, ty1, tz1).unwrap(),
        ).unwrap();
        let second = RigidTransform3d::new(
            UnitQuaterniond::from_axis_angle(second_axis, second_angle).unwrap(),
            Vector3d::new(tx2, ty2, tz2).unwrap(),
        ).unwrap();
        let point = Point3d::new(x, y, z).unwrap();
        let vector = Vector3d::new(x, y, z).unwrap();

        assert_point_close(first.inverse().unwrap().apply_point(first.apply_point(point).unwrap()).unwrap(), point);
        assert_vector_close(first.inverse().unwrap().apply_vector(first.apply_vector(vector).unwrap()).unwrap(), vector);
        let composed = first.compose(second).unwrap();
        assert_point_close(
            composed.apply_point(point).unwrap(),
            first.apply_point(second.apply_point(point).unwrap()).unwrap(),
        );
        assert_vector_close(
            composed.apply_vector(vector).unwrap(),
            first.apply_vector(second.apply_vector(vector).unwrap()).unwrap(),
        );

        let first_affine = first.to_affine().unwrap();
        let second_affine = second.to_affine().unwrap();
        assert_point_close(
            first_affine.inverse().unwrap().apply_point(first_affine.apply_point(point).unwrap()).unwrap(),
            point,
        );
        assert_vector_close(
            first_affine.inverse().unwrap().apply_vector(first_affine.apply_vector(vector).unwrap()).unwrap(),
            vector,
        );
        let affine_composed = first_affine.compose(second_affine).unwrap();
        assert_point_close(
            affine_composed.apply_point(point).unwrap(),
            first_affine.apply_point(second_affine.apply_point(point).unwrap()).unwrap(),
        );
        assert_vector_close(
            affine_composed.apply_vector(vector).unwrap(),
            first_affine.apply_vector(second_affine.apply_vector(vector).unwrap()).unwrap(),
        );
    }
}

#[test]
fn convention_canaries_cover_handedness_layout_composition_and_serialization() {
    let quarter_turn =
        UnitQuaterniond::from_axis_angle(Vector3d::Z, std::f64::consts::FRAC_PI_2).unwrap();
    assert_vector_close(
        quarter_turn.rotate_vector(Vector3d::X).unwrap(),
        Vector3d::Y,
    );

    let translate_x = AffineTransform3d::from_matrix4(
        math_geometry_3d::Matrix4d::new([
            [1.0, 0.0, 0.0, 2.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
        .unwrap(),
    )
    .unwrap();
    assert_point_close(
        translate_x
            .apply_point(Point3d::new(1.0, 0.0, 0.0).unwrap())
            .unwrap(),
        Point3d::new(3.0, 0.0, 0.0).unwrap(),
    );
    assert_vector_close(translate_x.apply_vector(Vector3d::X).unwrap(), Vector3d::X);

    let rotate_then_translate =
        RigidTransform3d::new(quarter_turn, Vector3d::new(2.0, 0.0, 0.0).unwrap()).unwrap();
    let translation_only = RigidTransform3d::new(
        UnitQuaterniond::IDENTITY,
        Vector3d::new(1.0, 0.0, 0.0).unwrap(),
    )
    .unwrap();
    let point = Point3d::new(1.0, 0.0, 0.0).unwrap();
    assert_point_close(
        rotate_then_translate
            .compose(translation_only)
            .unwrap()
            .apply_point(point)
            .unwrap(),
        rotate_then_translate
            .apply_point(translation_only.apply_point(point).unwrap())
            .unwrap(),
    );

    assert_eq!(
        serde_json::to_string(&quarter_turn).unwrap(),
        r#"{"x":0.0,"y":0.0,"z":0.7071067811865475,"w":0.7071067811865476}"#
    );
    assert_eq!(
        serde_json::to_string(&Matrix3d::IDENTITY).unwrap(),
        r#"{"rows":[[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]}"#
    );
}

#[test]
fn edge_cases_and_invalid_states_are_rejected() {
    assert!(UnitQuaterniond::from_axis_angle(Vector3d::ZERO, 0.1).is_err());
    assert!(Quaterniond::new(1.0e-320, 0.0, 0.0, 0.0)
        .unwrap()
        .normalized()
        .is_err());
    assert!(
        Matrix3d::new([[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
            .and_then(UnitQuaterniond::from_matrix3)
            .is_err()
    );
    assert!(Matrix3d::new([[f64::NAN, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).is_err());
    assert!(matches!(
        Vector3d::new(f64::MAX, 0.0, 0.0).unwrap().to_f32_checked(),
        Err(Geometry3dError::NotRepresentableAsF32("vector x"))
    ));
    assert!(Point3d::new(0.0, f64::MAX, 0.0)
        .unwrap()
        .to_f32_checked()
        .is_err());
    let oversized_matrix3 =
        Matrix3d::new([[f64::MAX, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
    assert!(oversized_matrix3.to_f32_checked().is_err());
    let oversized_matrix4 = Matrix4d::new([
        [1.0, 0.0, 0.0, f64::MAX],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
    .unwrap();
    assert!(oversized_matrix4.to_f32_checked().is_err());
    assert!(RigidTransform3d::new(
        UnitQuaterniond::IDENTITY,
        Vector3d::new(f64::MAX, 0.0, 0.0).unwrap(),
    )
    .unwrap()
    .to_f32_checked()
    .is_err());
    assert!(AffineTransform3d::new(oversized_matrix3, Vector3d::ZERO)
        .unwrap()
        .to_f32_checked()
        .is_err());

    for axis in [Vector3d::X, Vector3d::Y, Vector3d::Z] {
        for angle in [1.0e-14, std::f64::consts::PI - 1.0e-12] {
            let source = UnitQuaterniond::from_axis_angle(axis, angle).unwrap();
            let (recovered_axis, recovered_angle) = source.to_axis_angle().unwrap();
            let recovered =
                UnitQuaterniond::from_axis_angle(recovered_axis, recovered_angle).unwrap();
            assert!(quaternion_dot(source, recovered).abs() > 1.0 - 1.0e-10);
            let matrix_recovered =
                UnitQuaterniond::from_matrix3(source.to_matrix3().unwrap()).unwrap();
            assert!(quaternion_dot(source, matrix_recovered).abs() > 1.0 - 1.0e-10);
            let matrix_roundtrip = matrix_recovered.to_matrix3().unwrap();
            for (actual_row, expected_row) in matrix_roundtrip
                .rows()
                .into_iter()
                .zip(source.to_matrix3().unwrap().rows())
            {
                for (actual, expected) in actual_row.into_iter().zip(expected_row) {
                    assert_approx_eq_f64(actual, expected, tolerance());
                }
            }
        }
    }

    let singular_cases = [
        (EulerOrder::Xyz, 1),
        (EulerOrder::Xzy, 2),
        (EulerOrder::Yxz, 0),
        (EulerOrder::Yzx, 2),
        (EulerOrder::Zxy, 0),
        (EulerOrder::Zyx, 1),
    ];
    for (order, singular_axis) in singular_cases {
        for singular_angle in [std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2] {
            let mut angles = [0.2, -0.3, 0.4];
            angles[singular_axis] = singular_angle;
            let source =
                UnitQuaterniond::from_euler(order, angles[0], angles[1], angles[2]).unwrap();
            let (x, y, z) = source.to_euler(order).unwrap();
            let recovered = UnitQuaterniond::from_euler(order, x, y, z).unwrap();
            assert!(
                quaternion_dot(source, recovered).abs() > 1.0 - 1.0e-10,
                "{order:?} at {singular_angle}: recovered {x}, {y}, {z}"
            );
            let returned = [x, y, z];
            let zeroed_axis = match order {
                EulerOrder::Xyz | EulerOrder::Yzx => 0,
                EulerOrder::Xzy | EulerOrder::Zxy => 1,
                EulerOrder::Yxz | EulerOrder::Zyx => 2,
            };
            assert!(
                returned[zeroed_axis].abs() <= 1.0e-10,
                "{order:?}: {returned:?}"
            );
        }
    }
}

#[test]
fn quaternion_deserialization_preserves_rotation_invariants() {
    let normalized: UnitQuaterniond =
        serde_json::from_str(r#"{"x":0.0,"y":0.0,"z":0.0,"w":2.0}"#).unwrap();
    assert_eq!(normalized, UnitQuaterniond::IDENTITY);
    assert!(
        serde_json::from_str::<UnitQuaterniond>(r#"{"x":0.0,"y":0.0,"z":0.0,"w":0.0}"#).is_err()
    );
    assert!(
        serde_json::from_str::<UnitQuaternion>(r#"{"x":0.0,"y":0.0,"z":0.0,"w":0.0}"#).is_err()
    );
    assert!(serde_json::from_str::<RigidTransform3d>(
        r#"{"rotation":{"x":0.0,"y":0.0,"z":0.0,"w":0.0},"translation":{"x":0.0,"y":0.0,"z":0.0}}"#
    )
    .is_err());
}

#[test]
fn slerp_covers_endpoints_midpoint_shortest_path_and_validation() {
    let start = UnitQuaterniond::IDENTITY;
    let end = UnitQuaterniond::from_axis_angle(Vector3d::Z, std::f64::consts::PI).unwrap();
    let midpoint = start.slerp(end, 0.5).unwrap();
    assert_vector_close(
        start
            .slerp(end, 0.0)
            .unwrap()
            .rotate_vector(Vector3d::X)
            .unwrap(),
        Vector3d::X,
    );
    assert_vector_close(
        start
            .slerp(end, 1.0)
            .unwrap()
            .rotate_vector(Vector3d::X)
            .unwrap(),
        end.rotate_vector(Vector3d::X).unwrap(),
    );
    assert_vector_close(midpoint.rotate_vector(Vector3d::X).unwrap(), Vector3d::Y);

    let [x, y, z, w] = end.components();
    let equivalent_negative = Quaterniond::new(-x, -y, -z, -w)
        .unwrap()
        .normalized()
        .unwrap();
    assert_vector_close(
        start
            .slerp(equivalent_negative, 0.5)
            .unwrap()
            .rotate_vector(Vector3d::X)
            .unwrap(),
        midpoint.rotate_vector(Vector3d::X).unwrap(),
    );
    let norm = midpoint
        .components()
        .into_iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    assert_approx_eq_f64(norm, 1.0, tolerance());
    assert!(start.slerp(end, f64::NAN).is_err());

    let start_f32 = UnitQuaternion::IDENTITY;
    let end_f32 = UnitQuaternion::from_axis_angle(Vector3::Z, 1.0).unwrap();
    let midpoint_f32 = start_f32.slerp(end_f32, 0.5).unwrap();
    assert_vector3_close(
        start_f32
            .slerp(end_f32, 0.0)
            .unwrap()
            .rotate_vector(Vector3::X)
            .unwrap(),
        Vector3::X,
    );
    assert_vector3_close(
        start_f32
            .slerp(end_f32, 1.0)
            .unwrap()
            .rotate_vector(Vector3::X)
            .unwrap(),
        end_f32.rotate_vector(Vector3::X).unwrap(),
    );
    assert_vector3_close(
        midpoint_f32.rotate_vector(Vector3::X).unwrap(),
        UnitQuaternion::from_axis_angle(Vector3::Z, 0.5)
            .unwrap()
            .rotate_vector(Vector3::X)
            .unwrap(),
    );
    let [x, y, z, w] = end_f32.components();
    let equivalent_negative_f32 = Quaternion::new(-x, -y, -z, -w)
        .unwrap()
        .normalized()
        .unwrap();
    assert_vector3_close(
        start_f32
            .slerp(equivalent_negative_f32, 0.5)
            .unwrap()
            .rotate_vector(Vector3::X)
            .unwrap(),
        midpoint_f32.rotate_vector(Vector3::X).unwrap(),
    );
    let norm_f32 = midpoint_f32
        .components()
        .into_iter()
        .map(|component| component * component)
        .sum::<f32>()
        .sqrt();
    numerical::assert_approx_eq_f32(norm_f32, 1.0, ApproxTolerance::new(2.0e-5, 2.0e-5).unwrap());
    assert!(start_f32.slerp(end_f32, f32::INFINITY).is_err());
}

#[test]
fn f32_rotation_and_transform_contracts_are_deliberate() {
    let raw_f64 = Quaterniond::new(1.0, -2.0, 3.0, -4.0).unwrap();
    let raw_f32 = raw_f64.to_f32_checked().unwrap();
    assert_eq!(raw_f32.components(), [1.0, -2.0, 3.0, -4.0]);
    assert_eq!(raw_f32.to_f64().components(), raw_f64.components());
    assert!(Quaterniond::new(f64::MAX, 0.0, 0.0, 1.0)
        .unwrap()
        .to_f32_checked()
        .is_err());
    assert_eq!(
        Quaternion::new(1.0, 2.0, 3.0, 4.0)
            .unwrap()
            .to_f64()
            .components(),
        [1.0, 2.0, 3.0, 4.0]
    );

    let quarter_turn =
        UnitQuaternion::from_axis_angle(Vector3::Z, std::f32::consts::FRAC_PI_2).unwrap();
    let rotated = quarter_turn.rotate_vector(Vector3::X).unwrap();
    let tolerance = ApproxTolerance::new(2.0e-5, 2.0e-5).unwrap();
    numerical::assert_approx_eq_f32(rotated.x(), 0.0, tolerance);
    numerical::assert_approx_eq_f32(rotated.y(), 1.0, tolerance);
    numerical::assert_approx_eq_f32(rotated.z(), 0.0, tolerance);

    let transform =
        math_geometry_3d::RigidTransform3::new(quarter_turn, Vector3::new(2.0, 0.0, 0.0).unwrap())
            .unwrap();
    let point = math_geometry_3d::Point3::new(1.0, 0.0, 0.0).unwrap();
    let recovered = transform
        .inverse()
        .unwrap()
        .apply_point(transform.apply_point(point).unwrap())
        .unwrap();
    numerical::assert_approx_eq_f32(recovered.x(), point.x(), tolerance);
    numerical::assert_approx_eq_f32(recovered.y(), point.y(), tolerance);
    numerical::assert_approx_eq_f32(recovered.z(), point.z(), tolerance);
}

#[cfg(feature = "nalgebra-adapters")]
proptest! {
    #![proptest_config(deterministic_config())]

    #[test]
fn nalgebra_reference_agrees_on_generated_rotation(
        (axis, angle) in arbitrary_rotation(),
        x in -100.0_f64..=100.0_f64,
        y in -100.0_f64..=100.0_f64,
        z in -100.0_f64..=100.0_f64,
    ) {
        let quaternion = UnitQuaterniond::from_axis_angle(axis, angle).unwrap();
        let vector = Vector3d::new(x, y, z).unwrap();
        let expected = quaternion.to_nalgebra().transform_vector(&nalgebra::Vector3::new(x, y, z));
        assert_vector_close(quaternion.rotate_vector(vector).unwrap(), Vector3d::new(expected.x, expected.y, expected.z).unwrap());

        let translation = Vector3d::new(axis.x() * 3.0, axis.y() * -2.0, axis.z()).unwrap();
        let transform = RigidTransform3d::new(quaternion, translation).unwrap();
        let reference = nalgebra::Isometry3::from_parts(
            nalgebra::Translation3::new(translation.x(), translation.y(), translation.z()),
            quaternion.to_nalgebra(),
        );
        let point = Point3d::new(x, y, z).unwrap();
        let expected_point = reference.transform_point(&nalgebra::Point3::new(x, y, z));
        assert_point_close(
            transform.apply_point(point).unwrap(),
            Point3d::new(expected_point.x, expected_point.y, expected_point.z).unwrap(),
        );
        let expected_vector = reference.transform_vector(&nalgebra::Vector3::new(x, y, z));
        assert_vector_close(
            transform.apply_vector(vector).unwrap(),
            Vector3d::new(expected_vector.x, expected_vector.y, expected_vector.z).unwrap(),
        );

        let affine = AffineTransform3d::from_matrix4(Matrix4d::new([
            [1.0, 0.2, 0.0, translation.x()],
            [0.0, 1.5, -0.1, translation.y()],
            [0.0, 0.0, 0.75, translation.z()],
            [0.0, 0.0, 0.0, 1.0],
        ]).unwrap()).unwrap();
        let reference_linear = nalgebra::Matrix3::new(1.0, 0.2, 0.0, 0.0, 1.5, -0.1, 0.0, 0.0, 0.75);
        let expected_affine_vector = reference_linear * nalgebra::Vector3::new(x, y, z);
        assert_vector_close(
            affine.apply_vector(vector).unwrap(),
            Vector3d::new(expected_affine_vector.x, expected_affine_vector.y, expected_affine_vector.z).unwrap(),
        );
        let expected_affine_point = expected_affine_vector + nalgebra::Vector3::new(translation.x(), translation.y(), translation.z());
        assert_point_close(
            affine.apply_point(point).unwrap(),
            Point3d::new(expected_affine_point.x, expected_affine_point.y, expected_affine_point.z).unwrap(),
        );
    }
}
