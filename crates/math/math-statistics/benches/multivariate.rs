use criterion::{black_box, criterion_group, criterion_main, Criterion};
use math_linear::{F32Matrix, MatrixShape};
use math_statistics::{
    ordinary_least_squares, PrincipalComponents, RunningCovariance, WeightedObservation,
    ZScoreNormalizer,
};

fn matrix(rows: usize, cols: usize) -> F32Matrix {
    F32Matrix::new(
        MatrixShape::new(rows, cols).unwrap(),
        (0..rows * cols)
            .map(|index| {
                let row = index / cols;
                let col = index % cols;
                (row as f32 * 0.013 + col as f32 * 0.071).sin()
                    + (row as f32 * col as f32 * 0.0001).cos() * 0.2
            })
            .collect(),
    )
    .unwrap()
}

fn bench_multivariate(c: &mut Criterion) {
    let matrix = matrix(2_048, 16);
    let view = matrix.as_view();

    c.bench_function("running_covariance_2048x16", |b| {
        b.iter(|| {
            let mut covariance = RunningCovariance::new(16).unwrap();
            for row in 0..view.shape().rows {
                let values = view
                    .row(row)
                    .unwrap()
                    .as_slice()
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>();
                covariance
                    .push(WeightedObservation::new(black_box(values)).unwrap())
                    .unwrap();
            }
            covariance.covariance_matrix().unwrap()
        })
    });

    c.bench_function("z_score_fit_transform_2048x16", |b| {
        b.iter(|| {
            let normalizer = ZScoreNormalizer::fit(black_box(&view)).unwrap();
            normalizer.transform_matrix(black_box(&view)).unwrap()
        })
    });

    c.bench_function("pca_fit_transform_2048x16_to_4", |b| {
        b.iter(|| {
            let pca = PrincipalComponents::fit(black_box(&view), black_box(4)).unwrap();
            pca.transform(black_box(&view)).unwrap()
        })
    });

    let target = (0..matrix.shape().rows)
        .map(|index| (index as f32 * 0.011).sin())
        .collect::<Vec<_>>();
    c.bench_function("ols_2048x16", |b| {
        b.iter(|| ordinary_least_squares(black_box(&view), black_box(&target)).unwrap())
    });
}

criterion_group!(benches, bench_multivariate);
criterion_main!(benches);
