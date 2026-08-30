use std::collections::BTreeMap;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use crate::{DetectError, Result};

/// A domain-neutral reference to the source that produced or contains media data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaSourceRef {
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

/// A finite interval on a media timeline, expressed in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTimeRange {
    start_seconds: f64,
    end_seconds: f64,
}

impl MediaTimeRange {
    pub fn new(start_seconds: f64, end_seconds: f64) -> Result<Self> {
        validate_seconds_range(Some(start_seconds), Some(end_seconds))?;
        Ok(Self {
            start_seconds,
            end_seconds,
        })
    }

    pub fn duration_seconds(self) -> f64 {
        self.end_seconds - self.start_seconds
    }

    pub fn start_seconds(self) -> f64 {
        self.start_seconds
    }

    pub fn end_seconds(self) -> f64 {
        self.end_seconds
    }

    pub fn midpoint_seconds(self) -> f64 {
        (self.start_seconds + self.end_seconds) * 0.5
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UncheckedMediaTimeRange {
    start_seconds: f64,
    end_seconds: f64,
}

impl<'de> Deserialize<'de> for MediaTimeRange {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unchecked = UncheckedMediaTimeRange::deserialize(deserializer)?;
        Self::new(unchecked.start_seconds, unchecked.end_seconds).map_err(D::Error::custom)
    }
}

/// Optional character-level timing carried by a timed-text producer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimedTextCharContract {
    #[serde(rename = "char")]
    pub character: String,
    #[serde(
        default,
        rename = "start",
        alias = "start_seconds",
        alias = "startSeconds"
    )]
    pub start_seconds: Option<f64>,
    #[serde(default, rename = "end", alias = "end_seconds", alias = "endSeconds")]
    pub end_seconds: Option<f64>,
    #[serde(default, rename = "score", alias = "confidence")]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

/// Optional word-level timing carried by a timed-text producer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimedTextWordContract {
    pub text: String,
    #[serde(default)]
    pub start_seconds: Option<f64>,
    #[serde(default)]
    pub end_seconds: Option<f64>,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub speaker: Option<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

/// One ordered unit of timed text.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimedTextSegmentContract {
    pub index: u64,
    #[serde(default)]
    pub start_seconds: Option<f64>,
    #[serde(default)]
    pub end_seconds: Option<f64>,
    pub text: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub speaker: Option<String>,
    #[serde(default)]
    pub confidence: Option<f32>,
    pub is_final: bool,
    #[serde(default)]
    pub words: Vec<TimedTextWordContract>,
    #[serde(default)]
    pub chars: Vec<TimedTextCharContract>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

impl TimedTextSegmentContract {
    pub fn new(index: u64, text: impl Into<String>) -> Self {
        Self {
            index,
            start_seconds: None,
            end_seconds: None,
            text: text.into(),
            language: None,
            speaker: None,
            confidence: None,
            is_final: true,
            words: Vec::new(),
            chars: Vec::new(),
            attributes: BTreeMap::new(),
        }
    }

    pub fn time_range(&self) -> Result<Option<MediaTimeRange>> {
        match (self.start_seconds, self.end_seconds) {
            (Some(start), Some(end)) => MediaTimeRange::new(start, end).map(Some),
            _ => Ok(None),
        }
    }

    pub fn validate(&self) -> Result<()> {
        validate_seconds_range(self.start_seconds, self.end_seconds)?;
        validate_confidence(self.confidence, "timed-text segment")?;
        for word in &self.words {
            validate_seconds_range(word.start_seconds, word.end_seconds)?;
            validate_confidence(word.confidence, "timed-text word")?;
            validate_nested_range(
                self.start_seconds,
                self.end_seconds,
                word.start_seconds,
                word.end_seconds,
                "word",
            )?;
        }
        for character in &self.chars {
            validate_seconds_range(character.start_seconds, character.end_seconds)?;
            validate_confidence(character.confidence, "timed-text character")?;
            validate_nested_range(
                self.start_seconds,
                self.end_seconds,
                character.start_seconds,
                character.end_seconds,
                "character",
            )?;
        }
        Ok(())
    }
}

/// Domain-neutral timed text exchanged between media producers and consumers.
///
/// This contract deliberately does not own transcript parsing, subtitle formats,
/// NLP annotations, model execution, or provider-specific behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TimedTextContract {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub segments: Vec<TimedTextSegmentContract>,
    /// Simple source identifier or URI. Rich source metadata is represented by
    /// `MediaSourceRef` separately so this DTO stays compatible with existing
    /// transcript producers.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UncheckedTimedTextCharContract {
    #[serde(rename = "char")]
    character: String,
    #[serde(
        default,
        rename = "start",
        alias = "start_seconds",
        alias = "startSeconds"
    )]
    start_seconds: Option<f64>,
    #[serde(default, rename = "end", alias = "end_seconds", alias = "endSeconds")]
    end_seconds: Option<f64>,
    #[serde(default, rename = "score", alias = "confidence")]
    confidence: Option<f32>,
    #[serde(default)]
    attributes: BTreeMap<String, String>,
}

