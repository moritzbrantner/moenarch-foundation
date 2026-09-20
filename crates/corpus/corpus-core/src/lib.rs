#![doc = include_str!("../README.md")]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Deterministic string metadata used for portable corpus annotations and filters.
pub type Metadata = BTreeMap<String, String>;

/// Errors produced by corpus contract validation helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusContractError {
    /// A typed corpus identifier was empty or whitespace-only.
    EmptyIdentifier {
        /// The identifier family that rejected the value.
        kind: &'static str,
    },
    /// A content hash was not canonical lowercase SHA-256 hexadecimal.
    InvalidSha256,
    /// A half-open range ended before it started.
    InvalidRange {
        /// Inclusive start coordinate.
        start: u64,
        /// Exclusive end coordinate.
        end: u64,
    },
    /// A retrieval query had no usable input.
    InvalidQuery(&'static str),
    /// A dense query vector contained a non-finite value.
    InvalidVectorValue {
        /// Index of the invalid value.
        index: usize,
    },
}

impl Display for CorpusContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyIdentifier { kind } => {
                write!(formatter, "{kind} identifier must not be empty")
            }
            Self::InvalidSha256 => formatter.write_str(
                "content hash must be exactly 64 lowercase hexadecimal SHA-256 characters",
            ),
            Self::InvalidRange { start, end } => {
                write!(formatter, "range end {end} is before start {start}")
            }
            Self::InvalidQuery(message) => write!(formatter, "invalid retrieval query: {message}"),
            Self::InvalidVectorValue { index } => {
                write!(
                    formatter,
                    "dense query vector contains a non-finite value at index {index}"
                )
            }
        }
    }
}

impl Error for CorpusContractError {}

/// Result type for validating and constructing corpus contracts.
pub type ContractResult<T> = std::result::Result<T, CorpusContractError>;

fn push_hex(output: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.reserve(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    push_hex(&mut output, &digest);
    output
}

fn hash_component(hasher: &mut Sha256, component: &[u8]) {
    hasher.update((component.len() as u64).to_be_bytes());
    hasher.update(component);
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
        #[doc = concat!("Stable typed identifier for a corpus ", $kind, ".")]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[doc = concat!("Creates a ", $kind, " identifier from an existing stable value.")]
            pub fn new(value: impl Into<String>) -> ContractResult<Self> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(CorpusContractError::EmptyIdentifier { kind: $kind });
                }
                Ok(Self(value))
            }

            /// Derives a deterministic identifier from length-delimited components.
            ///
            /// Length delimiters make component boundaries significant, so `['ab', 'c']`
            /// and `['a', 'bc']` cannot alias merely through concatenation.
            #[must_use]
            pub fn derive(parts: &[&str]) -> Self {
                Self(derive_identifier($namespace, parts))
            }

            /// Returns the identifier as its stable string representation.
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

typed_id!(SourceId, "source", "source");
typed_id!(AssetId, "asset", "asset");
typed_id!(SegmentId, "segment", "segment");
typed_id!(RepresentationId, "representation", "representation");

/// Canonical lowercase SHA-256 identity of immutable content bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash(String);

impl ContentHash {
    /// Computes a SHA-256 content hash.
    #[must_use]
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Self {
        Self(sha256_hex(bytes.as_ref()))
    }

    /// Parses a canonical lowercase SHA-256 hexadecimal hash.
    pub fn parse(value: impl Into<String>) -> ContractResult<Self> {
        let value = value.into();
        let valid = value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if !valid {
            return Err(CorpusContractError::InvalidSha256);
        }
        Ok(Self(value))
    }

    /// Returns the canonical lowercase hexadecimal value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ContentHash {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Half-open coordinate range `[start, end)` used by segment locators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// Inclusive start coordinate.
    pub start: u64,
    /// Exclusive end coordinate.
    pub end: u64,
}

impl Span {
    /// Creates a validated half-open span.
    pub fn new(start: u64, end: u64) -> ContractResult<Self> {
        if end < start {
            return Err(CorpusContractError::InvalidRange { start, end });
        }
        Ok(Self { start, end })
    }

