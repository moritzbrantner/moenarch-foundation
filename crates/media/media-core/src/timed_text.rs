use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTimeRange {
    pub start_seconds: f64,
    pub end_seconds: f64,
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

    pub fn midpoint_seconds(self) -> f64 {
        (self.start_seconds + self.end_seconds) * 0.5
    }
}

/// Optional character-level timing carried by a timed-text producer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    pub fn duration_seconds(&self) -> Option<f64> {
        Some((self.end_seconds? - self.start_seconds?).max(0.0))
    }

    pub fn midpoint_seconds(&self) -> Option<f64> {
        Some((self.start_seconds? + self.end_seconds?) * 0.5)
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

    pub fn normalized(mut self) -> Self {
        self.text = self.text.trim().to_string();
        self.language = normalize_optional_string(self.language);
        self.speaker = normalize_optional_string(self.speaker);
        self.confidence = sanitize_confidence(self.confidence);
        self.words = self
            .words
            .into_iter()
            .filter_map(|mut word| {
                word.text = word.text.trim().to_string();
                word.speaker = normalize_optional_string(word.speaker);
                word.confidence = sanitize_confidence(word.confidence);
                (!word.text.is_empty()).then_some(word)
            })
            .collect();
        self.chars = self
            .chars
            .into_iter()
            .filter_map(|mut character| {
                character.confidence = sanitize_confidence(character.confidence);
                (!character.character.is_empty()).then_some(character)
            })
            .collect();
        self
    }
}

/// Domain-neutral timed text exchanged between media producers and consumers.
///
/// This contract deliberately does not own transcript parsing, subtitle formats,
/// NLP annotations, model execution, or provider-specific behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TimedTextContract {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub segments: Vec<TimedTextSegmentContract>,
    /// Backward-compatible simple source identifier or URI.
    #[serde(default)]
    pub source: Option<String>,
    /// Optional structured source metadata for consumers that need more than a URI.
    #[serde(default)]
    pub source_ref: Option<MediaSourceRef>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
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

    pub fn joined_text(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn text_or_joined(&self) -> String {
        self.text
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| self.joined_text())
    }

    pub fn normalized(mut self) -> Result<Self> {
        self.text = normalize_optional_string(self.text);
        self.language = normalize_optional_string(self.language);
        self.source = normalize_optional_string(self.source);
        self.segments = self
            .segments
            .into_iter()
            .map(TimedTextSegmentContract::normalized)
            .collect();
        if self.text.is_none() {
            let joined = self.joined_text();
            if !joined.is_empty() {
                self.text = Some(joined);
            }
        }
        self.validate()?;
        Ok(self)
    }
}

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
    if value.is_some_and(|confidence| !confidence.is_finite()) {
        return invalid(format!("{subject} confidence must be finite"));
    }
    Ok(())
}

fn sanitize_confidence(value: Option<f32>) -> Option<f32> {
    value.and_then(|confidence| confidence.is_finite().then(|| confidence.clamp(0.0, 1.0)))
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
        assert_eq!(MediaTimeRange::new(1.0, 2.5).unwrap().duration_seconds(), 1.5);
    }

    #[test]
    fn timed_text_normalizes_without_adding_domain_semantics() {
        let contract = TimedTextContract {
            source: Some(" clip.wav ".to_string()),
            segments: vec![TimedTextSegmentContract {
                index: 0,
                start_seconds: Some(0.0),
                end_seconds: Some(1.0),
                text: " hello ".to_string(),
                language: Some(" en ".to_string()),
                speaker: Some(" speaker-a ".to_string()),
                confidence: Some(1.2),
                is_final: true,
                words: Vec::new(),
                chars: Vec::new(),
                attributes: BTreeMap::new(),
            }],
            ..TimedTextContract::default()
        }
        .normalized()
        .unwrap();

        assert_eq!(contract.text.as_deref(), Some("hello"));
        assert_eq!(contract.source.as_deref(), Some("clip.wav"));
        assert_eq!(contract.segments[0].language.as_deref(), Some("en"));
        assert_eq!(contract.segments[0].speaker.as_deref(), Some("speaker-a"));
        assert_eq!(contract.segments[0].confidence, Some(1.0));
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
