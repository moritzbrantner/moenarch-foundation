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
    start_seconds: Option<f64>,
    #[serde(default, rename = "end", alias = "end_seconds", alias = "endSeconds")]
    end_seconds: Option<f64>,
    #[serde(default, rename = "score", alias = "confidence")]
    confidence: Option<f32>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

/// Optional word-level timing carried by a timed-text producer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimedTextWordContract {
    pub text: String,
    #[serde(default)]
    start_seconds: Option<f64>,
    #[serde(default)]
    end_seconds: Option<f64>,
    #[serde(default)]
    confidence: Option<f32>,
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
    start_seconds: Option<f64>,
    #[serde(default)]
    end_seconds: Option<f64>,
    pub text: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub speaker: Option<String>,
    #[serde(default)]
    confidence: Option<f32>,
    pub is_final: bool,
    #[serde(default)]
    words: Vec<TimedTextWordContract>,
    #[serde(default)]
    chars: Vec<TimedTextCharContract>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

impl TimedTextCharContract {
    pub fn new(character: impl Into<String>) -> Self {
        Self {
            character: character.into(),
            start_seconds: None,
            end_seconds: None,
            confidence: None,
            attributes: BTreeMap::new(),
        }
    }

    pub fn with_time_range(
        mut self,
        start_seconds: Option<f64>,
        end_seconds: Option<f64>,
    ) -> Result<Self> {
        validate_seconds_range(start_seconds, end_seconds)?;
        self.start_seconds = start_seconds;
        self.end_seconds = end_seconds;
        Ok(self)
    }

    pub fn with_confidence(mut self, confidence: Option<f32>) -> Result<Self> {
        validate_confidence(confidence, "timed-text character")?;
        self.confidence = confidence;
        Ok(self)
    }

    pub fn start_seconds(&self) -> Option<f64> {
        self.start_seconds
    }

    pub fn end_seconds(&self) -> Option<f64> {
        self.end_seconds
    }

    pub fn confidence(&self) -> Option<f32> {
        self.confidence
    }
}

