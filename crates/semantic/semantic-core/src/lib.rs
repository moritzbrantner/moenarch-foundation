#![doc = include_str!("../README.md")]

pub mod map;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Errors produced while constructing or deserializing semantic contracts.
#[derive(Debug, Clone, PartialEq)]
pub enum SemanticContractError {
    /// A stable identifier was empty or whitespace-only.
    EmptyIdentifier {
        /// Identifier family that rejected the value.
        kind: &'static str,
    },
    /// A confidence value was NaN or infinite.
    NonFiniteConfidence,
    /// A finite confidence value fell outside the inclusive `[0, 1]` range.
    ConfidenceOutOfRange {
        /// Rejected confidence value.
        value: f64,
    },
    /// A numeric evidence value was NaN or infinite.
    NonFiniteNumber,
}

impl Display for SemanticContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyIdentifier { kind } => {
                write!(formatter, "{kind} identifier must not be empty")
            }
            Self::NonFiniteConfidence => formatter.write_str("confidence must be finite"),
            Self::ConfidenceOutOfRange { value } => {
                write!(formatter, "confidence must be in [0, 1], got {value}")
            }
            Self::NonFiniteNumber => formatter.write_str("numeric evidence must be finite"),
        }
    }
}

impl Error for SemanticContractError {}

/// Result type for semantic contract validation.
pub type ContractResult<T> = std::result::Result<T, SemanticContractError>;

fn hash_component(hasher: &mut Sha256, component: &[u8]) {
    hasher.update((component.len() as u64).to_be_bytes());
    hasher.update(component);
}

fn push_hex(output: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.reserve(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
}

fn derive_identifier(namespace: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, namespace.as_bytes());
    for part in parts {
        hash_component(&mut hasher, part.as_bytes());
    }
    let digest = hasher.finalize();
    let mut value = String::with_capacity(namespace.len() + 1 + 64);
    value.push_str(namespace);
    value.push(':');
    push_hex(&mut value, &digest);
    value
}

macro_rules! typed_id {
    ($name:ident, $namespace:literal, $kind:literal) => {
        #[doc = concat!("Stable typed identifier for a semantic ", $kind, ".")]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[doc = concat!("Creates a ", $kind, " identifier from an existing stable value.")]
            pub fn new(value: impl Into<String>) -> ContractResult<Self> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(SemanticContractError::EmptyIdentifier { kind: $kind });
                }
                Ok(Self(value))
            }

            /// Derives a deterministic identifier from length-delimited components.
            ///
            /// Length delimiters make component boundaries significant, so `['ab', 'c']`
            /// and `['a', 'bc']` cannot alias through concatenation.
            #[must_use]
            pub fn derive(parts: &[&str]) -> Self {
                Self(derive_identifier($namespace, parts))
            }

            /// Returns the stable string representation.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

typed_id!(EntityId, "entity", "entity");
typed_id!(ConceptId, "concept", "concept");
typed_id!(ProducerId, "producer", "producer");
typed_id!(EvidenceKey, "evidence-key", "evidence feature");
typed_id!(EvidenceRef, "evidence-ref", "evidence reference");

/// Validated confidence in the inclusive range `[0, 1]`.
///
/// Confidence is intentionally optional in claim types. Producers that do not
/// have a calibrated score should leave it absent instead of inventing one.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Confidence(f64);

impl Confidence {
    /// Creates a validated confidence value.
    pub fn new(value: f64) -> ContractResult<Self> {
        if !value.is_finite() {
            return Err(SemanticContractError::NonFiniteConfidence);
        }
        if !(0.0..=1.0).contains(&value) {
            return Err(SemanticContractError::ConfidenceOutOfRange { value });
        }
        Ok(Self(value))
    }

    /// Returns the underlying finite value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Confidence {
    type Error = SemanticContractError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Confidence> for f64 {
    fn from(value: Confidence) -> Self {
        value.0
    }
}

/// Finite numeric evidence without a range restriction.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct FiniteNumber(f64);

impl FiniteNumber {
    /// Creates a finite numeric evidence value.
    pub fn new(value: f64) -> ContractResult<Self> {
        if !value.is_finite() {
            return Err(SemanticContractError::NonFiniteNumber);
        }
        Ok(Self(value))
    }

    /// Returns the underlying finite value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for FiniteNumber {
    type Error = SemanticContractError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<FiniteNumber> for f64 {
    fn from(value: FiniteNumber) -> Self {
        value.0
    }
}

/// Producer that emitted a semantic claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Producer {
    /// Stable rule, model, algorithm, or tool identity.
    pub id: ProducerId,
    /// Optional exact producer/model/rule-set version.
    pub version: Option<String>,
}

impl Producer {
    /// Creates an unversioned producer identity.
    #[must_use]
    pub const fn new(id: ProducerId) -> Self {
        Self { id, version: None }
    }

    /// Attaches an exact version string to this producer.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
}

/// Typed scalar value captured as semantic evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum EvidenceValue {
    /// Textual observation.
    Text(String),
    /// Boolean observation.
    Boolean(bool),
    /// Signed integer observation.
    Integer(i64),
    /// Finite floating-point observation.
    Number(FiniteNumber),
}

/// Inspectable evidence supporting a semantic claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Evidence {
    /// A named typed observation produced from the subject or its context.
    Observation {
        /// Stable feature/observation identity.
        feature: EvidenceKey,
        /// Observed value.
        value: EvidenceValue,
    },
    /// An opaque stable reference to source evidence owned elsewhere.
    Reference {
        /// Stable reference identity or locator.
        reference: EvidenceRef,
        /// Optional consumer-defined selector within the referenced source.
        selector: Option<String>,
    },
}

