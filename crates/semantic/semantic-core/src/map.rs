use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt::{Display, Formatter};

use serde::Serialize;

use crate::EntityId;

/// Borrowed entity/value pair supplied by a consumer to semantic-map derivation.
///
/// The value is opaque to `semantic-core`. Consumers choose the representation and
/// similarity function, so text embeddings, visual embeddings, attributes, and fused
/// multimodal evidence can share the same structural derivation without leaking domain
/// or vector-math ownership into this crate.
#[derive(Debug, Clone, Copy)]
pub struct SemanticMapInput<'a, T: ?Sized> {
    /// Stable identity of the entity represented by the value.
    pub entity_id: &'a EntityId,
    /// Caller-owned value used by the supplied similarity function.
    pub value: &'a T,
}

/// Structural options for semantic-map derivation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticMapOptions {
    /// Maximum number of nearest candidates contributed by each entity.
    pub neighbors_per_entity: usize,
    /// Minimum similarity required for a neighborhood edge.
    pub neighbor_threshold: f32,
    /// Minimum similarity used to connect entities into clusters.
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

/// Undirected similarity edge between two entities.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SemanticMapNeighbor {
    /// First endpoint in deterministic input-order orientation.
    pub source_entity_id: EntityId,
    /// Second endpoint in deterministic input-order orientation.
    pub target_entity_id: EntityId,
    /// Caller-supplied similarity of the two values.
    pub similarity: f32,
}

/// Deterministic connected component over the supplied entities.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SemanticMapCluster {
    /// Member identities in original input order.
    pub member_entity_ids: Vec<EntityId>,
    /// Medoid selected by maximum mean similarity, with input order breaking ties.
    pub representative_entity_id: EntityId,
    /// Mean pairwise similarity inside the cluster.
    pub mean_similarity: f32,
}

/// Domain-neutral semantic structure derived from caller-owned similarity evidence.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SemanticMap {
    /// Undirected nearest-neighbor edges.
    pub neighbors: Vec<SemanticMapNeighbor>,
    /// Deterministic threshold-connected clusters.
    pub clusters: Vec<SemanticMapCluster>,
}

/// Validation failures produced before or during semantic-map derivation.
#[derive(Debug, Clone, PartialEq)]
pub enum SemanticMapError {
    /// No entities were supplied.
    EmptyInput,
    /// The nearest-neighbor limit was zero.
    ZeroNeighborsPerEntity,
    /// A configured similarity threshold was NaN, infinite, or outside `[-1, 1]`.
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
    /// The caller returned NaN, infinity, or a score outside `[-1, 1]`.
    InvalidSimilarity {
        /// First entity in the rejected pair.
        source_entity_id: EntityId,
        /// Second entity in the rejected pair.
        target_entity_id: EntityId,
        /// Rejected similarity value.
        value: f32,
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
            Self::InvalidSimilarity {
                source_entity_id,
                target_entity_id,
                value,
            } => write!(
                formatter,
                "semantic map similarity between `{source_entity_id}` and `{target_entity_id}` must be finite and between -1 and 1, got {value}"
            ),
        }
    }
}

impl Error for SemanticMapError {}

