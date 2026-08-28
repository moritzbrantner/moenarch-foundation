#[path = "../../../test-support/numerical.rs"]
mod numerical;

use math_geometry_3d::{
    AffineTransform3d, EulerOrder, Matrix3d, Point3d, Quaterniond, RigidTransform3d,
    UnitQuaternion, UnitQuaterniond, Vector3, Vector3d,
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
        vx in -100.0_f64..=100.0_f64,
        vy in -100.0_f64..=100.0_f64,
        vz in -100.0_f64..=100.0_f64,
    ) {
        let quaternion = UnitQuaterniond::from_axis_angle(axis, angle).unwrap();
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

    let serialized = serde_json::to_string(&quarter_turn).unwrap();
    assert!(serialized.starts_with(r#"{"x":"#), "{serialized}");
    assert!(
        serialized.contains(",\"y\":")
            && serialized.contains(",\"z\":")
            && serialized.contains(",\"w\":")
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
    assert!(Vector3d::new(f64::MAX, 0.0, 0.0)
        .unwrap()
        .to_f32_checked()
        .is_err());

    for angle in [1.0e-14, std::f64::consts::PI - 1.0e-12] {
        let source = UnitQuaterniond::from_axis_angle(Vector3d::Y, angle).unwrap();
        let (axis, recovered_angle) = source.to_axis_angle().unwrap();
        let recovered = UnitQuaterniond::from_axis_angle(axis, recovered_angle).unwrap();
        assert!(quaternion_dot(source, recovered).abs() > 1.0 - 1.0e-10);
    }

    for order in [
        EulerOrder::Xyz,
        EulerOrder::Xzy,
        EulerOrder::Yxz,
        EulerOrder::Yzx,
        EulerOrder::Zxy,
        EulerOrder::Zyx,
    ] {
        let source =
            UnitQuaterniond::from_euler(order, 0.2, std::f64::consts::FRAC_PI_2, -0.4).unwrap();
        let (x, y, z) = source.to_euler(order).unwrap();
        let recovered = UnitQuaterniond::from_euler(order, x, y, z).unwrap();
        assert!(
            quaternion_dot(source, recovered).abs() > 1.0 - 1.0e-10,
            "{order:?}"
        );
    }
}

#[test]
fn f32_rotation_and_transform_contracts_are_deliberate() {
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
    }
}
