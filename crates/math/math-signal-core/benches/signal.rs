use criterion::{black_box, criterion_group, criterion_main, Criterion};
use math_signal_core::{apply_fir_mono, signal_levels, FirKernel1d};

fn samples(count: usize) -> Vec<f32> {
    (0..count)
        .map(|index| (index as f32 * 0.013).sin() * 0.8)
        .collect()
}

fn bench_signal(c: &mut Criterion) {
    let samples = samples(48_000);
    let kernel = FirKernel1d::new([0.25, 0.5, 0.25]).unwrap();
    c.bench_function("signal_levels_48k", |b| {
        b.iter(|| signal_levels(black_box(&samples)).unwrap())
    });
    c.bench_function("fir_apply_48k_3tap", |b| {
        b.iter(|| apply_fir_mono(black_box(&samples), black_box(&kernel)).unwrap())
    });
}

criterion_group!(benches, bench_signal);
criterion_main!(benches);
