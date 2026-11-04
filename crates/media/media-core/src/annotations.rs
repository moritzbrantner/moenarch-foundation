//! Media-neutral annotation envelopes located on an exact media timeline.
//!
//! Domain-specific findings remain owned by their audio, text, image, vision,
//! video, and spatial consumers. This module provides the small
//! interoperability envelope that lets those findings coexist on one exact
//! media timeline: *where* in a source (`AnnotationSelector`), *when*
//! (`AnnotationTiming`), *what* (`kind`, `label`, `AnnotationValue`), and *who
//! produced it* (`AnnotationProvenance`).
//!
//! Semantic claims about entities and concepts (`semantic-core`'s
//! `Annotation`) are a different layer; see the crate README for the mapping.
//! Dataset storage, persistence formats, and legacy-dataset adapters are owned
//! by consumers and are deliberately not part of this module.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{AnalysisEvent, DetectError, MediaRange, Timestamp};

pub use crate::MediaSourceRef;

/// Errors produced by annotation validation and timing operations.
///
/// The enum is `#[non_exhaustive]`: downstream `match` expressions need a
/// wildcard arm so later variants are not breaking changes.
#[derive(Debug)]
#[non_exhaustive]
pub enum AnnotationError {
    /// Annotation data violates the neutral contract.
    Invalid(String),
    /// Shared media timing validation failed.
    Media(DetectError),
}

impl fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "invalid annotation data: {message}"),
            Self::Media(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl std::error::Error for AnnotationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Invalid(_) => None,
            Self::Media(error) => error.source(),
        }
    }
}

impl From<DetectError> for AnnotationError {
    fn from(error: DetectError) -> Self {
        Self::Media(error)
    }
}

/// Result type for media annotation operations.
pub type Result<T> = std::result::Result<T, AnnotationError>;

impl MediaSourceRef {
    /// Creates a source reference with a source id.
    pub fn source(source_id: impl Into<String>) -> Self {
        Self {
            source_id: Some(source_id.into()),
            ..Self::default()
        }
    }

    /// Adds a URI or external locator.
    pub fn uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    /// Adds a caller-defined source kind such as `video`, `audio`, or `transcript`.
    pub fn source_kind(mut self, source_kind: impl Into<String>) -> Self {
        self.source_kind = Some(source_kind.into());
        self
    }

    /// Adds source-specific metadata.
    pub fn attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Validates that populated identifiers and attribute keys are non-empty.
    pub fn validate(&self) -> Result<()> {
        validate_optional_non_empty(self.source_id.as_deref(), "source id")?;
        validate_optional_non_empty(self.uri.as_deref(), "source uri")?;
        validate_optional_non_empty(self.source_kind.as_deref(), "source kind")?;
        validate_attribute_keys(&self.attributes, "source attribute")
    }
}

/// Provenance describing how an annotation was produced.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationProvenance {
    /// Analyzer or producer name.
    #[serde(default)]
    pub analyzer: Option<String>,
    /// Operation or pipeline stage name.
    #[serde(default)]
    pub operation: Option<String>,
    /// Model identifier, when a model produced the result.
    #[serde(default)]
    pub model_id: Option<String>,
    /// Runtime or backend identifier.
    #[serde(default)]
    pub runtime: Option<String>,
    /// Producer or model version.
    #[serde(default)]
    pub version: Option<String>,
}

impl AnnotationProvenance {
    /// Creates provenance for an analyzer.
    pub fn analyzer(analyzer: impl Into<String>) -> Self {
        Self {
            analyzer: Some(analyzer.into()),
            ..Self::default()
        }
    }

    /// Validates all populated provenance fields.
    pub fn validate(&self) -> Result<()> {
        validate_optional_non_empty(self.analyzer.as_deref(), "provenance analyzer")?;
        validate_optional_non_empty(self.operation.as_deref(), "provenance operation")?;
        validate_optional_non_empty(self.model_id.as_deref(), "provenance model id")?;
        validate_optional_non_empty(self.runtime.as_deref(), "provenance runtime")?;
        validate_optional_non_empty(self.version.as_deref(), "provenance version")
    }
}

