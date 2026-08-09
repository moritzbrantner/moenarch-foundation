use criterion::{black_box, criterion_group, criterion_main, Criterion};
use math_linear::{F32Matrix, F64Matrix, MatrixShape, SvdOptions};

fn matrix(rows: usize, cols: usize, seed: f32) -> F32Matrix {
    F32Matrix::new(
        MatrixShape::new(rows, cols).unwrap(),
        (0..rows * cols)
            .map(|index| {
                let value = index as f32 * 0.017 + seed;
                value.sin() * 0.5 + value.cos() * 0.25
            })
            .collect(),
    )
    .unwrap()
}

fn matrix_f64(rows: usize, cols: usize, seed: f64) -> F64Matrix {
    F64Matrix::new(
        MatrixShape::new(rows, cols).unwrap(),
        (0..rows * cols)
            .map(|index| {
                let value = index as f64 * 0.017 + seed;
                value.sin() * 0.5 + value.cos() * 0.25
            })
            .collect(),
    )
    .unwrap()
}

fn bench_matrix(c: &mut Criterion) {
    let left = matrix(128, 128, 0.1);
    let right = matrix(128, 128, 1.3);
    let small = F32Matrix::from_rows([
        [4.0, 7.0, 2.0, 3.0],
        [0.0, 5.0, 1.0, 2.0],
        [2.0, 1.0, 6.0, 1.0],
        [1.0, 0.0, 2.0, 4.0],
    ])
    .unwrap();
    let tall = matrix(512, 128, 2.1);
    let query = matrix(256, 128, 0.7);
    let svd_input = matrix_f64(96, 48, 3.7);
    let vector = (0..128)
        .map(|index| (index as f32 * 0.023).sin())
        .collect::<Vec<_>>();
    let small_vector = vec![1.0, 2.0, 3.0, 4.0];

    c.bench_function("matrix_matmul_128x128", |b| {
        b.iter(|| left.matmul(black_box(&right.as_view())).unwrap())
    });

    c.bench_function("matrix_transpose_owned_128x128", |b| {
        b.iter(|| left.as_view().transpose_owned().unwrap())
    });

    c.bench_function("matrix_matvec_512x128", |b| {
        b.iter(|| tall.matvec(black_box(&vector)).unwrap())
    });

    c.bench_function("matrix_lu_decompose_4x4", |b| {
        b.iter(|| small.as_view().lu_decompose().unwrap())
    });

    c.bench_function("matrix_solve_vector_4x4", |b| {
        b.iter(|| small.solve_vector(black_box(&small_vector)).unwrap())
    });

    c.bench_function("matrix_inverse_4x4", |b| {
        b.iter(|| small.inverse().unwrap())
    });

    c.bench_function("matrix_cholesky_4x4", |b| {
        b.iter(|| {
            F32Matrix::from_rows([
                [6.0, 1.0, 1.0, 1.0],
                [1.0, 5.0, 1.0, 1.0],
                [1.0, 1.0, 4.0, 1.0],
                [1.0, 1.0, 1.0, 3.0],
            ])
            .unwrap()
            .cholesky_decompose()
            .unwrap()
        })
    });

    c.bench_function("matrix_qr_512x128", |b| {
        b.iter(|| tall.qr_decompose().unwrap())
    });

    c.bench_function("matrix_l2_normalize_rows_512x128", |b| {
        b.iter(|| tall.l2_normalize_rows().unwrap())
    });

    c.bench_function("matrix_pairwise_row_cosine_512x256x128", |b| {
        b.iter(|| {
            tall.pairwise_row_cosine(black_box(&query.as_view()))
                .unwrap()
        })
    });

    c.bench_function("matrix_svd_pure_rust_96x48", |b| {
        b.iter(|| svd_input.svd(black_box(SvdOptions::default())).unwrap())
    });
}

criterion_group!(benches, bench_matrix);
criterion_main!(benches);