impl TimedTextWordContract {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            start_seconds: None,
            end_seconds: None,
            confidence: None,
            speaker: None,
            attributes: BTreeMap::new(),
        }
    }

    pub fn with_time_range(
        mut self,
        start_seconds: Option<f64>,
        end_seconds: Option<f64>,
    ) -> Result<Self> {
        validate_seconds_range(start_seconds, end_seconds)?;
        self.start_seconds = start_seconds;
        self.end_seconds = end_seconds;
        Ok(self)
    }

    pub fn with_confidence(mut self, confidence: Option<f32>) -> Result<Self> {
        validate_confidence(confidence, "timed-text word")?;
        self.confidence = confidence;
        Ok(self)
    }

    pub fn start_seconds(&self) -> Option<f64> {
        self.start_seconds
    }

    pub fn end_seconds(&self) -> Option<f64> {
        self.end_seconds
    }

    pub fn confidence(&self) -> Option<f32> {
        self.confidence
    }
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

    pub fn with_time_range(
        mut self,
        start_seconds: Option<f64>,
        end_seconds: Option<f64>,
    ) -> Result<Self> {
        validate_seconds_range(start_seconds, end_seconds)?;
        for word in &self.words {
            validate_nested_range(
                start_seconds,
                end_seconds,
                word.start_seconds,
                word.end_seconds,
                "word",
            )?;
        }
        for character in &self.chars {
            validate_nested_range(
                start_seconds,
                end_seconds,
                character.start_seconds,
                character.end_seconds,
                "character",
            )?;
        }
        self.start_seconds = start_seconds;
        self.end_seconds = end_seconds;
        Ok(self)
    }

    pub fn with_confidence(mut self, confidence: Option<f32>) -> Result<Self> {
        validate_confidence(confidence, "timed-text segment")?;
        self.confidence = confidence;
        Ok(self)
    }

    pub fn push_word(&mut self, word: TimedTextWordContract) -> Result<()> {
        validate_nested_range(
            self.start_seconds,
            self.end_seconds,
            word.start_seconds,
            word.end_seconds,
            "word",
        )?;
        self.words.push(word);
        Ok(())
    }

    pub fn push_char(&mut self, character: TimedTextCharContract) -> Result<()> {
        validate_nested_range(
            self.start_seconds,
            self.end_seconds,
            character.start_seconds,
            character.end_seconds,
            "character",
        )?;
        self.chars.push(character);
        Ok(())
    }

    pub fn start_seconds(&self) -> Option<f64> {
        self.start_seconds
    }

    pub fn end_seconds(&self) -> Option<f64> {
        self.end_seconds
    }

    pub fn confidence(&self) -> Option<f32> {
        self.confidence
    }

    pub fn words(&self) -> &[TimedTextWordContract] {
        &self.words
    }

    pub fn chars(&self) -> &[TimedTextCharContract] {
        &self.chars
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
    for child_value in [child_start, child_end].into_iter().flatten() {
        if parent_start.is_some_and(|start| child_value < start)
            || parent_end.is_some_and(|end| child_value > end)
        {
            return invalid(format!(
                "timed-text {child_name} timing must stay inside its segment"
            ));
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
        let mut word = TimedTextWordContract::new("hello")
            .with_time_range(Some(0.0), Some(1.0))
            .unwrap()
            .with_confidence(Some(0.8))
            .unwrap();
        word.speaker = Some("speaker-a".to_string());
        let character = TimedTextCharContract::new("h")
            .with_time_range(Some(0.0), Some(0.2))
            .unwrap()
            .with_confidence(Some(0.7))
            .unwrap();
        let mut segment = TimedTextSegmentContract::new(0, "hello")
            .with_time_range(Some(0.0), Some(1.0))
            .unwrap()
            .with_confidence(Some(0.9))
            .unwrap();
        segment.language = Some("en".to_string());
        segment.speaker = Some("speaker-a".to_string());
        segment.push_word(word).unwrap();
        segment.push_char(character).unwrap();
        let contract = TimedTextContract {
            text: Some("hello".to_string()),
            language: Some("en".to_string()),
            source: Some(" clip.wav ".to_string()),
            segments: vec![segment],
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
        assert!(serde_json::from_str::<TimedTextSegmentContract>(
            r#"{"index":0,"startSeconds":1.0,"endSeconds":2.0,"text":"hello","isFinal":true,"words":[{"text":"hello","startSeconds":3.0}]}"#
        )
        .is_err());
        assert!(serde_json::from_str::<TimedTextSegmentContract>(
            r#"{"index":0,"startSeconds":1.0,"endSeconds":2.0,"text":"hello","isFinal":true,"words":[{"text":"hello","endSeconds":0.0}]}"#
        )
        .is_err());
        assert!(serde_json::from_str::<TimedTextSegmentContract>(
            r#"{"index":0,"startSeconds":1.0,"endSeconds":2.0,"text":"hello","isFinal":true,"chars":[{"char":"h","start":3.0}]}"#
        )
        .is_err());
        assert!(serde_json::from_str::<TimedTextSegmentContract>(
            r#"{"index":0,"startSeconds":1.0,"endSeconds":2.0,"text":"hello","isFinal":true,"chars":[{"char":"h","end":0.0}]}"#
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
        assert!(TimedTextSegmentContract::new(0, "hello")
            .with_time_range(Some(f64::NAN), Some(1.0))
            .is_err());
        assert!(TimedTextWordContract::new("hello")
            .with_confidence(Some(f32::INFINITY))
            .is_err());
    }

    #[test]
    fn native_builders_preserve_nested_timing_invariants() {
        let mut bounded = TimedTextSegmentContract::new(0, "hello")
            .with_time_range(Some(1.0), Some(2.0))
            .unwrap();
        let early_word = TimedTextWordContract::new("hello")
            .with_time_range(Some(0.5), Some(1.5))
            .unwrap();
        let late_character = TimedTextCharContract::new("h")
            .with_time_range(Some(1.5), Some(2.5))
            .unwrap();

        assert!(bounded.push_word(early_word).is_err());
        assert!(bounded.push_char(late_character).is_err());
        assert!(bounded.words().is_empty());
        assert!(bounded.chars().is_empty());

        let mut populated = TimedTextSegmentContract::new(1, "hello");
        populated
            .push_word(
                TimedTextWordContract::new("hello")
                    .with_time_range(Some(1.0), Some(2.0))
                    .unwrap(),
            )
            .unwrap();
        populated
            .push_char(
                TimedTextCharContract::new("h")
                    .with_time_range(Some(1.25), Some(1.5))
                    .unwrap(),
            )
            .unwrap();

        assert!(populated
            .clone()
            .with_time_range(Some(1.1), Some(2.0))
            .is_err());
        assert!(populated.with_time_range(Some(1.0), Some(1.9)).is_err());
    }

    #[test]
    fn strict_validation_keeps_nested_timing_inside_segment() {
        let later = TimedTextSegmentContract::new(0, "later")
            .with_time_range(Some(2.0), Some(3.0))
            .unwrap();
        let earlier = TimedTextSegmentContract::new(1, "earlier")
            .with_time_range(Some(1.0), Some(2.0))
            .unwrap();
        let contract = TimedTextContract::new(vec![later, earlier]);

        assert!(contract.validate_strict().is_err());
    }
}
