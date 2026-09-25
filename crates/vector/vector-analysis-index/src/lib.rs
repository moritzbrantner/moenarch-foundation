#![doc = include_str!("../README.md")]

pub mod approximate;
pub mod surface;
use std::collections::BTreeMap;
use std::fmt;

use media_core::{DetectError, Result};
use search_kernels::top_k_by;
use serde::{Deserialize, Serialize};
use vector_analysis_core::{metric_distance, DenseVector, VectorMetric};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
/// Data type for vector record identifier.
pub struct VectorRecordId(String);

impl VectorRecordId {
    /// Creates a new value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrows this value as a str.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes this value into a string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for VectorRecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for VectorRecordId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<String> for VectorRecordId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for VectorRecordId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Data type for vector record metadata.
pub struct VectorRecordMetadata {
    /// The tags value.
    pub tags: Vec<String>,
    /// Metadata associated with this value.
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for vector record.
pub struct VectorRecord {
    /// Identifier for this value.
    pub id: String,
    /// The vector value.
    pub vector: DenseVector,
    /// The payload value.
    pub payload: VectorRecordMetadata,
}

impl VectorRecord {
    /// Creates a new value.
    pub fn new(id: impl Into<VectorRecordId>, vector: DenseVector) -> Self {
        Self::with_payload(id, vector, VectorRecordMetadata::default())
    }

    /// Returns this value with payload.
    pub fn with_payload(
        id: impl Into<VectorRecordId>,
        vector: DenseVector,
        payload: VectorRecordMetadata,
    ) -> Self {
        let id = id.into();
        Self {
            id: id.into_string(),
            vector,
            payload,
        }
    }

