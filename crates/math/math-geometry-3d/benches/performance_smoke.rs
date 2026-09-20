use iai_callgrind::{
    library_benchmark, library_benchmark_group, main, Callgrind, EventKind, LibraryBenchmarkConfig,
};
use math_geometry_3d::{Matrix3, Matrix3d, Quaterniond, UnitQuaternion, UnitQuaterniond, Vector3d};
use std::hint::black_box;

fn rotation(angle: f64) -> UnitQuaterniond {
    UnitQuaterniond::from_axis_angle(Vector3d::Z, angle).expect("finite benchmark rotation")
}

fn negated_rotation(angle: f64) -> UnitQuaterniond {
    let [x, y, z, w] = rotation(angle).components();
    Quaterniond::new(-x, -y, -z, -w)
        .expect("finite quaternion")
        .normalized()
        .expect("unit quaternion")
}

fn narrowed_rotation() -> Matrix3 {
    rotation(0.5)
        .to_matrix3()
        .expect("rotation matrix")
        .to_f32_checked()
        .expect("bounded rotation entries")
}

fn matrix(scale: f64) -> Matrix3d {
    Matrix3d::new(
        [[2.0, 1.0, 0.0], [0.0, 3.0, 1.0], [1.0, 0.0, 4.0]]
            .map(|row| row.map(|value| value * scale)),
    )
    .expect("finite benchmark matrix")
}

#[library_benchmark]
#[bench::rounded_f32(narrowed_rotation())]
fn rotation_import(matrix: Matrix3) -> UnitQuaternion {
    black_box(UnitQuaternion::from_matrix3(black_box(matrix)).expect("valid rounded rotation"))
}

#[library_benchmark]
#[bench::ordinary(matrix(1.0))]
#[bench::small(matrix(1e-200))]
#[bench::large(matrix(1e200))]
fn matrix_inverse(matrix: Matrix3d) -> Matrix3d {
    black_box(
        black_box(matrix)
            .inverse()
            .expect("well-conditioned matrix"),
    )
}

#[library_benchmark]
#[bench::ordinary(rotation(0.5))]
#[bench::tiny(rotation(1e-8))]
#[bench::tiny_negated(negated_rotation(1e-200))]
fn axis_angle(rotation: UnitQuaterniond) -> (Vector3d, f64) {
    black_box(
        black_box(rotation)
            .to_axis_angle()
            .expect("unit quaternion"),
    )
}

library_benchmark_group!(
    name = geometry_contracts_smoke;
    benchmarks = rotation_import, matrix_inverse, axis_angle
);

fn benchmark_config() -> LibraryBenchmarkConfig {
    let mut callgrind = Callgrind::default();
    callgrind
        .soft_limits([(EventKind::Ir, 5.0)])
        .fail_fast(true);
    let mut config = LibraryBenchmarkConfig::default();
    config.tool(callgrind);
    config
}

main!(
    config = benchmark_config();
    library_benchmark_groups = geometry_contracts_smoke
);
