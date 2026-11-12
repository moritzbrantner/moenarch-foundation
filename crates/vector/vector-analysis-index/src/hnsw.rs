//! Optional cosine HNSW index backed by the external `hnsw_rs` crate.
//!
//! The adapter keeps `hnsw_rs` types private: callers build from Foundation
//! [`VectorRecord`]s and receive Foundation [`SearchResult`]s. The external
//! graph only selects candidates; every candidate is re-scored with the same
//! exact cosine similarity and `(distance, id)` ordering used by
//! [`super::VectorSearchIndex`], so ties and scores keep exact-path semantics
//! within the selected candidate set.
//!
//! Construction inserts records serially in iterator order. `hnsw_rs` draws
//! node levels from a fixed internal seed, so rebuilding from the same records
//! and configuration yields the same graph and the same results on the same
//! build and target. Parallel insertion is deliberately not exposed because its
//! graph depends on thread scheduling.

use std::fmt;

use hnsw_rs::prelude::{DistCosine, Hnsw};
use media_core::Result;
use vector_analysis_core::{cosine_similarity, l2_norm, DenseVector};

use super::{invalid_argument, top_k_fallible, SearchResult, VectorRecord};

/// `hnsw_rs` aborts the process above this connection count, so it is
/// validated here instead.
const MAX_CONNECTIONS_LIMIT: usize = 256;
/// `hnsw_rs` clamps layers to this bound.
const MAX_LAYERS_LIMIT: usize = 16;

/// Build-time configuration for [`CosineHnswIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CosineHnswConfig {
    /// Maximum neighbours stored per node and layer (`M`), at most 256.
    pub max_connections: usize,
    /// Candidate-list width while inserting (`efConstruction`).
    pub ef_construction: usize,
    /// Maximum graph layers, at most 16.
    pub max_layers: usize,
}

impl Default for CosineHnswConfig {
    fn default() -> Self {
        Self {
            max_connections: 16,
            ef_construction: 200,
            max_layers: 16,
        }
    }
}

/// Query-time effort configuration for [`CosineHnswIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CosineHnswSearchConfig {
    /// Maximum number of ranked results to return.
    pub limit: usize,
    /// Candidate-list width while searching (`efSearch`); must be at least `limit`.
    /// This is also the number of candidates re-scored exactly.
    pub ef_search: usize,
}

impl Default for CosineHnswSearchConfig {
    fn default() -> Self {
        Self {
            limit: 10,
            ef_search: 64,
        }
    }
}

/// Approximate HNSW cosine-search output with explicit work evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct CosineHnswSearchReport {
    /// Ranked approximate results.
    pub results: Vec<SearchResult>,
    /// Number of graph candidates re-scored exactly.
    pub candidate_count: usize,
    /// Total number of records in the index.
    pub total_record_count: usize,
}

/// Immutable approximate cosine index over an external HNSW graph.
pub struct CosineHnswIndex {
    config: CosineHnswConfig,
    dimensions: Option<usize>,
    records: Vec<VectorRecord>,
    graph: Hnsw<'static, f32, DistCosine>,
}

impl fmt::Debug for CosineHnswIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CosineHnswIndex")
            .field("config", &self.config)
            .field("dimensions", &self.dimensions)
            .field("record_count", &self.records.len())
            .finish_non_exhaustive()
    }
}

impl CosineHnswIndex {
    /// Builds the graph from records, inserting serially in iterator order.
    ///
    /// Records follow [`super::VectorSearchIndex::add`] validation and must
    /// additionally have a non-zero norm, because cosine is undefined for them.
    pub fn from_records(
        config: CosineHnswConfig,
        records: impl IntoIterator<Item = VectorRecord>,
    ) -> Result<Self> {
        validate_build_config(config)?;
        let records = records.into_iter().collect::<Vec<_>>();
        let mut dimensions = None;
        for record in &records {
            record.vector.validate()?;
            if record.id.trim().is_empty() {
                return Err(invalid_argument("record id must not be empty"));
            }
            match dimensions {
                Some(dimensions) if dimensions != record.vector.dimensions() => {
                    return Err(invalid_argument(
                        "indexed vectors must have the same dimensions",
                    ));
                }
                None => dimensions = Some(record.vector.dimensions()),
                _ => {}
            }
            if l2_norm(record.vector.as_slice())? <= f32::EPSILON {
                return Err(invalid_argument(
                    "cosine HNSW requires non-zero record vectors",
                ));
            }
        }

        let mut graph = Hnsw::new(
            config.max_connections,
            records.len().max(1),
            config.max_layers,
            config.ef_construction,
            DistCosine {},
        );
        for (record_index, record) in records.iter().enumerate() {
            graph.insert_slice((record.vector.as_slice(), record_index));
        }
        graph.set_searching_mode(true);

        Ok(Self {
            config,
            dimensions,
            records,
            graph,
        })
    }