    /// Returns record identifier.
    pub fn record_id(&self) -> VectorRecordId {
        VectorRecordId::from(self.id.clone())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Data type for vector search filter.
pub struct VectorSearchFilter {
    /// The required tags value.
    pub required_tags: Vec<String>,
    /// The metadata equals value.
    pub metadata_equals: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Data type for search config.
pub struct SearchConfig {
    /// The metric value.
    pub metric: VectorMetric,
    /// The limit value.
    pub limit: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            metric: VectorMetric::Cosine,
            limit: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for search result.
pub struct SearchResult {
    /// Identifier for this value.
    pub id: String,
    /// The distance value.
    pub distance: f32,
    /// Score assigned to this value.
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for vector hit.
pub struct VectorHit {
    /// Identifier for this value.
    pub id: VectorRecordId,
    /// The distance value.
    pub distance: f32,
    /// Score assigned to this value.
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Data type for serializable vector record.
pub struct SerializableVectorRecord {
    /// Identifier for this value.
    pub id: VectorRecordId,
    /// The vector value.
    pub vector: Vec<f32>,
    /// The payload value.
    pub payload: VectorRecordMetadata,
}

#[derive(Debug, Clone, Default, PartialEq)]
/// Data type for vector search index.
pub struct VectorSearchIndex {
    dimensions: Option<usize>,
    records: Vec<VectorRecord>,
}

impl VectorSearchIndex {
    /// Creates a new value.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns dimensions.
    pub fn dimensions(&self) -> Option<usize> {
        self.dimensions
    }

    /// Returns records.
    pub fn records(&self) -> &[VectorRecord] {
        &self.records
    }

    /// Returns clear.
    pub fn clear(&mut self) {
        self.dimensions = None;
        self.records.clear();
    }

    /// Returns add.
    pub fn add(&mut self, record: VectorRecord) -> Result<()> {
        record.vector.validate()?;
        if record.id.trim().is_empty() {
            return Err(invalid_argument("record id must not be empty"));
        }
        match self.dimensions {
            Some(dimensions) if dimensions != record.vector.dimensions() => {
                return Err(invalid_argument(
                    "indexed vectors must have the same dimensions",
                ));
            }
            None => self.dimensions = Some(record.vector.dimensions()),
            _ => {}
        }
        self.records.push(record);
        Ok(())
    }

    /// Returns extend.
    pub fn extend(&mut self, records: impl IntoIterator<Item = VectorRecord>) -> Result<()> {
        for record in records {
            self.add(record)?;
        }
        Ok(())
    }

    /// Builds this value from records.
    pub fn from_records(records: impl IntoIterator<Item = VectorRecord>) -> Result<Self> {
        let mut index = Self::new();
        index.extend(records)?;
        Ok(index)
    }

    /// Returns export records.
    pub fn export_records(&self) -> Vec<SerializableVectorRecord> {
        self.records
            .iter()
            .map(|record| SerializableVectorRecord {
                id: record.record_id(),
                vector: record.vector.as_slice().to_vec(),
                payload: record.payload.clone(),
            })
            .collect()
    }

    /// Returns import records.
    pub fn import_records(
        records: impl IntoIterator<Item = SerializableVectorRecord>,
    ) -> Result<Self> {
        let mut index = Self::new();
        for record in records {
            index.add(VectorRecord::with_payload(
                record.id,
                DenseVector::new(record.vector)?,
                record.payload,
            ))?;
        }
        Ok(index)
    }

    /// Returns search.
    pub fn search(&self, query: &DenseVector, config: SearchConfig) -> Result<Vec<SearchResult>> {
        if config.limit == 0 {
            return Err(invalid_argument("search limit must be greater than zero"));
        }
        if let Some(dimensions) = self.dimensions {
            if query.dimensions() != dimensions {
                return Err(invalid_argument("query dimensions must match the index"));
            }
        }
        top_k_fallible(
            self.records.iter().map(|record| {
                let distance =
                    metric_distance(config.metric, query.as_slice(), record.vector.as_slice())?;
                Ok(SearchResult {
                    id: record.id.clone(),
                    distance,
                    score: score_from_distance(config.metric, distance),
                })
            }),
            config.limit,
            |left, right| {
                left.distance
                    .total_cmp(&right.distance)
                    .then_with(|| left.id.cmp(&right.id))
            },
        )
    }

    /// Returns search filtered.
    pub fn search_filtered(
        &self,
        query: &[f32],
        top_k: usize,
        filter: Option<&VectorSearchFilter>,
    ) -> Result<Vec<VectorHit>> {
        if top_k == 0 {
            return Err(invalid_argument("search limit must be greater than zero"));
        }
        validate_query_slice(query)?;
        if let Some(dimensions) = self.dimensions {
            if query.len() != dimensions {
                return Err(invalid_argument("query dimensions must match the index"));
            }
        }

        top_k_fallible(
            self.records.iter().filter_map(|record| {
                if filter.is_some_and(|filter| !matches_filter(&record.payload, filter)) {
                    return None;
                }
                Some((|| {
                    let distance =
                        metric_distance(VectorMetric::Cosine, query, record.vector.as_slice())?;
                    Ok(VectorHit {
                        id: record.record_id(),
                        distance,
                        score: score_from_distance(VectorMetric::Cosine, distance),
                    })
                })())
            }),
            top_k,
            |left, right| {
                left.distance
                    .total_cmp(&right.distance)
                    .then_with(|| left.id.cmp(&right.id))
            },
        )
    }
}

/// Returns assign nearest centroids.
pub fn assign_nearest_centroids(
    vectors: &[DenseVector],
    centroids: &[DenseVector],
    metric: VectorMetric,
) -> Result<Vec<usize>> {
    if centroids.is_empty() {
        return Err(invalid_argument("centroids must not be empty"));
    }
    let mut assignments = Vec::with_capacity(vectors.len());
    for vector in vectors {
        let mut best_index = 0;
        let mut best_distance = f32::INFINITY;
        for (index, centroid) in centroids.iter().enumerate() {
            let distance = metric_distance(metric, vector.as_slice(), centroid.as_slice())?;
            if distance < best_distance {
                best_distance = distance;
                best_index = index;
            }
        }
        assignments.push(best_index);
    }
    Ok(assignments)
}

fn top_k_fallible<T>(
    values: impl IntoIterator<Item = Result<T>>,
    limit: usize,
    mut compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
) -> Result<Vec<T>> {
    top_k_by(values, limit, |left, right| match (left, right) {
        (Ok(left), Ok(right)) => compare(left, right),
        (Err(_), Err(_)) => std::cmp::Ordering::Equal,
        (Err(_), Ok(_)) => std::cmp::Ordering::Less,
        (Ok(_), Err(_)) => std::cmp::Ordering::Greater,
    })
    .into_iter()
    .collect()
}

fn score_from_distance(metric: VectorMetric, distance: f32) -> f32 {
    match metric {
        VectorMetric::Cosine => 1.0 - distance,
        VectorMetric::Dot => -distance,
        VectorMetric::Euclidean | VectorMetric::Manhattan => 1.0 / (1.0 + distance),
    }
}

fn matches_filter(payload: &VectorRecordMetadata, filter: &VectorSearchFilter) -> bool {
    filter
        .required_tags
        .iter()
        .all(|tag| payload.tags.iter().any(|candidate| candidate == tag))
        && filter
            .metadata_equals
            .iter()
            .all(|(key, value)| payload.metadata.get(key) == Some(value))
}

fn validate_query_slice(query: &[f32]) -> Result<()> {
    if query.is_empty() {
        return Err(invalid_argument("query vector must not be empty"));
    }
    if query.iter().any(|value| !value.is_finite()) {
        return Err(invalid_argument("query vector components must be finite"));
    }
    Ok(())
}

fn invalid_argument(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_nearest_vector() {
        let mut index = VectorSearchIndex::new();
        index
            .add(VectorRecord::new(
                "x",
                DenseVector::new([1.0, 0.0]).unwrap(),
            ))
            .unwrap();
        index
            .add(VectorRecord::new(
                "y",
                DenseVector::new([0.0, 1.0]).unwrap(),
            ))
            .unwrap();
        let results = index
            .search(
                &DenseVector::new([0.9, 0.1]).unwrap(),
                SearchConfig::default(),
            )
            .unwrap();
        assert_eq!(results[0].id, "x");
    }

    #[test]
    fn bounded_search_preserves_id_tie_breaking() {
        let index = VectorSearchIndex::from_records([
            VectorRecord::new("z", DenseVector::new([1.0, 0.0]).unwrap()),
            VectorRecord::new("a", DenseVector::new([1.0, 0.0]).unwrap()),
        ])
        .unwrap();
        let query = DenseVector::new([1.0, 0.0]).unwrap();

        let exact = index
            .search(
                &query,
                SearchConfig {
                    metric: VectorMetric::Cosine,
                    limit: 1,
                },
            )
            .unwrap();
        let filtered = index.search_filtered(query.as_slice(), 1, None).unwrap();

        assert_eq!(exact[0].id, "a");
        assert_eq!(filtered[0].id.as_str(), "a");
    }

    #[test]
    fn bounded_search_does_not_hide_late_metric_errors() {
        let index = VectorSearchIndex::from_records([
            VectorRecord::new("good", DenseVector::new([1.0, 0.0]).unwrap()),
            VectorRecord::new("zero", DenseVector::new([0.0, 0.0]).unwrap()),
        ])
        .unwrap();
        let query = DenseVector::new([1.0, 0.0]).unwrap();

        assert!(index
            .search(
                &query,
                SearchConfig {
                    metric: VectorMetric::Cosine,
                    limit: 1,
                },
            )
            .is_err());
        assert!(index.search_filtered(query.as_slice(), 1, None).is_err());
    }

    #[test]
    fn filters_search_results_by_tags_and_metadata() {
        let mut index = VectorSearchIndex::new();
        index
            .add(VectorRecord::with_payload(
                "alpha",
                DenseVector::new([1.0, 0.0]).unwrap(),
                VectorRecordMetadata {
                    tags: vec!["docs".to_string()],
                    metadata: BTreeMap::from([(String::from("lang"), String::from("en"))]),
                },
            ))
            .unwrap();
        index
            .add(VectorRecord::with_payload(
                "beta",
                DenseVector::new([1.0, 0.1]).unwrap(),
                VectorRecordMetadata {
                    tags: vec!["blog".to_string()],
                    metadata: BTreeMap::from([(String::from("lang"), String::from("de"))]),
                },
            ))
            .unwrap();

        let filter = VectorSearchFilter {
            required_tags: vec!["docs".to_string()],
            metadata_equals: BTreeMap::from([(String::from("lang"), String::from("en"))]),
        };
        let results = index
            .search_filtered(&[1.0, 0.0], 10, Some(&filter))
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id.as_str(), "alpha");
    }

    #[test]
    fn export_records_round_trip() {
        let mut index = VectorSearchIndex::new();
        index
            .add(VectorRecord::with_payload(
                "alpha",
                DenseVector::new([1.0, 0.0]).unwrap(),
                VectorRecordMetadata {
                    tags: vec!["docs".to_string()],
                    metadata: BTreeMap::from([(String::from("lang"), String::from("en"))]),
                },
            ))
            .unwrap();

        let exported = index.export_records();
        let imported = VectorSearchIndex::import_records(exported).unwrap();

        assert_eq!(imported.records(), index.records());
    }

    #[test]
    fn assigns_nearest_centroid() {
        let assignments = assign_nearest_centroids(
            &[DenseVector::new([9.0, 0.0]).unwrap()],
            &[
                DenseVector::new([0.0, 0.0]).unwrap(),
                DenseVector::new([10.0, 0.0]).unwrap(),
            ],
            VectorMetric::Euclidean,
        )
        .unwrap();
        assert_eq!(assignments, vec![1]);
    }
}