/// Reference to a node that can participate in a semantic relation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "node_type", content = "id", rename_all = "snake_case")]
pub enum SemanticNodeRef {
    /// A concrete entity.
    Entity(EntityId),
    /// A reusable concept.
    Concept(ConceptId),
}

/// Evidence-backed claim that an entity is associated with a concept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    /// Entity being annotated.
    pub subject: EntityId,
    /// Concept asserted about the entity.
    pub concept: ConceptId,
    /// Optional calibrated confidence.
    pub confidence: Option<Confidence>,
    /// Optional producer identity.
    pub producer: Option<Producer>,
    /// Inspectable supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// Directional evidence-backed semantic relationship.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    /// Source endpoint of the relationship.
    pub subject: SemanticNodeRef,
    /// Predicate concept naming the relationship semantics.
    pub predicate: ConceptId,
    /// Target endpoint of the relationship.
    pub object: SemanticNodeRef,
    /// Optional calibrated confidence.
    pub confidence: Option<Confidence>,
    /// Optional producer identity.
    pub producer: Option<Producer>,
    /// Inspectable supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_reject_empty_values_and_derive_without_component_aliasing() {
        assert!(matches!(
            EntityId::new("   "),
            Err(SemanticContractError::EmptyIdentifier { kind: "entity" })
        ));
        assert_ne!(
            ConceptId::derive(&["ab", "c"]),
            ConceptId::derive(&["a", "bc"])
        );
        assert_eq!(
            ConceptId::derive(&["document", "page_number"]),
            ConceptId::derive(&["document", "page_number"])
        );
    }

    #[test]
    fn confidence_is_finite_and_bounded_including_deserialization() {
        assert_eq!(Confidence::new(0.0).unwrap().get(), 0.0);
        assert_eq!(Confidence::new(1.0).unwrap().get(), 1.0);
        assert!(matches!(
            Confidence::new(f64::NAN),
            Err(SemanticContractError::NonFiniteConfidence)
        ));
        assert!(matches!(
            Confidence::new(f64::INFINITY),
            Err(SemanticContractError::NonFiniteConfidence)
        ));
        assert!(matches!(
            Confidence::new(-0.01),
            Err(SemanticContractError::ConfidenceOutOfRange { .. })
        ));
        assert!(matches!(
            Confidence::new(1.01),
            Err(SemanticContractError::ConfidenceOutOfRange { .. })
        ));
        assert!(serde_json::from_str::<Confidence>("1.01").is_err());
    }

    #[test]
    fn numeric_evidence_rejects_non_finite_values() {
        assert_eq!(FiniteNumber::new(-42.5).unwrap().get(), -42.5);
        assert!(matches!(
            FiniteNumber::new(f64::NEG_INFINITY),
            Err(SemanticContractError::NonFiniteNumber)
        ));
    }

    #[test]
    fn annotation_round_trip_preserves_typed_evidence() {
        let annotation = Annotation {
            subject: EntityId::new("ocr:block:481").unwrap(),
            concept: ConceptId::new("document:page_number").unwrap(),
            confidence: Some(Confidence::new(0.99).unwrap()),
            producer: Some(
                Producer::new(ProducerId::new("speedreader:margin-detector").unwrap())
                    .with_version("1"),
            ),
            evidence: vec![
                Evidence::Observation {
                    feature: EvidenceKey::new("relative_y").unwrap(),
                    value: EvidenceValue::Number(FiniteNumber::new(0.973).unwrap()),
                },
                Evidence::Observation {
                    feature: EvidenceKey::new("sequential_number_pattern").unwrap(),
                    value: EvidenceValue::Boolean(true),
                },
                Evidence::Reference {
                    reference: EvidenceRef::new("ocr:page:12:block:481").unwrap(),
                    selector: Some("line:0".to_owned()),
                },
            ],
        };

        let encoded = serde_json::to_string(&annotation).unwrap();
        let decoded: Annotation = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, annotation);
    }

    #[test]
    fn overlapping_annotations_do_not_require_a_single_category() {
        let subject = EntityId::new("track:example").unwrap();
        let claims = [
            Annotation {
                subject: subject.clone(),
                concept: ConceptId::new("genre:dream_pop").unwrap(),
                confidence: Some(Confidence::new(0.72).unwrap()),
                producer: None,
                evidence: vec![],
            },
            Annotation {
                subject,
                concept: ConceptId::new("genre:shoegaze").unwrap(),
                confidence: Some(Confidence::new(0.64).unwrap()),
                producer: None,
                evidence: vec![],
            },
        ];

        assert_eq!(claims.len(), 2);
        assert_ne!(claims[0].concept, claims[1].concept);
    }

    #[test]
    fn relations_support_entity_and_concept_endpoints() {
        let relation = Relation {
            subject: SemanticNodeRef::Entity(EntityId::new("product:42").unwrap()),
            predicate: ConceptId::new("relation:is_a").unwrap(),
            object: SemanticNodeRef::Concept(ConceptId::new("product:hiking_boot").unwrap()),
            confidence: Some(Confidence::new(0.93).unwrap()),
            producer: None,
            evidence: vec![],
        };

        let encoded = serde_json::to_string(&relation).unwrap();
        let decoded: Relation = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, relation);
    }
}