    /// Returns the build-time configuration.
    pub fn config(&self) -> CosineHnswConfig {
        self.config
    }

    /// Returns indexed vector dimensions when at least one record exists.
    pub fn dimensions(&self) -> Option<usize> {
        self.dimensions
    }

    /// Returns indexed records in insertion order.
    pub fn records(&self) -> &[VectorRecord] {
        &self.records
    }

    /// Searches graph candidates and ranks them with exact cosine similarity.
    pub fn search(
        &self,
        query: &DenseVector,
        config: CosineHnswSearchConfig,
    ) -> Result<CosineHnswSearchReport> {
        validate_search_config(config)?;
        query.validate()?;
        if let Some(dimensions) = self.dimensions {
            if query.dimensions() != dimensions {
                return Err(invalid_argument("query dimensions must match the index"));
            }
        }
        if l2_norm(query.as_slice())? <= f32::EPSILON {
            return Err(invalid_argument("cosine HNSW requires a non-zero query"));
        }
        if self.records.is_empty() {
            return Ok(CosineHnswSearchReport {
                results: Vec::new(),
                candidate_count: 0,
                total_record_count: 0,
            });
        }

        let neighbours = self
            .graph
            .search(query.as_slice(), config.ef_search, config.ef_search);
        let candidate_count = neighbours.len();
        let results = top_k_fallible(
            neighbours.into_iter().map(|neighbour| {
                let record = self
                    .records
                    .get(neighbour.d_id)
                    .ok_or_else(|| invalid_argument("HNSW returned an unknown record"))?;
                let score = cosine_similarity(query.as_slice(), record.vector.as_slice())?;
                Ok(SearchResult {
                    id: record.id.clone(),
                    distance: 1.0 - score,
                    score,
                })
            }),
            config.limit,
            |left, right| {
                left.distance
                    .total_cmp(&right.distance)
                    .then_with(|| left.id.cmp(&right.id))
            },
        )?;

        Ok(CosineHnswSearchReport {
            results,
            candidate_count,
            total_record_count: self.records.len(),
        })
    }
}

fn validate_build_config(config: CosineHnswConfig) -> Result<()> {
    if !(2..=MAX_CONNECTIONS_LIMIT).contains(&config.max_connections) {
        return Err(invalid_argument(
            "cosine HNSW max_connections must be between 2 and 256",
        ));
    }
    if config.ef_construction == 0 {
        return Err(invalid_argument(
            "cosine HNSW ef_construction must be greater than zero",
        ));
    }
    if !(1..=MAX_LAYERS_LIMIT).contains(&config.max_layers) {
        return Err(invalid_argument(
            "cosine HNSW max_layers must be between 1 and 16",
        ));
    }
    Ok(())
}

