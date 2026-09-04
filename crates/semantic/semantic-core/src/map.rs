use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt::{Display, Formatter};

use serde::Serialize;
use vector_analysis_core::cosine_similarity;

use crate::EntityId;

/// Borrowed entity/vector pair supplied by a consumer to semantic-map derivation.
///
/// The vector remains consumer-owned. `semantic-core` neither generates nor persists
/// embeddings; it only derives deterministic structural evidence from the supplied values.
#[derive(Debug, Clone, Copy)]
pub struct SemanticMapInput<'a> {
    /// Stable identity of the entity represented by the vector.
    pub entity_id: &'a EntityId,
    /// Caller-owned finite vector representation for this analysis pass.
    pub vector: &'a [f32],
}

/// Structural options for exact semantic-map derivation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticMapOptions {
    /// Maximum number of nearest candidates contributed by each entity.
    pub neighbors_per_entity: usize,
    /// Minimum cosine similarity required for a neighborhood edge.
    pub neighbor_threshold: f32,
    /// Minimum cosine similarity used to connect entities into clusters.
    pub cluster_threshold: f32,
}

impl Default for SemanticMapOptions {
    fn default() -> Self {
        Self {
            neighbors_per_entity: 4,
            neighbor_threshold: 0.25,
            cluster_threshold: 0.60,
        }
    }
}

/// Undirected exact-similarity edge between two entities.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SemanticMapNeighbor {
    /// First endpoint in deterministic input-order orientation.
    pub source_entity_id: EntityId,
    /// Second endpoint in deterministic input-order orientation.
    pub target_entity_id: EntityId,
    /// Cosine similarity of the two supplied vectors.
    pub similarity: f32,
}

/// Deterministic connected component over the supplied semantic vectors.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SemanticMapCluster {
    /// Member identities in original input order.
    pub member_entity_ids: Vec<EntityId>,
    /// Medoid selected by maximum mean similarity, with input order breaking ties.
    pub representative_entity_id: EntityId,
    /// Mean pairwise cosine similarity inside the cluster.
    pub mean_similarity: f32,
}

/// Domain-neutral exact semantic structure derived from caller-provided vectors.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SemanticMap {
    /// Undirected nearest-neighbor edges.
    pub neighbors: Vec<SemanticMapNeighbor>,
    /// Deterministic threshold-connected clusters.
    pub clusters: Vec<SemanticMapCluster>,
}

/// Validation failures produced before semantic-map derivation.
#[derive(Debug, Clone, PartialEq)]
pub enum SemanticMapError {
    /// No entities were supplied.
    EmptyInput,
    /// The nearest-neighbor limit was zero.
    ZeroNeighborsPerEntity,
    /// A similarity threshold was NaN, infinite, or outside `[-1, 1]`.
    InvalidSimilarityThreshold {
        /// Option name that rejected the value.
        name: &'static str,
        /// Rejected threshold.
        value: f32,
    },
    /// The same stable entity identity occurred more than once.
    DuplicateEntity {
        /// Duplicated identity.
        entity_id: EntityId,
    },
    /// An entity was represented by an empty vector.
    EmptyVector {
        /// Entity with the invalid vector.
        entity_id: EntityId,
    },
    /// A vector contained NaN or infinity.
    NonFiniteVector {
        /// Entity with the invalid vector.
        entity_id: EntityId,
    },
    /// All vectors in one map must use one dimensionality.
    InconsistentDimensions {
        /// Expected dimensions established by the first input.
        expected: usize,
        /// Dimensions found on this entity.
        actual: usize,
        /// Entity with the mismatched vector.
        entity_id: EntityId,
    },
}

