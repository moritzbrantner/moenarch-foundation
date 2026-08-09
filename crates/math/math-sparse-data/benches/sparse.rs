use criterion::{black_box, criterion_group, criterion_main, Criterion};
use math_sparse_data::{CooMatrix, SparseVector};

fn sparse_vector(dimensions: usize, non_zero: usize, offset: usize) -> SparseVector {
    let indices = (0..non_zero)
        .map(|index| (index * 37 + offset) % dimensions)
        .collect::<Vec<_>>();
    let values = (0..non_zero)
        .map(|index| {
            let value = index as f32 * 0.019 + offset as f32 * 0.003;
            value.sin() * 0.5 + 0.75
        })
        .collect::<Vec<_>>();
    SparseVector::new(dimensions, indices, values).unwrap()
}

fn coo(rows: usize, cols: usize, entries: usize) -> CooMatrix {
    CooMatrix::new(
        rows,
        cols,
        (0..entries)
            .map(|index| {
                let row = (index * 17) % rows;
                let col = (index * 31) % cols;
                let value = (index as f32 * 0.011).sin();
                (row, col, value)
            })
            .collect(),
    )
    .unwrap()
}

fn bench_sparse(c: &mut Criterion) {
    let left = sparse_vector(100_000, 20_000, 3);
    let right = sparse_vector(100_000, 20_000, 11);
    let coo = coo(4_096, 4_096, 100_000);

    c.bench_function("sparse_canonicalize_20k_nnz", |b| {
        b.iter(|| left.canonicalized().unwrap())
    });

    c.bench_function("sparse_dot_20k_nnz", |b| {
        b.iter(|| left.dot(black_box(&right)).unwrap())
    });

    c.bench_function("sparse_cosine_20k_nnz", |b| {
        b.iter(|| left.cosine_similarity(black_box(&right)).unwrap())
    });

    c.bench_function("coo_to_csr_100k_entries", |b| {
        b.iter(|| coo.to_csr().unwrap())
    });

    let csr = coo.to_csr().unwrap();
    let dense = vec![0.5; 4_096];
    c.bench_function("csr_matvec_100k_entries", |b| {
        b.iter(|| csr.mul_dense_vector(black_box(&dense)).unwrap())
    });

    c.bench_function("csr_transpose_100k_entries", |b| {
        b.iter(|| csr.transpose().unwrap())
    });
}

criterion_group!(benches, bench_sparse);
criterion_main!(benches);