/// Domain-neutral selector locating an annotation within a media source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnnotationSelector {
    /// A video-frame index.
    Frame {
        /// Zero-based frame index.
        frame_index: u64,
    },
    /// A text segment within a timed or untimed stream.
    TextSegment {
        /// Text stream identifier.
        stream_id: String,
        /// Stable segment index within the stream.
        segment_index: u64,
    },
    /// A half-open UTF-8 text span.
    TextSpan {
        /// Inclusive byte offset.
        byte_start: usize,
        /// Exclusive byte offset.
        byte_end: usize,
        /// Optional inclusive Unicode-scalar offset.
        #[serde(default)]
        char_start: Option<usize>,
        /// Optional exclusive Unicode-scalar offset.
        #[serde(default)]
        char_end: Option<usize>,
    },
    /// A two-dimensional region in a caller-declared coordinate space.
    Region2d {
        /// Horizontal origin.
        x: f64,
        /// Vertical origin.
        y: f64,
        /// Region width.
        width: f64,
        /// Region height.
        height: f64,
        /// Coordinate-space identifier such as `pixels` or `normalized`.
        #[serde(default)]
        coordinate_space: Option<String>,
    },
    /// A stable track identifier.
    Track {
        /// Track identifier.
        track_id: String,
    },
    /// A domain extension that remains opaque to the neutral layer.
    Custom {
        /// Selector kind owned by the domain adapter.
        selector_kind: String,
        /// Structured selector fields.
        #[serde(default)]
        fields: BTreeMap<String, serde_json::Value>,
    },
}

impl AnnotationSelector {
    /// Validates selector ranges, dimensions, and identifiers.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Frame { .. } => Ok(()),
            Self::TextSegment { stream_id, .. } => {
                validate_non_empty(stream_id, "text segment stream id")
            }
            Self::TextSpan {
                byte_start,
                byte_end,
                char_start,
                char_end,
            } => {
                if byte_end < byte_start {
                    return Err(invalid("text span byte_end must not precede byte_start"));
                }
                match (char_start, char_end) {
                    (Some(start), Some(end)) if end < start => {
                        Err(invalid("text span char_end must not precede char_start"))
                    }
                    (Some(_), None) | (None, Some(_)) => Err(invalid(
                        "text span char_start and char_end must be provided together",
                    )),
                    _ => Ok(()),
                }
            }
            Self::Region2d {
                x,
                y,
                width,
                height,
                coordinate_space,
            } => {
                if [*x, *y, *width, *height]
                    .into_iter()
                    .any(|value| !value.is_finite())
                    || *width <= 0.0
                    || *height <= 0.0
                {
                    return Err(invalid(
                        "2D regions must be finite with positive width and height",
                    ));
                }
                validate_optional_non_empty(coordinate_space.as_deref(), "region coordinate space")
            }
            Self::Track { track_id } => validate_non_empty(track_id, "track id"),
            Self::Custom {
                selector_kind,
                fields,
            } => {
                validate_non_empty(selector_kind, "custom selector kind")?;
                for key in fields.keys() {
                    validate_non_empty(key, "custom selector field")?;
                }
                Ok(())
            }
        }
    }
}

/// Exact temporal anchor for an annotation.
///
/// The wire shape is `{ "kind": "instant", "timestamp": <Timestamp> }` or
/// `{ "kind": "range", "start": <Timestamp>, "end": <Timestamp> }`, using the
/// crate's canonical [`Timestamp`] encoding. Deserialization validates the
/// timebases and the half-open range ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationTiming {
    /// A finding at one exact media instant.
    Instant(Timestamp),
    /// A finding spanning a half-open media range.
    Range(MediaRange),
}

impl AnnotationTiming {
    /// Validates the underlying media timing.
    pub fn validate(self) -> Result<()> {
        match self {
            Self::Instant(timestamp) => timestamp.validate()?,
            Self::Range(range) => range.validate()?,
        }
        Ok(())
    }

