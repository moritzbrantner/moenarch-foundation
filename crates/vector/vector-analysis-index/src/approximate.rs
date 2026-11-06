use std::collections::{BTreeMap, BTreeSet};

use media_core::Result;
use vector_analysis_core::{cosine_similarity, DenseVector};

use super::{invalid_argument, top_k_fallible, SearchResult, VectorRecord};

const DEFAULT_SEED: u64 = 0x6a09_e667_f3bc_c909;

/// Build-time configuration for deterministic random-hyperplane cosine LSH.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CosineLshConfig {
    /// Number of signature bits. More bits create narrower buckets.
    pub hash_bits: u8,
    /// Seed used to derive deterministic hyperplanes.
    pub seed: u64,
}

impl Default for CosineLshConfig {
    fn default() -> Self {
        Self {
            hash_bits: 16,
            seed: DEFAULT_SEED,
        }
    }
}

/// Query-time effort configuration for [`CosineLshIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CosineLshSearchConfig {
    /// Maximum number of ranked results to return.
    pub limit: usize,
    /// Maximum Hamming distance from the query signature to probe.
    pub probe_radius: u8,
    /// Maximum number of candidate records to score exactly.
    pub max_candidates: usize,
}

impl Default for CosineLshSearchConfig {
    fn default() -> Self {
        Self {
            limit: 10,
            probe_radius: 1,
            max_candidates: 256,
        }
    }
}

/// Approximate cosine-search output with explicit work evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct CosineLshSearchReport {
    /// Ranked approximate results.
    pub results: Vec<SearchResult>,
    /// Number of record vectors scored exactly after LSH candidate selection.
    pub candidate_count: usize,
    /// Number of signature buckets that existed and were inspected.
    pub probed_bucket_count: usize,
    /// Total number of records in the index.
    pub total_record_count: usize,
}

/// Deterministic random-hyperplane LSH index for approximate cosine search.
///
/// The index uses seeded hyperplanes only to choose a bounded candidate set.
/// Candidate vectors are then ranked with the same exact cosine similarity used
/// by the reference [`super::VectorSearchIndex`]. This keeps approximation at
/// the candidate-selection boundary and makes search effort observable.
#[derive(Debug, Clone, PartialEq)]
pub struct CosineLshIndex {
    config: CosineLshConfig,
    dimensions: Option<usize>,
    records: Vec<VectorRecord>,
    buckets: BTreeMap<u64, Vec<usize>>,
}

impl CosineLshIndex {
    /// Creates an empty LSH index with validated configuration.
    pub fn new(config: CosineLshConfig) -> Result<Self> {
        validate_build_config(config)?;
        Ok(Self {
            config,
            dimensions: None,
            records: Vec::new(),
            buckets: BTreeMap::new(),
        })
    }

    /// Returns the build-time configuration.
    pub fn config(&self) -> CosineLshConfig {
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

    /// Removes all records while preserving LSH configuration.
    pub fn clear(&mut self) {
        self.dimensions = None;
        self.records.clear();
        self.buckets.clear();
    }

    /// Adds one record to the approximate index.
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

        let signature = signature(
            record.vector.as_slice(),
            self.config.hash_bits,
            self.config.seed,
        );
        let record_index = self.records.len();
        self.records.push(record);
        self.buckets
            .entry(signature)
            .or_default()
            .push(record_index);
        Ok(())
    }

    /// Adds many records in iterator order.
    pub fn extend(&mut self, records: impl IntoIterator<Item = VectorRecord>) -> Result<()> {
        for record in records {
            self.add(record)?;
        }
        Ok(())
    }

    /// Builds an LSH index from records.
    pub fn from_records(
        config: CosineLshConfig,
        records: impl IntoIterator<Item = VectorRecord>,
    ) -> Result<Self> {
        let mut index = Self::new(config)?;
        index.extend(records)?;
        Ok(index)
    }

    /// Searches a bounded candidate set selected by LSH signature proximity.
    pub fn search(
        &self,
        query: &DenseVector,
        config: CosineLshSearchConfig,
    ) -> Result<CosineLshSearchReport> {
        validate_search_config(self.config, config)?;
        query.validate()?;
        if let Some(dimensions) = self.dimensions {
            if query.dimensions() != dimensions {
                return Err(invalid_argument("query dimensions must match the index"));
            }
        }

        let query_signature = signature(query.as_slice(), self.config.hash_bits, self.config.seed);
        let probe_signatures =
            signatures_within_radius(query_signature, self.config.hash_bits, config.probe_radius);
        let mut candidate_indices = BTreeSet::new();
        let mut probed_bucket_count = 0usize;

        'probes: for candidate_signature in probe_signatures {
            let Some(bucket) = self.buckets.get(&candidate_signature) else {
                continue;
            };
            probed_bucket_count += 1;
            for record_index in bucket {
                candidate_indices.insert(*record_index);
                if candidate_indices.len() >= config.max_candidates {
                    break 'probes;
                }
            }
        }