/// Derives a deterministic semantic neighborhood and cluster map with caller-owned similarity.
///
/// The supplied function is evaluated exactly once for every unordered pair. Input order is
/// semantically observable: it breaks otherwise-equal medoid ties and determines cluster/member
/// ordering. Stable entity identities break nearest-neighbor score ties, so repeated runs over
/// the same ordered inputs and deterministic similarity function produce identical output.
pub fn build_semantic_map_with<T: ?Sized, F>(
    inputs: &[SemanticMapInput<'_, T>],
    options: SemanticMapOptions,
    mut similarity: F,
) -> Result<SemanticMap, SemanticMapError>
where
    F: FnMut(&T, &T) -> f32,
{
    validate_inputs(inputs, options)?;

    let similarities = similarity_matrix(inputs, &mut similarity)?;
    Ok(SemanticMap {
        neighbors: neighborhood_graph(inputs, &similarities, options),
        clusters: concept_clusters(inputs, &similarities, options.cluster_threshold),
    })
}

fn validate_inputs<T: ?Sized>(
    inputs: &[SemanticMapInput<'_, T>],
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

    let mut seen = BTreeSet::new();
    for input in inputs {
        if !seen.insert(input.entity_id.clone()) {
            return Err(SemanticMapError::DuplicateEntity {
                entity_id: input.entity_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_similarity_threshold(name: &'static str, value: f32) -> Result<(), SemanticMapError> {
    if !is_similarity(value) {
        return Err(SemanticMapError::InvalidSimilarityThreshold { name, value });
    }
    Ok(())
}

fn is_similarity(value: f32) -> bool {
    value.is_finite() && (-1.0..=1.0).contains(&value)
}

fn similarity_matrix<T: ?Sized, F>(
    inputs: &[SemanticMapInput<'_, T>],
    similarity: &mut F,
) -> Result<Vec<Vec<f32>>, SemanticMapError>
where
    F: FnMut(&T, &T) -> f32,
{
    let mut matrix = vec![vec![0.0; inputs.len()]; inputs.len()];
    for left in 0..inputs.len() {
        matrix[left][left] = 1.0;
        for right in (left + 1)..inputs.len() {
            let score = similarity(inputs[left].value, inputs[right].value);
            if !is_similarity(score) {
                return Err(SemanticMapError::InvalidSimilarity {
                    source_entity_id: inputs[left].entity_id.clone(),
                    target_entity_id: inputs[right].entity_id.clone(),
                    value: score,
                });
            }
            matrix[left][right] = score;
            matrix[right][left] = score;
        }
    }
    Ok(matrix)
}

fn neighborhood_graph<T: ?Sized>(
    inputs: &[SemanticMapInput<'_, T>],
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

fn concept_clusters<T: ?Sized>(
    inputs: &[SemanticMapInput<'_, T>],
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(value: &str) -> EntityId {
        EntityId::new(value).unwrap()
    }

    fn axis_similarity(left: &f32, right: &f32) -> f32 {
        1.0 - (left - right).abs()
    }

    #[test]
    fn clothing_values_form_a_domain_neutral_cluster() {
        let navy_shirt = entity("clothes:navy-shirt");
        let blue_shirt = entity("clothes:blue-shirt");
        let running_shoe = entity("clothes:running-shoe");
        let navy_style = 0.0;
        let blue_style = 0.05;
        let shoe_style = 1.0;
        let inputs = [
            SemanticMapInput {
                entity_id: &navy_shirt,
                value: &navy_style,
            },
            SemanticMapInput {
                entity_id: &blue_shirt,
                value: &blue_style,
            },
            SemanticMapInput {
                entity_id: &running_shoe,
                value: &shoe_style,
            },
        ];

        let map = build_semantic_map_with(
            &inputs,
            SemanticMapOptions {
                neighbors_per_entity: 2,
                neighbor_threshold: 0.5,
                cluster_threshold: 0.8,
            },
            axis_similarity,
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
        let first_value = 0.0;
        let second_value = 0.1;
        let third_value = 1.0;
        let inputs = [
            SemanticMapInput {
                entity_id: &first,
                value: &first_value,
            },
            SemanticMapInput {
                entity_id: &second,
                value: &second_value,
            },
            SemanticMapInput {
                entity_id: &third,
                value: &third_value,
            },
        ];

        let first_map =
            build_semantic_map_with(&inputs, SemanticMapOptions::default(), axis_similarity)
                .unwrap();
        let second_map =
            build_semantic_map_with(&inputs, SemanticMapOptions::default(), axis_similarity)
                .unwrap();

        assert_eq!(first_map, second_map);
    }

    #[test]
    fn stable_entity_ids_break_equal_neighbor_scores() {
        let source = entity("entity:source");
        let alpha = entity("entity:alpha");
        let zeta = entity("entity:zeta");
        let source_value = 0.5;
        let zeta_value = 1.0;
        let alpha_value = 0.0;
        let inputs = [
            SemanticMapInput {
                entity_id: &source,
                value: &source_value,
            },
            SemanticMapInput {
                entity_id: &zeta,
                value: &zeta_value,
            },
            SemanticMapInput {
                entity_id: &alpha,
                value: &alpha_value,
            },
        ];

        let map = build_semantic_map_with(
            &inputs,
            SemanticMapOptions {
                neighbors_per_entity: 1,
                neighbor_threshold: 0.0,
                cluster_threshold: 1.0,
            },
            axis_similarity,
        )
        .unwrap();

        assert!(map
            .neighbors
            .iter()
            .any(|edge| edge.source_entity_id == source && edge.target_entity_id == alpha));
    }

    #[test]
    fn rejects_duplicate_ids_and_invalid_similarity() {
        let repeated = entity("entity:repeated");
        let first_value = 0.0;
        let second_value = 1.0;
        let duplicate_inputs = [
            SemanticMapInput {
                entity_id: &repeated,
                value: &first_value,
            },
            SemanticMapInput {
                entity_id: &repeated,
                value: &second_value,
            },
        ];
        assert!(matches!(
            build_semantic_map_with(
                &duplicate_inputs,
                SemanticMapOptions::default(),
                axis_similarity
            ),
            Err(SemanticMapError::DuplicateEntity { .. })
        ));

        let other = entity("entity:other");
        let invalid_inputs = [
            SemanticMapInput {
                entity_id: &repeated,
                value: &first_value,
            },
            SemanticMapInput {
                entity_id: &other,
                value: &second_value,
            },
        ];
        assert!(matches!(
            build_semantic_map_with(&invalid_inputs, SemanticMapOptions::default(), |_, _| {
                f32::NAN
            }),
            Err(SemanticMapError::InvalidSimilarity { .. })
        ));
    }
}
