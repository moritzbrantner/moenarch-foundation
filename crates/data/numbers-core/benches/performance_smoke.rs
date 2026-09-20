use iai_callgrind::{
    library_benchmark, library_benchmark_group, main, Callgrind, EventKind, LibraryBenchmarkConfig,
};
use numbers_core::{ApproxTolerance, NumberRange};
use std::hint::black_box;

#[library_benchmark]
#[bench::ordinary(10_000.0, 10_005.0, 0.001)]
#[bench::finite_extremes(f64::MAX, -f64::MAX, 2.0)]
fn relative_comparison(left: f64, right: f64, relative: f64) -> bool {
    let tolerance = ApproxTolerance::new(0.0, black_box(relative)).expect("finite tolerance");
    black_box(tolerance.allows_f64(black_box(left), black_box(right)))
}

#[library_benchmark]
#[bench::valid(NumberRange { min: -2.0, max: 2.0 })]
#[bench::reversed(NumberRange { min: 2.0, max: 1.0 })]
fn validated_clamp(range: NumberRange) -> bool {
    black_box(black_box(range).clamp(black_box(0.5)).is_ok())
}

#[library_benchmark]
#[bench::valid(r#"{"min":-2.0,"max":2.0}"#)]
#[bench::reversed(r#"{"min":2.0,"max":1.0}"#)]
fn deserialize_range(input: &str) -> bool {
    black_box(serde_json::from_str::<NumberRange>(black_box(input)).is_ok())
}

library_benchmark_group!(
    name = numerical_contracts_smoke;
    benchmarks = relative_comparison, validated_clamp, deserialize_range
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
    library_benchmark_groups = numerical_contracts_smoke
);