    /// Returns the span length.
    #[must_use]
    pub const fn len(self) -> u64 {
        self.end - self.start
    }

    /// Returns whether the span is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// Broad source families without binding consumers to one ingestion implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Files or directories on a filesystem.
    FileSystem,
    /// A network URL or URL-addressed feed.
    Url,
    /// S3-compatible or another object store.
    ObjectStore,
    /// A database-backed source.
    Database,
    /// Material supplied directly by a caller.
    Inline,
    /// A named source family not yet standardized by this crate.
    Other(String),
}

/// Broad asset families used across text and media corpora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// Plain or structured textual material.
    Text,
    /// A paged or otherwise document-oriented asset.
    Document,
    /// A still or animated image asset.
    Image,
    /// A video asset.
    Video,
    /// An audio asset.
    Audio,
    /// A subtitle or timed-text asset.
    Subtitle,
    /// A web page captured as one corpus asset.
    WebPage,
    /// A named asset family not yet standardized by this crate.
    Other(String),
}

/// A location plus immutable identity metadata for content bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRef {
    /// Consumer-resolvable URI, path, object key, or other location string.
    pub uri: String,
    /// Exact content identity when bytes are available.
    pub hash: Option<ContentHash>,
    /// Optional MIME media type.
    pub media_type: Option<String>,
    /// Optional exact byte length.
    pub byte_length: Option<u64>,
}

/// Origin from which one or more corpus assets are discovered or imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// Stable source identity.
    pub id: SourceId,
    /// Broad source family.
    pub kind: SourceKind,
    /// Source URI, path, connection label, or another consumer-resolvable locator.
    pub uri: String,
    /// Deterministic source metadata.
    #[serde(default)]
    pub metadata: Metadata,
}

/// One concrete original item in a corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    /// Stable asset identity.
    pub id: AssetId,
    /// Source from which this asset originates.
    pub source_id: SourceId,
    /// Broad asset family.
    pub kind: AssetKind,
    /// Location and content identity of the original asset bytes.
    pub content: ContentRef,
    /// Deterministic asset metadata.
    #[serde(default)]
    pub metadata: Metadata,
}

/// Portable coordinates for a part of an asset.
///
/// Multiple coordinates may be present together, for example a PDF page plus a
/// character range or a video time range plus frame range. `selectors` is the
/// escape hatch for domain coordinates that have not earned a shared typed field.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SegmentLocator {
    /// Optional byte coordinates in the original asset.
    pub byte_range: Option<Span>,
    /// Optional character coordinates in extracted text.
    pub character_range: Option<Span>,
    /// Optional zero-based page index.
    pub page_index: Option<u32>,
    /// Optional millisecond coordinates in timed media.
    pub time_range_ms: Option<Span>,
    /// Optional frame coordinates in video.
    pub frame_range: Option<Span>,
    /// Extensible deterministic selectors such as a named section or spatial region.
    #[serde(default)]
    pub selectors: Metadata,
}

/// Reference to any addressable record in a corpus.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "record_type", content = "id", rename_all = "snake_case")]
pub enum CorpusItemRef {
    /// A source record.
    Source(SourceId),
    /// An asset record.
    Asset(AssetId),
    /// A segment record.
    Segment(SegmentId),
    /// A representation record.
    Representation(RepresentationId),
}

/// Reproducibility metadata for a derived corpus record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Stable operation name such as `transcribe`, `ocr`, or `chunk`.
    pub operation: String,
    /// Tool, model, library, or algorithm that produced the record.
    pub producer: String,
    /// Optional exact producer/model version.
    pub producer_version: Option<String>,
    /// Records used as direct inputs to this derivation.
    #[serde(default)]
    pub inputs: Vec<CorpusItemRef>,
    /// Optional hash of canonicalized processing parameters.
    pub parameters_hash: Option<ContentHash>,
}