    /// Returns the first timestamp represented by this timing.
    pub const fn start_timestamp(self) -> Timestamp {
        match self {
            Self::Instant(timestamp) => timestamp,
            Self::Range(range) => range.start,
        }
    }

    /// Returns whether this timing contains an exact timestamp.
    pub fn contains(self, timestamp: Timestamp) -> Result<bool> {
        match self {
            Self::Instant(instant) => Ok(instant.same_instant(timestamp)?),
            Self::Range(range) => Ok(range.contains(timestamp)?),
        }
    }

    /// Returns whether this timing intersects a half-open query range.
    pub fn overlaps(self, range: MediaRange) -> Result<bool> {
        match self {
            Self::Instant(timestamp) => Ok(range.contains(timestamp)?),
            Self::Range(annotation_range) => Ok(annotation_range.overlaps(range)?),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum AnnotationTimingWire {
    Instant { timestamp: Timestamp },
    Range { start: Timestamp, end: Timestamp },
}

impl Serialize for AnnotationTiming {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            Self::Instant(timestamp) => AnnotationTimingWire::Instant { timestamp },
            Self::Range(range) => AnnotationTimingWire::Range {
                start: range.start,
                end: range.end,
            },
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AnnotationTiming {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error as _;

        match AnnotationTimingWire::deserialize(deserializer)? {
            AnnotationTimingWire::Instant { timestamp } => {
                timestamp.validate().map_err(D::Error::custom)?;
                Ok(Self::Instant(timestamp))
            }
            AnnotationTimingWire::Range { start, end } => MediaRange::new(start, end)
                .map(Self::Range)
                .map_err(D::Error::custom),
        }
    }
}

/// Typed or structured value carried by an annotation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum AnnotationValue {
    /// Human-readable text.
    Text(String),
    /// Floating-point feature or measurement.
    Number(f64),
    /// Integer feature or measurement.
    Integer(i64),
    /// Boolean finding.
    Boolean(bool),
    /// Collection of labels or categories.
    Labels(Vec<String>),
    /// Domain-specific structured payload that stays opaque to this layer.
    Json(serde_json::Value),
}

impl AnnotationValue {
    /// Validates finite numeric values and non-empty labels.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Number(value) if !value.is_finite() => {
                Err(invalid("annotation numeric values must be finite"))
            }
            Self::Labels(labels) => {
                for label in labels {
                    validate_non_empty(label, "annotation value label")?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// A neutral finding that can coexist with annotations from any media domain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAnnotation {
    /// Stable identifier within the consumer's annotation collection.
    pub id: String,
    /// Stable machine-readable annotation kind.
    pub kind: String,
    /// Optional human-readable or detector-provided label.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional exact temporal location.
    #[serde(default)]
    pub timing: Option<AnnotationTiming>,
    /// Optional source identity.
    #[serde(default)]
    pub source: Option<MediaSourceRef>,
    /// Optional location within the source.
    #[serde(default)]
    pub selector: Option<AnnotationSelector>,
    /// Optional finite analyzer score. The neutral layer does not assume a `0..=1` scale.
    #[serde(default)]
    pub score: Option<f32>,
    /// Producer/model provenance entries.
    #[serde(default)]
    pub provenance: Vec<AnnotationProvenance>,
    /// Optional typed or structured result payload.
    #[serde(default)]
    pub value: Option<AnnotationValue>,
    /// Domain- or adapter-specific string metadata.
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

impl MediaAnnotation {
    /// Creates an untimed annotation with no optional metadata.
    pub fn new(id: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            label: None,
            timing: None,
            source: None,
            selector: None,
            score: None,
            provenance: Vec::new(),
            value: None,
            attributes: BTreeMap::new(),
        }
    }

    /// Adds a label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Anchors the annotation at one instant.
    pub fn at(mut self, timestamp: Timestamp) -> Self {
        self.timing = Some(AnnotationTiming::Instant(timestamp));
        self
    }

    /// Anchors the annotation to a half-open range.
    pub fn during(mut self, range: MediaRange) -> Self {
        self.timing = Some(AnnotationTiming::Range(range));
        self
    }

    /// Adds source identity.
    pub fn source(mut self, source: MediaSourceRef) -> Self {
        self.source = Some(source);
        self
    }

    /// Adds a source selector.
    pub fn selector(mut self, selector: AnnotationSelector) -> Self {
        self.selector = Some(selector);
        self
    }

    /// Adds an analyzer score.
    pub fn score(mut self, score: f32) -> Self {
        self.score = Some(score);
        self
    }

    /// Adds producer provenance.
    pub fn provenance(mut self, provenance: AnnotationProvenance) -> Self {
        self.provenance.push(provenance);
        self
    }

    /// Adds a typed or structured value.
    pub fn value(mut self, value: AnnotationValue) -> Self {
        self.value = Some(value);
        self
    }

    /// Adds adapter-specific metadata.
    pub fn attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Returns the annotation's first timestamp, when timed.
    pub fn start_timestamp(&self) -> Option<Timestamp> {
        self.timing.map(AnnotationTiming::start_timestamp)
    }

    /// Validates the complete annotation envelope.
    pub fn validate(&self) -> Result<()> {
        validate_non_empty(&self.id, "annotation id")?;
        validate_non_empty(&self.kind, "annotation kind")?;
        validate_optional_non_empty(self.label.as_deref(), "annotation label")?;
        if let Some(timing) = self.timing {
            timing.validate()?;
        }
        if let Some(source) = &self.source {
            source.validate()?;
        }
        if let Some(selector) = &self.selector {
            selector.validate()?;
        }
        if self.score.is_some_and(|score| !score.is_finite()) {
            return Err(invalid("annotation score must be finite"));
        }
        for provenance in &self.provenance {
            provenance.validate()?;
        }
        if let Some(value) = &self.value {
            value.validate()?;
        }
        validate_attribute_keys(&self.attributes, "annotation attribute")
    }

    /// Validates and returns this annotation.
    pub fn validated(self) -> Result<Self> {
        self.validate()?;
        Ok(self)
    }

    /// Converts the neutral [`AnalysisEvent`] into the annotation envelope.
    pub fn from_analysis_event(id: impl Into<String>, event: AnalysisEvent) -> Result<Self> {
        let AnalysisEvent {
            timestamp,
            analyzer,
            label,
            score,
        } = event;
        let mut annotation = Self::new(id, "analysis_event")
            .label(label)
            .provenance(AnnotationProvenance::analyzer(analyzer));
        if let Some(timestamp) = timestamp {
            annotation = annotation.at(timestamp);
        }
        if let Some(score) = score {
            annotation = annotation.score(score);
        }
        annotation.validated()
    }
}

fn invalid(message: &str) -> AnnotationError {
    AnnotationError::Invalid(message.to_string())
}

fn validate_non_empty(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(AnnotationError::Invalid(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(())
    }
}

fn validate_optional_non_empty(value: Option<&str>, field: &str) -> Result<()> {
    if let Some(value) = value {
        validate_non_empty(value, field)?;
    }
    Ok(())
}

fn validate_attribute_keys(attributes: &BTreeMap<String, String>, field: &str) -> Result<()> {
    for key in attributes.keys() {
        validate_non_empty(key, field)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Timebase;

    fn timestamp(pts: i64, num: i32, den: i32) -> Timestamp {
        Timestamp::try_new(pts, Timebase::try_new(num, den).unwrap()).unwrap()
    }

    fn region() -> AnnotationSelector {
        AnnotationSelector::Region2d {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 120.0,
            coordinate_space: Some("pixels".to_string()),
        }
    }

    #[test]
    fn json_round_trip_preserves_exact_range_selector_and_value() {
        let annotation = MediaAnnotation::new("face-1", "face")
            .label("Alice")
            .during(MediaRange::new(timestamp(500, 1, 1_000), timestamp(2, 1, 1)).unwrap())
            .source(
                MediaSourceRef::source("clip-7")
                    .source_kind("video")
                    .uri("file:///clip-7.mp4")
                    .attribute("camera", "a"),
            )
            .selector(region())
            .score(0.95)
            .provenance(AnnotationProvenance::analyzer("face-detector"))
            .value(AnnotationValue::Text("Alice".to_string()))
            .attribute("reviewed", "false")
            .validated()
            .unwrap();

        let encoded = serde_json::to_value(&annotation).unwrap();
        assert_eq!(
            encoded["timing"],
            serde_json::json!({
                "kind": "range",
                "start": { "pts": "500", "timebase": { "num": 1, "den": 1_000 } },
                "end": { "pts": "2", "timebase": { "num": 1, "den": 1 } }
            })
        );
        assert_eq!(encoded["selector"]["kind"], "region2d");
        assert_eq!(encoded["source"]["sourceKind"], "video");
        assert_eq!(
            encoded["value"],
            serde_json::json!({ "type": "text", "value": "Alice" })
        );

        let decoded = serde_json::from_value::<MediaAnnotation>(encoded).unwrap();
        assert_eq!(decoded, annotation);
    }

    #[test]
    fn legacy_rust_packages_wire_still_deserializes() {
        let legacy = serde_json::json!({
            "id": "note-1",
            "kind": "spatial_note",
            "label": "door",
            "timing": {
                "kind": "instant",
                "timestamp": { "pts": 48, "timebase": { "num": 1, "den": 24 } }
            },
            "source": { "sourceId": "clip.mp4", "streamId": null, "sourceKind": "video" },
            "selector": { "kind": "frame", "frame_index": 48 },
            "provenance": [{ "analyzer": "explorer", "operation": "manual" }],
            "value": { "type": "text", "value": "note" }
        });
        let decoded = serde_json::from_value::<MediaAnnotation>(legacy)
            .unwrap()
            .validated()
            .unwrap();
        assert!(decoded
            .start_timestamp()
            .unwrap()
            .same_instant(timestamp(2, 1, 1))
            .unwrap());
        assert_eq!(
            decoded.source,
            Some(MediaSourceRef::source("clip.mp4").source_kind("video"))
        );
        assert_eq!(
            decoded.selector,
            Some(AnnotationSelector::Frame { frame_index: 48 })
        );
    }

    #[test]
    fn every_selector_and_value_variant_round_trips() {
        let selectors = [
            AnnotationSelector::Frame { frame_index: 42 },
            AnnotationSelector::TextSegment {
                stream_id: "captions".to_string(),
                segment_index: 3,
            },
            AnnotationSelector::TextSpan {
                byte_start: 4,
                byte_end: 9,
                char_start: Some(4),
                char_end: Some(8),
            },
            region(),
            AnnotationSelector::Track {
                track_id: "track-1".to_string(),
            },
            AnnotationSelector::Custom {
                selector_kind: "point3d".to_string(),
                fields: BTreeMap::from([("x".to_string(), serde_json::json!(1.5))]),
            },
        ];
        for selector in selectors {
            selector.validate().unwrap();
            let encoded = serde_json::to_string(&selector).unwrap();
            assert_eq!(
                serde_json::from_str::<AnnotationSelector>(&encoded).unwrap(),
                selector
            );
        }

        let values = [
            AnnotationValue::Text("note".to_string()),
            AnnotationValue::Number(1.25),
            AnnotationValue::Integer(-3),
            AnnotationValue::Boolean(true),
            AnnotationValue::Labels(vec!["a".to_string(), "b".to_string()]),
            AnnotationValue::Json(serde_json::json!({ "nested": [1, 2] })),
        ];
        for value in values {
            value.validate().unwrap();
            let encoded = serde_json::to_string(&value).unwrap();
            assert_eq!(
                serde_json::from_str::<AnnotationValue>(&encoded).unwrap(),
                value
            );
        }
    }

    #[test]
    fn timing_queries_compare_different_timebases_exactly() {
        let instant = AnnotationTiming::Instant(timestamp(1, 1, 1));
        let range = AnnotationTiming::Range(
            MediaRange::new(timestamp(1_500, 1, 1_000), timestamp(2_500, 1, 1_000)).unwrap(),
        );
        let query = MediaRange::new(timestamp(900, 1, 1_000), timestamp(2, 1, 1)).unwrap();

        assert!(instant.overlaps(query).unwrap());
        assert!(range.overlaps(query).unwrap());
        assert!(instant.contains(timestamp(1_000, 1, 1_000)).unwrap());
        assert!(range.contains(timestamp(2, 1, 1)).unwrap());
        assert!(!range.contains(timestamp(5, 1, 2)).unwrap());
        assert!(range
            .start_timestamp()
            .same_instant(timestamp(3, 1, 2))
            .unwrap());
    }

    #[test]
    fn timing_deserialization_rejects_reversed_ranges_and_invalid_timebases() {
        let reversed = serde_json::json!({
            "kind": "range",
            "start": { "pts": "2", "timebase": { "num": 1, "den": 1 } },
            "end": { "pts": "1", "timebase": { "num": 1, "den": 1 } }
        });
        assert!(serde_json::from_value::<AnnotationTiming>(reversed).is_err());

        let zero_timebase = serde_json::json!({
            "kind": "instant",
            "timestamp": { "pts": "1", "timebase": { "num": 1, "den": 0 } }
        });
        assert!(serde_json::from_value::<AnnotationTiming>(zero_timebase).is_err());
    }

    #[test]
    fn analysis_events_adapt_without_losing_timing_or_score() {
        let event = AnalysisEvent::new("rhythm", "beat")
            .at_timestamp(timestamp(24, 1, 24))
            .score(0.8);
        let annotation = MediaAnnotation::from_analysis_event("beat-24", event).unwrap();

        assert_eq!(annotation.kind, "analysis_event");
        assert_eq!(annotation.label.as_deref(), Some("beat"));
        assert_eq!(annotation.score, Some(0.8));
        assert!(annotation
            .start_timestamp()
            .unwrap()
            .same_instant(timestamp(1, 1, 1))
            .unwrap());
        assert_eq!(annotation.provenance[0].analyzer.as_deref(), Some("rhythm"));
    }

    #[test]
    fn validation_rejects_invalid_envelopes() {
        let invalid_annotations = [
            MediaAnnotation::new(" ", "kind"),
            MediaAnnotation::new("id", ""),
            MediaAnnotation::new("id", "kind").score(f32::NAN),
            MediaAnnotation::new("id", "kind").selector(AnnotationSelector::TextSpan {
                byte_start: 5,
                byte_end: 4,
                char_start: None,
                char_end: None,
            }),
            MediaAnnotation::new("id", "kind").selector(AnnotationSelector::TextSpan {
                byte_start: 0,
                byte_end: 4,
                char_start: Some(0),
                char_end: None,
            }),
            MediaAnnotation::new("id", "kind").selector(AnnotationSelector::Region2d {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 1.0,
                coordinate_space: None,
            }),
            MediaAnnotation::new("id", "kind").value(AnnotationValue::Number(f64::INFINITY)),
            MediaAnnotation::new("id", "kind").value(AnnotationValue::Labels(vec![String::new()])),
            MediaAnnotation::new("id", "kind").source(MediaSourceRef::source("")),
            MediaAnnotation::new("id", "kind").provenance(AnnotationProvenance::analyzer(" ")),
            MediaAnnotation::new("id", "kind").attribute("", "value"),
            MediaAnnotation::new("id", "kind").at(Timestamp::new(1, Timebase::new(1, 0))),
        ];
        for annotation in invalid_annotations {
            assert!(annotation.validate().is_err(), "{annotation:?}");
        }
    }
}
