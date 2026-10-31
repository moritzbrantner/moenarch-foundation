//! Deterministic weighted k-means clustering.

use media_core::Result;
use serde::{Deserialize, Serialize};

use crate::summary::SummaryAccumulator;
use crate::validation::{invalid_argument, validate_point_set};
use crate::{DenseAverages, DenseBounds, DensePoint};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
/// Configuration for deterministic weighted k-means clustering.
pub struct KMeansConfig {
    /// Number of clusters to fit.
    pub clusters: usize,
    /// Maximum number of centroid update iterations.
    pub max_iterations: usize,
    /// Non-negative movement tolerance used for convergence.
    pub tolerance: f64,
}

impl KMeansConfig {
    /// Creates a new value.
    pub fn new(clusters: usize) -> Result<Self> {
        let config = Self {
            clusters,
            ..Self::default()
        };
        config.validate()?;
        Ok(config)
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if self.clusters == 0 {
            return Err(invalid_argument("cluster count must be positive"));
        }
        if self.max_iterations == 0 {
            return Err(invalid_argument("max iterations must be positive"));
        }
        if self.tolerance < 0.0 || !self.tolerance.is_finite() {
            return Err(invalid_argument(
                "cluster tolerance must be finite and non-negative",
            ));
        }
        Ok(())
    }
}

impl Default for KMeansConfig {
    fn default() -> Self {
        Self {
            clusters: 8,
            max_iterations: 100,
            tolerance: 0.0001,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// One deterministic weighted k-means cluster.
pub struct DenseCluster {
    /// Stable cluster index.
    pub cluster_index: usize,
    /// Current centroid coordinates.
    pub centroid: Vec<f64>,
    /// Number of items represented by this value.
    pub count: u64,
    /// Sum of assigned point weights.
    pub weight_sum: f64,
    /// Weighted averages for assigned points.
    pub averages: Option<DenseAverages>,
    /// Coordinate bounds for assigned points.
    pub bounds: Option<DenseBounds>,
    /// Source point indices assigned to this cluster.
    pub point_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Result of deterministic weighted k-means clustering.
pub struct ClusterResult {
    /// Number of iterations performed before convergence or limit.
    pub iterations: usize,
    /// Clusters in stable index order.
    pub clusters: Vec<DenseCluster>,
}

/// Clusters dense points with deterministic k-means initialization.
pub fn k_means(points: &[DensePoint], config: KMeansConfig) -> Result<ClusterResult> {
    config.validate()?;
    let dimensions = validate_point_set(points)?;
    if config.clusters > points.len() {
        return Err(invalid_argument(
            "cluster count must not exceed the number of points",
        ));
    }

    let mut centroids = initial_centroids(points, config.clusters);
    let mut assignments = vec![usize::MAX; points.len()];
    let mut iterations = 0;

    for iteration in 1..=config.max_iterations {
        iterations = iteration;
        let changed = assign_points(points, &centroids, &mut assignments);
        let next_centroids = recompute_centroids(points, &assignments, &centroids, dimensions);
        let shift = centroid_shift(&centroids, &next_centroids);
        centroids = next_centroids;
        if !changed || shift <= config.tolerance {
            break;
        }
    }

    let mut accumulators = centroids
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, centroid)| ClusterAccumulator::new(index, centroid))
        .collect::<Vec<_>>();
    for (point_index, point) in points.iter().enumerate() {
        accumulators[assignments[point_index]].push(point_index, point);
    }

    Ok(ClusterResult {
        iterations,
        clusters: accumulators
            .into_iter()
            .map(ClusterAccumulator::finish)
            .collect(),
    })
}

#[derive(Debug, Clone)]
struct ClusterAccumulator {
    cluster_index: usize,
    centroid: Vec<f64>,
    count: u64,
    point_indices: Vec<usize>,
    summary: SummaryAccumulator,
}

impl ClusterAccumulator {
    fn new(cluster_index: usize, centroid: Vec<f64>) -> Self {
        Self {
            cluster_index,
            centroid,
            count: 0,
            point_indices: Vec::new(),
            summary: SummaryAccumulator::new(),
        }
    }

    fn push(&mut self, point_index: usize, point: &DensePoint) {
        self.count += 1;
        self.point_indices.push(point_index);
        self.summary.push(point);
    }

    fn finish(self) -> DenseCluster {
        DenseCluster {
            cluster_index: self.cluster_index,
            centroid: self.centroid,
            count: self.count,
            weight_sum: self.summary.weight_sum,
            averages: self.summary.averages().ok(),
            bounds: self.summary.bounds().ok(),
            point_indices: self.point_indices,
        }
    }
}

fn initial_centroids(points: &[DensePoint], clusters: usize) -> Vec<Vec<f64>> {
    if clusters == 1 {
        return vec![points[0].coordinates.clone()];
    }
    let last_index = points.len() - 1;
    (0..clusters)
        .map(|cluster| {
            let index = (cluster * last_index + (clusters - 1) / 2) / (clusters - 1);
            points[index].coordinates.clone()
        })
        .collect()
}

fn assign_points(points: &[DensePoint], centroids: &[Vec<f64>], assignments: &mut [usize]) -> bool {
    let mut changed = false;
    for (point_index, point) in points.iter().enumerate() {
        let mut best_cluster = 0;
        let mut best_distance = f64::INFINITY;
        for (cluster_index, centroid) in centroids.iter().enumerate() {
            let distance = squared_distance(&point.coordinates, centroid);
            if distance < best_distance {
                best_distance = distance;
                best_cluster = cluster_index;
            }
        }
        if assignments[point_index] != best_cluster {
            assignments[point_index] = best_cluster;
            changed = true;
        }
    }
    changed
}

fn recompute_centroids(
    points: &[DensePoint],
    assignments: &[usize],
    centroids: &[Vec<f64>],
    dimensions: usize,
) -> Vec<Vec<f64>> {
    let mut next = vec![vec![0.0; dimensions]; centroids.len()];
    let mut weights = vec![0.0; centroids.len()];

    for (point, cluster) in points.iter().zip(assignments) {
        weights[*cluster] += point.weight;
        for (value, coordinate) in next[*cluster].iter_mut().zip(&point.coordinates) {
            *value += point.weight * coordinate;
        }
    }

    for (cluster, centroid) in next.iter_mut().enumerate() {
        if weights[cluster] == 0.0 {
            *centroid = centroids[cluster].clone();
            continue;
        }
        for value in centroid {
            *value /= weights[cluster];
        }
    }
    next
}

fn centroid_shift(left: &[Vec<f64>], right: &[Vec<f64>]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| squared_distance(left, right).sqrt())
        .fold(0.0, f64::max)
}

fn squared_distance(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let delta = left - right;
            delta * delta
        })
        .sum()
}