/// Addressable part of an asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    /// Stable segment identity.
    pub id: SegmentId,
    /// Asset that owns this segment.
    pub asset_id: AssetId,
    /// Coordinates mapping the segment back to its asset.
    pub locator: SegmentLocator,
    /// Optional derivation metadata for generated segment boundaries.
    pub provenance: Option<Provenance>,
    /// Deterministic segment metadata.
    #[serde(default)]
    pub metadata: Metadata,
}

/// Common derived representation families.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationKind {
    /// Extracted or normalized textual content.
    ExtractedText,
    /// Text produced by optical character recognition.
    OcrText,
    /// Speech transcription or timed transcript data.
    Transcript,
    /// A derived thumbnail or preview image.
    Thumbnail,
    /// A dense or sparse embedding payload.
    Embedding,
    /// A perceptual/content similarity hash.
    PerceptualHash,
    /// A metadata projection or normalized metadata document.
    Metadata,
    /// A named representation family not yet standardized by this crate.
    Other(String),
}

/// Derived, rebuildable material attached to a source, asset, segment, or representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Representation {
    /// Stable representation identity.
    pub id: RepresentationId,
    /// Record this representation describes or derives from.
    pub owner: CorpusItemRef,
    /// Representation family.
    pub kind: RepresentationKind,
    /// Exact identity of the representation payload.
    pub content_hash: ContentHash,
    /// Optional external location for the representation payload.
    pub content: Option<ContentRef>,
    /// Derivation metadata required to understand or rebuild the representation.
    pub provenance: Provenance,
    /// Deterministic representation metadata.
    #[serde(default)]
    pub metadata: Metadata,
}

/// Any persistable corpus record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "record_type", content = "record", rename_all = "snake_case")]
pub enum CorpusRecord {
    /// Source record.
    Source(Source),
    /// Asset record.
    Asset(Asset),
    /// Segment record.
    Segment(Segment),
    /// Derived representation record.
    Representation(Representation),
}

impl CorpusRecord {
    /// Returns the typed reference for this record.
    #[must_use]
    pub fn item_ref(&self) -> CorpusItemRef {
        match self {
            Self::Source(source) => CorpusItemRef::Source(source.id.clone()),
            Self::Asset(asset) => CorpusItemRef::Asset(asset.id.clone()),
            Self::Segment(segment) => CorpusItemRef::Segment(segment.id.clone()),
            Self::Representation(representation) => {
                CorpusItemRef::Representation(representation.id.clone())
            }
        }
    }
}

/// Minimal persistence seam for corpus metadata and references.
///
/// Concrete implementations may use SQLite, Postgres, object storage, browser
/// persistence, or another backend. Payload bytes referenced by [`ContentRef`]
/// do not have to live in the same store.
pub trait CorpusStore {
    /// Backend-specific error type.
    type Error: Error + Send + Sync + 'static;

    /// Inserts or replaces one record by its stable typed identity.
    fn upsert(&mut self, record: CorpusRecord) -> std::result::Result<(), Self::Error>;

    /// Loads one record by typed identity.
    fn get(&self, id: &CorpusItemRef) -> std::result::Result<Option<CorpusRecord>, Self::Error>;

    /// Deletes one record and reports whether a record existed.
    fn delete(&mut self, id: &CorpusItemRef) -> std::result::Result<bool, Self::Error>;
}

/// Broad retrieval strategies that multiple search backends can implement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalStrategy {
    /// Lexical/full-text retrieval.
    Lexical,
    /// Semantic/vector retrieval.
    Semantic,
    /// Combined lexical and semantic retrieval.
    Hybrid,
    /// Media/content similarity retrieval.
    Similarity,
    /// Exact identifier/hash/value matching.
    Exact,
    /// A named strategy not yet standardized by this crate.
    Other(String),
}

/// Portable retrieval query input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "input_type", content = "value", rename_all = "snake_case")]
pub enum RetrievalInput {
    /// Textual query supplied by the caller.
    Text(String),
    /// Existing corpus item used as the query object.
    Item(CorpusItemRef),
    /// Dense numeric query representation supplied by an analysis/embedding layer.
    DenseVector(Vec<f32>),
}

