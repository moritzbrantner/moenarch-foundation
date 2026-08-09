use dense_data::{dense_summary, BucketGrid, DenseDataset, DensePoint, KMeansConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = DenseDataset::from_points([
        DensePoint::new([0.0, 0.0])?.named("left"),
        DensePoint::new([0.5, 0.2])?.named("left-near"),
        DensePoint::new([4.0, 4.5])?
            .named("right")
            .weighted(2.0)?
            .valued(7.0)?,
    ])?;

    let summary = dataset.summary()?;
    println!("dataset mean x = {:?}", summary.coordinate_stats[0].mean);

    let buckets = dataset.buckets(&BucketGrid::uniform(2, 1.0)?)?;
    println!("buckets = {}", buckets.len());

    let clusters = dataset.k_means(KMeansConfig::new(2)?)?;
    println!("clusters = {}", clusters.clusters.len());

    let value_summary = dense_summary(dataset.points())?
        .value_stats
        .expect("value stats exist");
    println!("weighted value mean = {:?}", value_summary.mean);

    Ok(())
}
