use criterion::{black_box, criterion_group, criterion_main, Criterion};
use math_geometry_2d::{
    broad_phase_pairs_2d, BroadPhase2Config, BroadPhase2Strategy, RectU32, SpatialCellSize2,
};

fn sparse_rects(count: usize) -> Vec<RectU32> {
    (0..count)
        .map(|index| RectU32::new((index * 10) as u32, (index * 7) as u32, 2, 2).unwrap())
        .collect()
}

fn clustered_rects(count: usize) -> Vec<RectU32> {
    (0..count)
        .map(|index| {
            let x = ((index % 256) * 3) as u32;
            let y = (((index / 256) % 256) * 3) as u32;
            RectU32::new(x, y, 8, 8).unwrap()
        })
        .collect()
}

fn overlapping_rects(count: usize) -> Vec<RectU32> {
    (0..count)
        .map(|index| RectU32::new((index % 512) as u32, 0, 4, 16).unwrap())
        .collect()
}

fn config(strategy: BroadPhase2Strategy) -> BroadPhase2Config {
    BroadPhase2Config {
        strategy,
        cell_size: SpatialCellSize2::Auto,
        ..BroadPhase2Config::default()
    }
}

fn bench_spatial(c: &mut Criterion) {
    for (name, make_rects) in [
        ("sparse", sparse_rects as fn(usize) -> Vec<RectU32>),
        ("clustered", clustered_rects),
        ("many_overlap", overlapping_rects),
    ] {
        for count in [1_000, 10_000, 50_000] {
            let rects = make_rects(count);
            c.bench_function(&format!("2d_{name}_auto_{count}"), |b| {
                b.iter(|| {
                    broad_phase_pairs_2d(
                        black_box(&rects),
                        black_box(config(BroadPhase2Strategy::Auto)),
                    )
                    .unwrap()
                })
            });
            c.bench_function(&format!("2d_{name}_sweep_{count}"), |b| {
                b.iter(|| {
                    broad_phase_pairs_2d(
                        black_box(&rects),
                        black_box(config(BroadPhase2Strategy::SweepAndPrune)),
                    )
                    .unwrap()
                })
            });
            c.bench_function(&format!("2d_{name}_grid_{count}"), |b| {
                b.iter(|| {
                    broad_phase_pairs_2d(
                        black_box(&rects),
                        black_box(config(BroadPhase2Strategy::SpatialHashGrid)),
                    )
                    .unwrap()
                })
            });
            if count == 1_000 {
                c.bench_function(&format!("2d_{name}_brute_{count}"), |b| {
                    b.iter(|| {
                        broad_phase_pairs_2d(
                            black_box(&rects),
                            black_box(config(BroadPhase2Strategy::BruteForce)),
                        )
                        .unwrap()
                    })
                });
            }
        }
    }
}

criterion_group!(benches, bench_spatial);
criterion_main!(benches);