/// Backend-neutral retrieval request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalQuery {
    /// Query material.
    pub input: RetrievalInput,
    /// Requested retrieval strategy.
    pub strategy: RetrievalStrategy,
    /// Maximum number of hits to return.
    pub limit: usize,
    /// Simple deterministic metadata equality filters.
    #[serde(default)]
    pub filters: Metadata,
}

impl RetrievalQuery {
    /// Creates a validated retrieval query.
    pub fn new(
        input: RetrievalInput,
        strategy: RetrievalStrategy,
        limit: usize,
    ) -> ContractResult<Self> {
        if limit == 0 {
            return Err(CorpusContractError::InvalidQuery(
                "result limit must be greater than zero",
            ));
        }
        match &input {
            RetrievalInput::Text(text) if text.trim().is_empty() => {
                return Err(CorpusContractError::InvalidQuery(
                    "text input must not be empty",
                ));
            }
            RetrievalInput::DenseVector(values) if values.is_empty() => {
                return Err(CorpusContractError::InvalidQuery(
                    "dense vector input must not be empty",
                ));
            }
            RetrievalInput::DenseVector(values) => {
                if let Some(index) = values.iter().position(|value| !value.is_finite()) {
                    return Err(CorpusContractError::InvalidVectorValue { index });
                }
            }
            RetrievalInput::Text(_) | RetrievalInput::Item(_) => {}
        }
        Ok(Self {
            input,
            strategy,
            limit,
            filters: Metadata::new(),
        })
    }
}

/// One ranked retrieval result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalHit {
    /// Matched corpus record.
    pub item: CorpusItemRef,
    /// Zero-based portable ordering position assigned by the retriever.
    pub rank: usize,
    /// Optional backend-defined score. Its scale is not portable across retrievers.
    pub raw_score: Option<f32>,
    /// Segments that explain or localize the match when available.
    #[serde(default)]
    pub matched_segments: Vec<SegmentId>,
    /// Deterministic hit metadata such as backend-specific score components.
    #[serde(default)]
    pub metadata: Metadata,
}

impl RetrievalHit {
    /// Creates an unscored ranked hit.
    #[must_use]
    pub fn new(item: CorpusItemRef, rank: usize) -> Self {
        Self {
            item,
            rank,
            raw_score: None,
            matched_segments: Vec::new(),
            metadata: Metadata::new(),
        }
    }
}

/// Minimal search seam over one or more corpus indexes.
pub trait Retriever {
    /// Backend-specific error type.
    type Error: Error + Send + Sync + 'static;