        let results = top_k_fallible(
            candidate_indices.iter().copied().map(|record_index| {
                let record = &self.records[record_index];
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

        Ok(CosineLshSearchReport {
            results,
            candidate_count: candidate_indices.len(),
            probed_bucket_count,
            total_record_count: self.records.len(),
        })
    }
}

fn validate_build_config(config: CosineLshConfig) -> Result<()> {
    if !(1..=32).contains(&config.hash_bits) {
        return Err(invalid_argument(
            "cosine LSH hash_bits must be between 1 and 32",
        ));
    }
    Ok(())
}

fn validate_search_config(build: CosineLshConfig, search: CosineLshSearchConfig) -> Result<()> {
    if search.limit == 0 {
        return Err(invalid_argument("search limit must be greater than zero"));
    }
    if search.max_candidates == 0 {
        return Err(invalid_argument(
            "cosine LSH max_candidates must be greater than zero",
        ));
    }
    if search.max_candidates < search.limit {
        return Err(invalid_argument(
            "cosine LSH max_candidates must be at least the result limit",
        ));
    }
    if search.probe_radius > build.hash_bits || search.probe_radius > 3 {
        return Err(invalid_argument(
            "cosine LSH probe_radius must not exceed hash_bits or 3",
        ));
    }
    Ok(())
}

fn signature(values: &[f32], hash_bits: u8, seed: u64) -> u64 {
    let mut signature = 0u64;
    for bit in 0..hash_bits {
        let projection = values
            .iter()
            .enumerate()
            .map(|(dimension, value)| value * hyperplane_weight(seed, bit, dimension))
            .sum::<f32>();
        if projection >= 0.0 {
            signature |= 1u64 << bit;
        }
    }
    signature
}

fn hyperplane_weight(seed: u64, bit: u8, dimension: usize) -> f32 {
    let mixed = splitmix64(
        seed ^ (u64::from(bit) + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ (dimension as u64 + 1).wrapping_mul(0xbf58_476d_1ce4_e5b9),
    );
    if mixed & 1 == 0 {
        -1.0
    } else {
        1.0
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// Probe exact, one-bit, two-bit, then three-bit neighbors in stable order.
///
/// Iteration stops as soon as the caller has enough candidates; in particular,
/// a query capped in its first bucket never materializes the other signatures.
fn signatures_within_radius(
    signature: u64,
    hash_bits: u8,
    radius: u8,
) -> impl Iterator<Item = u64> {
    let one_bit_limit = if radius >= 1 { hash_bits } else { 0 };
    let two_bit_limit = if radius >= 2 { hash_bits } else { 0 };
    let three_bit_limit = if radius >= 3 { hash_bits } else { 0 };

    std::iter::once(signature)
        .chain((0..one_bit_limit).map(move |first| signature ^ (1u64 << first)))
        .chain((0..two_bit_limit).flat_map(move |first| {
            ((first + 1)..two_bit_limit)
                .map(move |second| signature ^ (1u64 << first) ^ (1u64 << second))
        }))
        .chain((0..three_bit_limit).flat_map(move |first| {
            ((first + 1)..three_bit_limit).flat_map(move |second| {
                ((second + 1)..three_bit_limit).map(move |third| {
                    signature ^ (1u64 << first) ^ (1u64 << second) ^ (1u64 << third)
                })
            })
        }))
}

#[cfg(test)]
mod tests {
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
    fn deterministic_lsh_returns_same_result_for_same_config() {
        let records = clustered_records();
        let left =
            CosineLshIndex::from_records(CosineLshConfig::default(), records.clone()).unwrap();
        let right = CosineLshIndex::from_records(CosineLshConfig::default(), records).unwrap();
        let query = clustered_vector(3, 7);

        assert_eq!(
            left.search(&query, CosineLshSearchConfig::default())
                .unwrap(),
            right
                .search(&query, CosineLshSearchConfig::default())
                .unwrap()
        );
    }

    #[test]
    fn approximate_search_matches_exact_cluster_while_scoring_bounded_candidates() {
        let records = clustered_records();
        let exact = VectorSearchIndex::from_records(records.clone()).unwrap();
        let approximate = CosineLshIndex::from_records(
            CosineLshConfig {
                hash_bits: 12,
                ..CosineLshConfig::default()
            },
            records,
        )
        .unwrap();
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
        let report = approximate
            .search(
                &query,
                CosineLshSearchConfig {
                    limit: 10,
                    probe_radius: 1,
                    max_candidates: 64,
                },
            )
            .unwrap();
        let exact_ids = exact_results
            .iter()
            .map(|result| result.id.as_str())
            .collect::<BTreeSet<_>>();
        let shared = report
            .results
            .iter()
            .filter(|result| exact_ids.contains(result.id.as_str()))
            .count();

        assert_eq!(
            report.results.first().map(|result| result.id.as_str()),
            exact_results.first().map(|result| result.id.as_str())
        );
        assert!(
            shared >= 8,
            "expected at least 8/10 exact-neighbor recall, got {shared}"
        );
        assert!(report.candidate_count <= 64);
        assert!(report.candidate_count < report.total_record_count / 2);
    }

    #[test]
    fn search_effort_is_explicitly_bounded() {
        let index = CosineLshIndex::from_records(
            CosineLshConfig {
                hash_bits: 8,
                ..CosineLshConfig::default()
            },
            clustered_records(),
        )
        .unwrap();
        let report = index
            .search(
                &clustered_vector(2, 1),
                CosineLshSearchConfig {
                    limit: 5,
                    probe_radius: 2,
                    max_candidates: 24,
                },
            )
            .unwrap();

        assert!(report.candidate_count <= 24);
        assert_eq!(report.total_record_count, 256);
    }

    #[test]
    fn probe_signatures_preserve_radius_order_and_combinatorial_bounds() {
        assert_eq!(
            signatures_within_radius(0, 4, 2).collect::<Vec<_>>(),
            [0, 1, 2, 4, 8, 3, 5, 9, 6, 10, 12]
        );

        for bits in [1_u8, 8, 16, 32] {
            for radius in 0..=bits.min(3) {
                let signatures = signatures_within_radius(0, bits, radius).collect::<Vec<_>>();
                let expected = (0..=u64::from(radius))
                    .map(|chosen| {
                        (0..chosen).fold(1_u64, |count, index| {
                            count * (u64::from(bits) - index) / (index + 1)
                        }) as usize
                    })
                    .sum::<usize>();
                assert_eq!(signatures.len(), expected, "bits={bits}, radius={radius}");
                assert_eq!(
                    signatures.iter().copied().collect::<BTreeSet<_>>().len(),
                    expected,
                    "duplicate probe with bits={bits}, radius={radius}"
                );
            }
        }
        assert_eq!(signatures_within_radius(0, 32, 3).count(), 5_489);
    }

    #[test]
    fn dense_bucket_search_work_stays_capped_as_corpus_grows() {
        let query = DenseVector::new([1.0, 0.0, 0.0, 0.0]).unwrap();
        let mut index = CosineLshIndex::new(CosineLshConfig {
            hash_bits: 32,
            ..CosineLshConfig::default()
        })
        .unwrap();

        for corpus_size in [64, 256, 1_024, 4_096] {
            for record_index in index.records().len()..corpus_size {
                index
                    .add(VectorRecord::new(
                        format!("record-{record_index:05}"),
                        query.clone(),
                    ))
                    .unwrap();
            }

            for max_candidates in [8, 16, 32] {
                let report = index
                    .search(
                        &query,
                        CosineLshSearchConfig {
                            limit: 4,
                            probe_radius: 3,
                            max_candidates,
                        },
                    )
                    .unwrap();

                assert_eq!(report.total_record_count, corpus_size);
                assert_eq!(report.candidate_count, max_candidates);
                assert_eq!(report.probed_bucket_count, 1);
                assert_eq!(report.results.len(), 4);
            }
        }
    }

    #[test]
    fn rejects_invalid_build_and_search_effort() {
        assert!(CosineLshIndex::new(CosineLshConfig {
            hash_bits: 0,
            seed: DEFAULT_SEED,
        })
        .is_err());

        let index = CosineLshIndex::from_records(
            CosineLshConfig::default(),
            [VectorRecord::new(
                "one",
                DenseVector::new([1.0, 0.0]).unwrap(),
            )],
        )
        .unwrap();
        assert!(index
            .search(
                &DenseVector::new([1.0, 0.0]).unwrap(),
                CosineLshSearchConfig {
                    limit: 10,
                    max_candidates: 5,
                    ..CosineLshSearchConfig::default()
                },
            )
            .is_err());
    }
}