impl<'de> Deserialize<'de> for TimedTextCharContract {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unchecked = UncheckedTimedTextCharContract::deserialize(deserializer)?;
        validate_seconds_range(unchecked.start_seconds, unchecked.end_seconds)
            .map_err(D::Error::custom)?;
        validate_confidence(unchecked.confidence, "timed-text character")
            .map_err(D::Error::custom)?;
        Ok(Self {
            character: unchecked.character,
            start_seconds: unchecked.start_seconds,
            end_seconds: unchecked.end_seconds,
            confidence: unchecked.confidence,
            attributes: unchecked.attributes,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UncheckedTimedTextWordContract {
    text: String,
    #[serde(default)]
    start_seconds: Option<f64>,
    #[serde(default)]
    end_seconds: Option<f64>,
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    speaker: Option<String>,
    #[serde(default)]
    attributes: BTreeMap<String, String>,
}

impl<'de> Deserialize<'de> for TimedTextWordContract {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unchecked = UncheckedTimedTextWordContract::deserialize(deserializer)?;
        validate_seconds_range(unchecked.start_seconds, unchecked.end_seconds)
            .map_err(D::Error::custom)?;
        validate_confidence(unchecked.confidence, "timed-text word").map_err(D::Error::custom)?;
        Ok(Self {
            text: unchecked.text,
            start_seconds: unchecked.start_seconds,
            end_seconds: unchecked.end_seconds,
            confidence: unchecked.confidence,
            speaker: unchecked.speaker,
            attributes: unchecked.attributes,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UncheckedTimedTextSegmentContract {
    index: u64,
    #[serde(default)]
    start_seconds: Option<f64>,
    #[serde(default)]
    end_seconds: Option<f64>,
    text: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    speaker: Option<String>,
    #[serde(default)]
    confidence: Option<f32>,
    is_final: bool,
    #[serde(default)]
    words: Vec<TimedTextWordContract>,
    #[serde(default)]
    chars: Vec<TimedTextCharContract>,
    #[serde(default)]
    attributes: BTreeMap<String, String>,
}

impl<'de> Deserialize<'de> for TimedTextSegmentContract {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unchecked = UncheckedTimedTextSegmentContract::deserialize(deserializer)?;
        let contract = Self {
            index: unchecked.index,
            start_seconds: unchecked.start_seconds,
            end_seconds: unchecked.end_seconds,
            text: unchecked.text,
            language: unchecked.language,
            speaker: unchecked.speaker,
            confidence: unchecked.confidence,
            is_final: unchecked.is_final,
            words: unchecked.words,
            chars: unchecked.chars,
            attributes: unchecked.attributes,
        };
        contract.validate().map_err(D::Error::custom)?;
        Ok(contract)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UncheckedTimedTextContract {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    segments: Vec<TimedTextSegmentContract>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    attributes: BTreeMap<String, String>,
}

impl<'de> Deserialize<'de> for TimedTextContract {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unchecked = UncheckedTimedTextContract::deserialize(deserializer)?;
        let contract = Self {
            text: unchecked.text,
            language: unchecked.language,
            segments: unchecked.segments,
            source: unchecked.source,
            attributes: unchecked.attributes,
        };
        contract.validate().map_err(D::Error::custom)?;
        Ok(contract)
    }
}

impl TimedTextContract {
    pub fn new(segments: Vec<TimedTextSegmentContract>) -> Self {
        Self {
            segments,
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<()> {
        for segment in &self.segments {
            segment.validate()?;
        }
        Ok(())
    }

    pub fn validate_strict(&self) -> Result<()> {
        self.validate()?;
        let mut last_start = None;
        for segment in &self.segments {
            if segment.text.trim().is_empty() {
                return invalid("timed-text segment text must not be empty");
            }
            if let (Some(previous), Some(current)) = (last_start, segment.start_seconds) {
                if current < previous {
                    return invalid("timed-text segment start_seconds must not move backward");
                }
            }
            if segment.start_seconds.is_some() {
                last_start = segment.start_seconds;
            }
        }
        Ok(())
    }
}

// Compatibility names let capability repositories cut the package dependency
// over to foundation without a simultaneous large source rename. These aliases
// do not add parsing, formatting, or NLP behavior to foundation.
pub type TranscriptCharContract = TimedTextCharContract;
pub type TranscriptWordContract = TimedTextWordContract;
pub type TranscriptSegmentContract = TimedTextSegmentContract;
pub type TranscriptionContract = TimedTextContract;

fn validate_seconds_range(start: Option<f64>, end: Option<f64>) -> Result<()> {
    if start.is_some_and(|value| !value.is_finite()) || end.is_some_and(|value| !value.is_finite())
    {
        return invalid("media time values must be finite");
    }
    if let (Some(start), Some(end)) = (start, end) {
        if end < start {
            return invalid("media time range end must be greater than or equal to start");
        }
    }
    Ok(())
}

fn validate_nested_range(
    parent_start: Option<f64>,
    parent_end: Option<f64>,
    child_start: Option<f64>,
    child_end: Option<f64>,
    child_name: &str,
) -> Result<()> {
    if let (Some(parent_start), Some(child_start)) = (parent_start, child_start) {
        if child_start < parent_start {
            return invalid(format!("timed-text {child_name} starts before its segment"));
        }
    }
    if let (Some(parent_end), Some(child_end)) = (parent_end, child_end) {
        if child_end > parent_end {
            return invalid(format!("timed-text {child_name} ends after its segment"));
        }
    }
    Ok(())
}

fn validate_confidence(value: Option<f32>, subject: &str) -> Result<()> {
    if value.is_some_and(|confidence| !confidence.is_finite() || !(0.0..=1.0).contains(&confidence))
    {
        return invalid(format!(
            "{subject} confidence must be finite and between zero and one"
        ));
    }
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(DetectError::InvalidArgument(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_time_range_rejects_invalid_ranges() {
        assert!(MediaTimeRange::new(2.0, 1.0).is_err());
        assert!(MediaTimeRange::new(f64::NAN, 1.0).is_err());
        assert_eq!(
            MediaTimeRange::new(1.0, 2.5).unwrap().duration_seconds(),
            1.5
        );
    }

    #[test]
    fn timed_text_round_trips_without_adding_domain_semantics() {
        let contract = TimedTextContract {
            text: Some("hello".to_string()),
            language: Some("en".to_string()),
            source: Some(" clip.wav ".to_string()),
            segments: vec![TimedTextSegmentContract {
                index: 0,
                start_seconds: Some(0.0),
                end_seconds: Some(1.0),
                text: "hello".to_string(),
                language: Some("en".to_string()),
                speaker: Some("speaker-a".to_string()),
                confidence: Some(0.9),
                is_final: true,
                words: vec![TimedTextWordContract {
                    text: "hello".to_string(),
                    start_seconds: Some(0.0),
                    end_seconds: Some(1.0),
                    confidence: Some(0.8),
                    speaker: Some("speaker-a".to_string()),
                    attributes: BTreeMap::new(),
                }],
                chars: vec![TimedTextCharContract {
                    character: "h".to_string(),
                    start_seconds: Some(0.0),
                    end_seconds: Some(0.2),
                    confidence: Some(0.7),
                    attributes: BTreeMap::new(),
                }],
                attributes: BTreeMap::new(),
            }],
            attributes: BTreeMap::new(),
        };
        contract.validate().unwrap();

        let encoded = serde_json::to_string(&contract).unwrap();
        let decoded: TimedTextContract = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded, contract);
    }

    #[test]
    fn compatibility_names_preserve_the_neutral_shape() {
        let segment: TranscriptSegmentContract = TimedTextSegmentContract::new(0, "hello");
        let contract: TranscriptionContract = TimedTextContract::new(vec![segment]);

        assert_eq!(contract.segments[0].text, "hello");
    }

    #[test]
    fn deserialization_rejects_invalid_ranges_at_every_level() {
        assert!(
            serde_json::from_str::<MediaTimeRange>(r#"{"startSeconds":2.0,"endSeconds":1.0}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<TimedTextWordContract>(
            r#"{"text":"hello","startSeconds":2.0,"endSeconds":1.0}"#
        )
        .is_err());
        assert!(serde_json::from_str::<TimedTextSegmentContract>(
            r#"{"index":0,"startSeconds":1.0,"endSeconds":2.0,"text":"hello","isFinal":true,"words":[{"text":"hello","startSeconds":0.5,"endSeconds":1.5}]}"#
        )
        .is_err());
        assert!(serde_json::from_str::<TimedTextContract>(
            r#"{"segments":[{"index":0,"startSeconds":2.0,"endSeconds":1.0,"text":"hello","isFinal":true}]}"#
        )
        .is_err());
        assert!(serde_json::from_str::<TimedTextSegmentContract>(
            r#"{"index":0,"text":"hello","confidence":1.1,"isFinal":true}"#
        )
        .is_err());
    }

    #[test]
    fn validation_rejects_non_finite_nested_values() {
        let contract = TimedTextContract::new(vec![TimedTextSegmentContract {
            index: 0,
            start_seconds: Some(f64::NAN),
            end_seconds: Some(1.0),
            text: "hello".to_string(),
            language: None,
            speaker: None,
            confidence: None,
            is_final: true,
            words: Vec::new(),
            chars: Vec::new(),
            attributes: BTreeMap::new(),
        }]);

        assert!(contract.validate().is_err());
    }

    #[test]
    fn strict_validation_keeps_nested_timing_inside_segment() {
        let contract = TimedTextContract::new(vec![TimedTextSegmentContract {
            index: 0,
            start_seconds: Some(1.0),
            end_seconds: Some(2.0),
            text: "hello".to_string(),
            language: None,
            speaker: None,
            confidence: None,
            is_final: true,
            words: vec![TimedTextWordContract {
                text: "hello".to_string(),
                start_seconds: Some(0.5),
                end_seconds: Some(1.5),
                confidence: None,
                speaker: None,
                attributes: BTreeMap::new(),
            }],
            chars: Vec::new(),
            attributes: BTreeMap::new(),
        }]);

        assert!(contract.validate_strict().is_err());
    }
}