fn validate_search_config(config: CosineHnswSearchConfig) -> Result<()> {
    if config.limit == 0 {
        return Err(invalid_argument("search limit must be greater than zero"));
    }
    if config.ef_search < config.limit {
        return Err(invalid_argument(
            "cosine HNSW ef_search must be at least the result limit",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::{SearchConfig, VectorSearchIndex};
    use vector_analysis_core::VectorMetric;

    fn clustered_vector(cluster: usize, variant: usize) -> DenseVector {
        let mut values = vec![0.0; 32];
        values[cluster] = 1.0;
        values[16 + (variant % 16)] = 0.015 * (variant as f32 + 1.0);
        DenseVector::new(values).unwrap()
    }

    fn clustered_records() -> Vec<VectorRecord> {
        (0..16)
            .flat_map(|cluster| {
                (0..16).map(move |variant| {
                    VectorRecord::new(
                        format!("cluster-{cluster:02}-variant-{variant:02}"),
                        clustered_vector(cluster, variant),
                    )
                })
            })
            .collect()
    }

    #[test]
    fn matches_exact_neighbours_on_clustered_records() {
        let records = clustered_records();
        let exact = VectorSearchIndex::from_records(records.clone()).unwrap();
        let index = CosineHnswIndex::from_records(CosineHnswConfig::default(), records).unwrap();
        let query = clustered_vector(5, 4);
        let exact_results = exact
            .search(
                &query,
                SearchConfig {
                    metric: VectorMetric::Cosine,
                    limit: 10,
                },
            )
            .unwrap();
        let report = index
            .search(
                &query,
                CosineHnswSearchConfig {
                    limit: 10,
                    ef_search: 32,
                },
            )
            .unwrap();

        assert_eq!(report.results, exact_results);
        assert!(report.candidate_count <= 32);
        assert_eq!(report.total_record_count, 256);
    }

    #[test]
    fn rebuilding_from_same_records_is_deterministic() {
        let left = CosineHnswIndex::from_records(CosineHnswConfig::default(), clustered_records())
            .unwrap();
        let right = CosineHnswIndex::from_records(CosineHnswConfig::default(), clustered_records())
            .unwrap();
        for cluster in 0..16 {
            let query = clustered_vector(cluster, 3);
            assert_eq!(
                left.search(&query, CosineHnswSearchConfig::default())
                    .unwrap(),
                right
                    .search(&query, CosineHnswSearchConfig::default())
                    .unwrap()
            );
        }
    }

    #[test]
    fn equal_distances_tie_break_by_record_id() {
        let vector = DenseVector::new([1.0, 0.0]).unwrap();
        let index = CosineHnswIndex::from_records(
            CosineHnswConfig::default(),
            ["c", "a", "b"].map(|id| VectorRecord::new(id, vector.clone())),
        )
        .unwrap();
        let ids = index
            .search(
                &vector,
                CosineHnswSearchConfig {
                    limit: 3,
                    ef_search: 3,
                },
            )
            .unwrap()
            .results
            .into_iter()
            .map(|result| result.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, ["a", "b", "c"]);
    }

    #[test]
    fn empty_index_returns_no_results() {
        let index = CosineHnswIndex::from_records(CosineHnswConfig::default(), []).unwrap();
        let report = index
            .search(
                &DenseVector::new([1.0, 0.0]).unwrap(),
                CosineHnswSearchConfig::default(),
            )
            .unwrap();
        assert!(report.results.is_empty());
        assert_eq!(report.total_record_count, 0);
    }

    #[test]
    fn rejects_invalid_configuration_and_vectors() {
        for config in [
            CosineHnswConfig {
                max_connections: 257,
                ..CosineHnswConfig::default()
            },
            CosineHnswConfig {
                ef_construction: 0,
                ..CosineHnswConfig::default()
            },
            CosineHnswConfig {
                max_layers: 17,
                ..CosineHnswConfig::default()
            },
        ] {
            assert!(CosineHnswIndex::from_records(config, []).is_err());
        }
        assert!(CosineHnswIndex::from_records(
            CosineHnswConfig::default(),
            [VectorRecord::new(
                "zero",
                DenseVector::new([0.0, 0.0]).unwrap()
            )],
        )
        .is_err());

        let index = CosineHnswIndex::from_records(
            CosineHnswConfig::default(),
            [VectorRecord::new(
                "one",
                DenseVector::new([1.0, 0.0]).unwrap(),
            )],
        )
        .unwrap();
        let query = DenseVector::new([1.0, 0.0]).unwrap();
        assert!(index
            .search(
                &query,
                CosineHnswSearchConfig {
                    limit: 10,
                    ef_search: 5,
                },
            )
            .is_err());
        assert!(index
            .search(
                &DenseVector::new([1.0, 0.0, 0.0]).unwrap(),
                CosineHnswSearchConfig::default()
            )
            .is_err());
        let ids = index
            .search(&query, CosineHnswSearchConfig::default())
            .unwrap()
            .results
            .into_iter()
            .map(|result| result.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(ids, BTreeSet::from(["one".to_string()]));
    }
}
