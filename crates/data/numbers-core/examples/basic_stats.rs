use numbers_core::{histogram, quartiles, summarize_numbers, HistogramConfig, RunningStats};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stats = RunningStats::new();
    stats.push(2.0);
    stats.push_weighted(4.0, 2.0)?;
    stats.push(f64::NAN);

    let summary = stats.summary();
    println!("count={} mean={:?}", summary.count, summary.mean);

    let quartiles = quartiles(&[1.0, 2.0, 3.0, 4.0, 5.0])?;
    println!("median={}", quartiles.median);

    let histogram = histogram(&[1.0, 2.0, 3.0, 4.0], HistogramConfig::new(3)?)?;
    println!("bins={}", histogram.bins.len());

    let summary_from_slice = summarize_numbers(&[1.0, 2.0, 3.0, f64::INFINITY]);
    println!("finite={}", summary_from_slice.finite_count);

    Ok(())
}