impl Display for SemanticMapError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => formatter.write_str("semantic map input must not be empty"),
            Self::ZeroNeighborsPerEntity => {
                formatter.write_str("semantic map neighbors_per_entity must be greater than zero")
            }
            Self::InvalidSimilarityThreshold { name, value } => write!(
                formatter,
                "semantic map {name} must be finite and between -1 and 1, got {value}"
            ),
            Self::DuplicateEntity { entity_id } => {
                write!(formatter, "duplicate semantic map entity `{entity_id}`")
            }
            Self::EmptyVector { entity_id } => {
                write!(
                    formatter,
                    "semantic map entity `{entity_id}` has an empty vector"
                )
            }
            Self::NonFiniteVector { entity_id } => write!(
                formatter,
                "semantic map entity `{entity_id}` has non-finite vector components"
            ),
            Self::InconsistentDimensions {
                expected,
                actual,
                entity_id,
            } => write!(
                formatter,
                "semantic map entity `{entity_id}` has {actual} dimensions; expected {expected}"
            ),
        }
    }
}

impl Error for SemanticMapError {}

/// Derives an exact deterministic semantic neighborhood and cluster map.
///
/// Input order is semantically observable: it breaks otherwise-equal medoid ties and
/// determines cluster/member ordering. Stable entity identities break nearest-neighbor
/// score ties, so repeated runs over the same ordered inputs produce identical output.
pub fn build_semantic_map(
    inputs: &[SemanticMapInput<'_>],
    options: SemanticMapOptions,
) -> Result<SemanticMap, SemanticMapError> {
    validate_inputs(inputs, options)?;

    let similarities = similarity_matrix(inputs);
    Ok(SemanticMap {
        neighbors: neighborhood_graph(inputs, &similarities, options),
        clusters: concept_clusters(inputs, &similarities, options.cluster_threshold),
    })
}

fn validate_inputs(
    inputs: &[SemanticMapInput<'_>],
    options: SemanticMapOptions,
) -> Result<(), SemanticMapError> {
    if inputs.is_empty() {
        return Err(SemanticMapError::EmptyInput);
    }
    if options.neighbors_per_entity == 0 {
        return Err(SemanticMapError::ZeroNeighborsPerEntity);
    }
    validate_similarity_threshold("neighbor_threshold", options.neighbor_threshold)?;
    validate_similarity_threshold("cluster_threshold", options.cluster_threshold)?;

    let expected_dimensions = inputs[0].vector.len();
    let mut seen = BTreeSet::new();
    for input in inputs {
        if !seen.insert(input.entity_id.clone()) {
            return Err(SemanticMapError::DuplicateEntity {
                entity_id: input.entity_id.clone(),
            });
        }
        if input.vector.is_empty() {
            return Err(SemanticMapError::EmptyVector {
                entity_id: input.entity_id.clone(),
            });
        }
        if input.vector.len() != expected_dimensions {
            return Err(SemanticMapError::InconsistentDimensions {
                expected: expected_dimensions,
                actual: input.vector.len(),
                entity_id: input.entity_id.clone(),
            });
        }
        if !input.vector.iter().all(|value| value.is_finite()) {
            return Err(SemanticMapError::NonFiniteVector {
                entity_id: input.entity_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_similarity_threshold(name: &'static str, value: f32) -> Result<(), SemanticMapError> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        return Err(SemanticMapError::InvalidSimilarityThreshold { name, value });
    }
    Ok(())
}

fn similarity_matrix(inputs: &[SemanticMapInput<'_>]) -> Vec<Vec<f32>> {
    let mut matrix = vec![vec![0.0; inputs.len()]; inputs.len()];
    for left in 0..inputs.len() {
        matrix[left][left] = 1.0;
        for right in (left + 1)..inputs.len() {
            let similarity = cosine(inputs[left].vector, inputs[right].vector);
            matrix[left][right] = similarity;
            matrix[right][left] = similarity;
        }
    }
    matrix
}

fn neighborhood_graph(
    inputs: &[SemanticMapInput<'_>],
    similarities: &[Vec<f32>],
    options: SemanticMapOptions,
) -> Vec<SemanticMapNeighbor> {
    let mut edges = BTreeMap::<(usize, usize), f32>::new();
    for (source, source_similarities) in similarities.iter().enumerate() {
        let mut candidates = (0..inputs.len())
            .filter(|target| *target != source)
            .map(|target| (target, source_similarities[target]))
            .collect::<Vec<_>>();
        candidates.sort_by(|(left_index, left_score), (right_index, right_score)| {
            right_score.total_cmp(left_score).then_with(|| {
                inputs[*left_index]
                    .entity_id
                    .cmp(inputs[*right_index].entity_id)
            })
        });

        for (target, similarity) in candidates.into_iter().take(options.neighbors_per_entity) {
            if similarity < options.neighbor_threshold {
                continue;
            }
            let pair = if source < target {
                (source, target)
            } else {
                (target, source)
            };
            edges
                .entry(pair)
                .and_modify(|score| *score = score.max(similarity))
                .or_insert(similarity);
        }
    }

    edges
        .into_iter()
        .map(|((source, target), similarity)| SemanticMapNeighbor {
            source_entity_id: inputs[source].entity_id.clone(),
            target_entity_id: inputs[target].entity_id.clone(),
            similarity,
        })
        .collect()
}

fn concept_clusters(
    inputs: &[SemanticMapInput<'_>],
    similarities: &[Vec<f32>],
    threshold: f32,
) -> Vec<SemanticMapCluster> {
    let mut visited = vec![false; inputs.len()];
    let mut clusters = Vec::new();

    for seed in 0..inputs.len() {
        if visited[seed] {
            continue;
        }
        visited[seed] = true;
        let mut queue = VecDeque::from([seed]);
        let mut members = Vec::new();
        while let Some(current) = queue.pop_front() {
            members.push(current);
            for candidate in 0..inputs.len() {
                if !visited[candidate] && similarities[current][candidate] >= threshold {
                    visited[candidate] = true;
                    queue.push_back(candidate);
                }
            }
        }
        members.sort_unstable();

        let representative = cluster_medoid(&members, similarities);
        clusters.push(SemanticMapCluster {
            member_entity_ids: members
                .iter()
                .map(|member| inputs[*member].entity_id.clone())
                .collect(),
            representative_entity_id: inputs[representative].entity_id.clone(),
            mean_similarity: cluster_mean_similarity(&members, similarities),
        });
    }

    clusters
}

fn cluster_medoid(members: &[usize], similarities: &[Vec<f32>]) -> usize {
    if members.len() == 1 {
        return members[0];
    }

    members
        .iter()
        .copied()
        .map(|candidate| {
            let score = members
                .iter()
                .copied()
                .filter(|other| *other != candidate)
                .map(|other| similarities[candidate][other])
                .sum::<f32>()
                / (members.len() - 1) as f32;
            (candidate, score)
        })
        .max_by(|(left_index, left_score), (right_index, right_score)| {
            left_score
                .total_cmp(right_score)
                .then_with(|| right_index.cmp(left_index))
        })
        .map(|(candidate, _)| candidate)
        .unwrap_or(members[0])
}

fn cluster_mean_similarity(members: &[usize], similarities: &[Vec<f32>]) -> f32 {
    if members.len() <= 1 {
        return 1.0;
    }
    let mut total = 0.0;
    let mut pairs = 0usize;
    for left in 0..members.len() {
        for right in (left + 1)..members.len() {
            total += similarities[members[left]][members[right]];
            pairs += 1;
        }
    }
    total / pairs as f32
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    // Map construction validates non-empty, finite, equal-dimensional vectors first.
    // `vector-analysis-core` therefore only rejects the effectively-zero norm case,
    // which the existing semantic-map baseline intentionally treats as zero similarity.
    cosine_similarity(left, right).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(value: &str) -> EntityId {
        EntityId::new(value).unwrap()
    }

    #[test]
    fn clothing_vectors_form_a_domain_neutral_cluster() {
        let navy_shirt = entity("clothes:navy-shirt");
        let blue_shirt = entity("clothes:blue-shirt");
        let running_shoe = entity("clothes:running-shoe");
        let inputs = [
            SemanticMapInput {
                entity_id: &navy_shirt,
                vector: &[1.0, 0.0, 0.0],
            },
            SemanticMapInput {
                entity_id: &blue_shirt,
                vector: &[0.99, 0.10, 0.0],
            },
            SemanticMapInput {
                entity_id: &running_shoe,
                vector: &[0.0, 0.0, 1.0],
            },
        ];

        let map = build_semantic_map(
            &inputs,
            SemanticMapOptions {
                neighbors_per_entity: 2,
                neighbor_threshold: 0.5,
                cluster_threshold: 0.8,
            },
        )
        .unwrap();

        assert_eq!(map.neighbors.len(), 1);
        assert_eq!(map.neighbors[0].source_entity_id, navy_shirt);
        assert_eq!(map.neighbors[0].target_entity_id, blue_shirt);
        assert_eq!(map.clusters.len(), 2);
        assert_eq!(
            map.clusters[0].member_entity_ids,
            vec![navy_shirt.clone(), blue_shirt.clone()]
        );
        assert_eq!(map.clusters[0].representative_entity_id, navy_shirt);
        assert_eq!(map.clusters[1].member_entity_ids, vec![running_shoe]);
    }

    #[test]
    fn same_ordered_inputs_are_fully_deterministic() {
        let first = entity("entity:first");
        let second = entity("entity:second");
        let third = entity("entity:third");
        let inputs = [
            SemanticMapInput {
                entity_id: &first,
                vector: &[1.0, 0.0],
            },
            SemanticMapInput {
                entity_id: &second,
                vector: &[1.0, 0.0],
            },
            SemanticMapInput {
                entity_id: &third,
                vector: &[0.0, 1.0],
            },
        ];

        let first_map = build_semantic_map(&inputs, SemanticMapOptions::default()).unwrap();
        let second_map = build_semantic_map(&inputs, SemanticMapOptions::default()).unwrap();

        assert_eq!(first_map, second_map);
    }

    #[test]
    fn stable_entity_ids_break_equal_neighbor_scores() {
        let source = entity("entity:source");
        let alpha = entity("entity:alpha");
        let zeta = entity("entity:zeta");
        let inputs = [
            SemanticMapInput {
                entity_id: &source,
                vector: &[1.0, 0.0],
            },
            SemanticMapInput {
                entity_id: &zeta,
                vector: &[1.0, 0.0],
            },
            SemanticMapInput {
                entity_id: &alpha,
                vector: &[1.0, 0.0],
            },
        ];

        let map = build_semantic_map(
            &inputs,
            SemanticMapOptions {
                neighbors_per_entity: 1,
                neighbor_threshold: 0.0,
                cluster_threshold: 1.0,
            },
        )
        .unwrap();

        assert!(map
            .neighbors
            .iter()
            .any(|edge| { edge.source_entity_id == source && edge.target_entity_id == alpha }));
    }

    #[test]
    fn rejects_duplicate_ids_and_invalid_vectors() {
        let repeated = entity("entity:repeated");
        let duplicate_inputs = [
            SemanticMapInput {
                entity_id: &repeated,
                vector: &[1.0],
            },
            SemanticMapInput {
                entity_id: &repeated,
                vector: &[2.0],
            },
        ];
        assert!(matches!(
            build_semantic_map(&duplicate_inputs, SemanticMapOptions::default()),
            Err(SemanticMapError::DuplicateEntity { .. })
        ));

        let other = entity("entity:other");
        let mismatched = [
            SemanticMapInput {
                entity_id: &repeated,
                vector: &[1.0],
            },
            SemanticMapInput {
                entity_id: &other,
                vector: &[1.0, 2.0],
            },
        ];
        assert!(matches!(
            build_semantic_map(&mismatched, SemanticMapOptions::default()),
            Err(SemanticMapError::InconsistentDimensions { .. })
        ));
    }

    #[test]
    fn zero_vectors_are_valid_but_have_zero_similarity() {
        let zero = entity("entity:zero");
        let nonzero = entity("entity:nonzero");
        let inputs = [
            SemanticMapInput {
                entity_id: &zero,
                vector: &[0.0, 0.0],
            },
            SemanticMapInput {
                entity_id: &nonzero,
                vector: &[1.0, 0.0],
            },
        ];

        let map = build_semantic_map(
            &inputs,
            SemanticMapOptions {
                neighbors_per_entity: 1,
                neighbor_threshold: 0.0,
                cluster_threshold: 0.5,
            },
        )
        .unwrap();

        assert_eq!(map.neighbors.len(), 1);
        assert_eq!(map.neighbors[0].similarity, 0.0);
        assert_eq!(map.clusters.len(), 2);
    }
}
