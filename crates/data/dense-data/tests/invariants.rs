use dense_data::{BucketGrid, DenseDataset, DensePoint, KMeansConfig};

fn sample_points(count: usize) -> Vec<DensePoint> {
    (0..count)
        .map(|index| {
            DensePoint::new([
                (index % 8) as f64 * 0.5 - 2.0,
                (index / 8) as f64 * 0.25 - 1.0,
            ])
            .unwrap()
            .weighted(1.0 + (index % 4) as f64)
            .unwrap()
            .valued((index % 7) as f64 - 3.0)
            .unwrap()
        })
        .collect()
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-9,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn weighted_averages_match_a_direct_reference_across_dataset_sizes() {
    for count in 1..=64 {
        let points = sample_points(count);
        let dataset = DenseDataset::from_points(points.clone()).unwrap();
        let averages = dataset.averages().unwrap();

        let weight_sum = points.iter().map(|point| point.weight).sum::<f64>();
        let expected_coordinates = (0..2)
            .map(|dimension| {
                points
                    .iter()
                    .map(|point| point.coordinates[dimension] * point.weight)
                    .sum::<f64>()
                    / weight_sum
            })
            .collect::<Vec<_>>();
        let expected_value = points
            .iter()
            .map(|point| point.value.unwrap() * point.weight)
            .sum::<f64>()
            / weight_sum;

        assert_eq!(averages.count, count as u64);
        assert_close(averages.weight_sum, weight_sum);
        for (actual, expected) in averages.coordinates.iter().zip(expected_coordinates) {
            assert_close(*actual, expected);
        }
        assert_eq!(averages.value_count, count as u64);
        assert_close(averages.value_weight_sum, weight_sum);
        assert_close(averages.value.unwrap(), expected_value);
    }
}

#[test]
fn bucketing_is_a_lossless_partition_of_source_points() {
    for count in 1..=96 {
        let points = sample_points(count);
        let dataset = DenseDataset::from_points(points.clone()).unwrap();
        let buckets = dataset.buckets(&BucketGrid::uniform(2, 1.0).unwrap()).unwrap();

        let mut indices = buckets
            .iter()
            .flat_map(|bucket| bucket.point_indices.iter().copied())
            .collect::<Vec<_>>();
        indices.sort_unstable();
        assert_eq!(indices, (0..count).collect::<Vec<_>>());
        assert_eq!(
            buckets.iter().map(|bucket| bucket.count).sum::<u64>(),
            count as u64
        );
        assert_close(
            buckets.iter().map(|bucket| bucket.weight_sum).sum::<f64>(),
            points.iter().map(|point| point.weight).sum::<f64>(),
        );
    }
}

#[test]
fn k_means_is_deterministic_and_partitions_every_input_exactly_once() {
    for count in [4, 8, 16, 32, 64] {
        let points = sample_points(count);
        let dataset = DenseDataset::from_points(points.clone()).unwrap();
        let config = KMeansConfig {
            clusters: 4,
            max_iterations: 64,
            tolerance: 1.0e-9,
        };

        let first = dataset.k_means(config).unwrap();
        let second = dataset.k_means(config).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.clusters.len(), 4);
        assert!((1..=config.max_iterations).contains(&first.iterations));

        let mut indices = first
            .clusters
            .iter()
            .flat_map(|cluster| cluster.point_indices.iter().copied())
            .collect::<Vec<_>>();
        indices.sort_unstable();
        assert_eq!(indices, (0..count).collect::<Vec<_>>());

        for cluster in &first.clusters {
            assert_eq!(cluster.count as usize, cluster.point_indices.len());
            assert_eq!(cluster.centroid.len(), 2);
            assert!(cluster.centroid.iter().all(|value| value.is_finite()));
        }
        assert_close(
            first.clusters.iter().map(|cluster| cluster.weight_sum).sum(),
            points.iter().map(|point| point.weight).sum(),
        );
    }
}