    /// Returns hits in the retriever's canonical ranked order.
    fn retrieve(
        &self,
        query: &RetrievalQuery,
    ) -> std::result::Result<Vec<RetrievalHit>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn content_hash_is_canonical_sha256() {
        let hash = ContentHash::from_bytes(b"hello");
        assert_eq!(
            hash.as_str(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(ContentHash::parse(hash.to_string()).unwrap(), hash);
        assert!(ContentHash::parse("ABC").is_err());
    }

    #[test]
    fn derived_ids_preserve_component_boundaries() {
        let first = AssetId::derive(&["ab", "c"]);
        let second = AssetId::derive(&["a", "bc"]);
        assert_ne!(first, second);
        assert_eq!(first, AssetId::derive(&["ab", "c"]));
        assert!(AssetId::new("   ").is_err());
    }

    #[test]
    fn spans_are_half_open_and_validated() {
        let span = Span::new(10, 15).unwrap();
        assert_eq!(span.len(), 5);
        assert!(!span.is_empty());
        assert!(Span::new(15, 10).is_err());
    }

    #[test]
    fn serde_round_trip_preserves_corpus_relationships_and_provenance() {
        let source_id = SourceId::derive(&["file:///corpus"]);
        let asset_id = AssetId::derive(&[source_id.as_str(), "lecture.mp4"]);
        let segment_id = SegmentId::derive(&[asset_id.as_str(), "0", "30000"]);
        let representation = Representation {
            id: RepresentationId::derive(&[segment_id.as_str(), "transcript", "v1"]),
            owner: CorpusItemRef::Segment(segment_id.clone()),
            kind: RepresentationKind::Transcript,
            content_hash: ContentHash::from_bytes(b"hello world"),
            content: None,
            provenance: Provenance {
                operation: "transcribe".into(),
                producer: "fixture-asr".into(),
                producer_version: Some("1.0.0".into()),
                inputs: vec![CorpusItemRef::Segment(segment_id.clone())],
                parameters_hash: Some(ContentHash::from_bytes(b"language=en")),
            },
            metadata: Metadata::new(),
        };
        let record = CorpusRecord::Representation(representation);

        let json = serde_json::to_string(&record).unwrap();
        let decoded: CorpusRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, record);
        assert!(matches!(
            decoded.item_ref(),
            CorpusItemRef::Representation(_)
        ));
    }

    #[derive(Debug)]
    struct FixtureError;

    impl Display for FixtureError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("fixture error")
        }
    }

    impl Error for FixtureError {}

    #[derive(Default)]
    struct MemoryStore {
        records: HashMap<CorpusItemRef, CorpusRecord>,
    }

    impl CorpusStore for MemoryStore {
        type Error = FixtureError;

        fn upsert(&mut self, record: CorpusRecord) -> std::result::Result<(), Self::Error> {
            self.records.insert(record.item_ref(), record);
            Ok(())
        }

        fn get(
            &self,
            id: &CorpusItemRef,
        ) -> std::result::Result<Option<CorpusRecord>, Self::Error> {
            Ok(self.records.get(id).cloned())
        }

        fn delete(&mut self, id: &CorpusItemRef) -> std::result::Result<bool, Self::Error> {
            Ok(self.records.remove(id).is_some())
        }
    }

    #[test]
    fn corpus_store_trait_is_backend_neutral() {
        let source = Source {
            id: SourceId::derive(&["https://example.test/feed"]),
            kind: SourceKind::Url,
            uri: "https://example.test/feed".into(),
            metadata: Metadata::new(),
        };
        let record = CorpusRecord::Source(source);
        let id = record.item_ref();
        let mut store = MemoryStore::default();

        store.upsert(record.clone()).unwrap();
        assert_eq!(store.get(&id).unwrap(), Some(record));
        assert!(store.delete(&id).unwrap());
        assert_eq!(store.get(&id).unwrap(), None);
    }

    struct FixtureRetriever;

    impl Retriever for FixtureRetriever {
        type Error = FixtureError;

        fn retrieve(
            &self,
            query: &RetrievalQuery,
        ) -> std::result::Result<Vec<RetrievalHit>, Self::Error> {
            Ok(vec![RetrievalHit::new(
                CorpusItemRef::Asset(AssetId::derive(&["fixture", &query.limit.to_string()])),
                0,
            )])
        }
    }

    #[test]
    fn retrieval_contract_validates_inputs_and_preserves_rank() {
        let query = RetrievalQuery::new(
            RetrievalInput::Text("rust corpus".into()),
            RetrievalStrategy::Hybrid,
            10,
        )
        .unwrap();
        let hits = FixtureRetriever.retrieve(&query).unwrap();
        assert_eq!(hits[0].rank, 0);

        assert!(RetrievalQuery::new(
            RetrievalInput::Text("  ".into()),
            RetrievalStrategy::Lexical,
            10,
        )
        .is_err());
        assert!(RetrievalQuery::new(
            RetrievalInput::DenseVector(vec![0.0, f32::NAN]),
            RetrievalStrategy::Semantic,
            10,
        )
        .is_err());
        assert!(RetrievalQuery::new(
            RetrievalInput::Text("x".into()),
            RetrievalStrategy::Exact,
            0,
        )
        .is_err());
    }
}
