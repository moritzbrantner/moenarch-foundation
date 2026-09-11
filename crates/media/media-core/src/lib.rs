#![doc = include_str!("../README.md")]

mod time;
pub mod timed_text;
pub mod timed_text_format;
pub use time::MediaRange;
pub use timed_text::{
    MediaSourceRef, MediaTimeRange, TimedTextCharContract, TimedTextContract,
    TimedTextSegmentContract, TimedTextWordContract, TranscriptCharContract,
    TranscriptSegmentContract, TranscriptWordContract, TranscriptionContract,
};
pub use timed_text_format::{
    format_audacity_labels, format_plain_text, format_srt, format_tsv, format_webvtt,
    parse_plain_text, parse_srt, parse_webvtt,
};

const JAVASCRIPT_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// A compact identifier for the pixel layout of a video frame.
///
/// This is neutral stream-format metadata. Pixel buffers and video frames
/// remain owned by the visual domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// Packed RGB with eight bits per channel.
    Rgb24,
    /// Packed BGR with eight bits per channel.
    Bgr24,
}

/// A compact identifier for the scalar representation of audio samples.
///
/// This is neutral stream-format metadata. Audio buffers and frames remain
/// owned by the audio domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSampleFormat {
    /// Unsigned eight-bit samples.
    U8,
    /// Signed 16-bit samples.
    I16,
    /// Signed 32-bit samples.
    I32,
    /// 32-bit floating-point samples.
    F32,
}

/// Errors shared by media contract consumers.
#[derive(Debug)]
pub enum DetectError {
    /// The pixel format is unsupported by the requested operation.
    UnsupportedPixelFormat(PixelFormat),
    /// The audio sample format is unsupported by the requested operation.
    UnsupportedAudioSampleFormat(AudioSampleFormat),
    /// A frame buffer is shorter than its declared dimensions require.
    InvalidFrameBuffer {
        /// Minimum required byte length.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// Media dimensions are invalid.
    InvalidDimensions {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// Audio format metadata is invalid.
    InvalidAudioFormat {
        /// Sample rate in hertz.
        sample_rate: u32,
        /// Number of audio channels.
        channels: u16,
    },
    /// A media source failed.
    Source(String),
    /// An argument failed validation.
    InvalidArgument(String),
    /// An I/O operation failed.
    Io(std::io::Error),
}

impl std::fmt::Display for DetectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPixelFormat(format) => {
                write!(f, "unsupported pixel format: {format:?}")
            }
            Self::UnsupportedAudioSampleFormat(format) => {
                write!(f, "unsupported audio sample format: {format:?}")
            }
            Self::InvalidFrameBuffer { expected, actual } => {
                write!(
                    f,
                    "invalid frame buffer: expected at least {expected} bytes, got {actual}"
                )
            }
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid dimensions: {width}x{height}")
            }
            Self::InvalidAudioFormat {
                sample_rate,
                channels,
            } => {
                write!(
                    f,
                    "invalid audio format: sample_rate={sample_rate}, channels={channels}"
                )
            }
            Self::Source(message) => write!(f, "video source error: {message}"),
            Self::InvalidArgument(message) => write!(f, "invalid argument: {message}"),
            Self::Io(error) => write!(f, "I/O error: {error}"),
        }
    }
}

impl std::error::Error for DetectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for DetectError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Result type shared by media contract consumers.
pub type Result<T> = std::result::Result<T, DetectError>;

/// A rational number of seconds represented by one timestamp tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct Timebase {
    /// The numerator of seconds per tick.
    pub num: i32,
    /// The denominator of seconds per tick.
    pub den: i32,
}

#[derive(serde::Deserialize)]
struct TimebaseWire {
    num: i32,
    den: i32,
}

impl<'de> serde::Deserialize<'de> for Timebase {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = <TimebaseWire as serde::Deserialize>::deserialize(deserializer)?;
        Self::try_new(wire.num, wire.den).map_err(serde::de::Error::custom)
    }
}

impl Timebase {
    /// Creates a timebase from a seconds-per-tick numerator and denominator.
    ///
    /// This compatibility constructor does not validate its arguments. New
    /// boundary code should prefer [`Self::try_new`].
    pub const fn new(num: i32, den: i32) -> Self {
        Self { num, den }
    }

    /// Returns seconds per tick.
    pub fn seconds_per_tick(self) -> f64 {
        self.num as f64 / self.den as f64
    }
}

/// A presentation timestamp paired with its timebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    /// Presentation timestamp ticks.
    pub pts: i64,
    /// The timebase that gives each tick meaning.
    pub timebase: Timebase,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TimestampBinaryWire {
    pts: i64,
    timebase: Timebase,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum TimestampPtsWire {
    Decimal(String),
    SafeInteger(i64),
}

impl TimestampPtsWire {
    fn into_i64<E>(self) -> std::result::Result<i64, E>
    where
        E: serde::de::Error,
    {
        match self {
            Self::Decimal(value) => value.parse::<i64>().map_err(E::custom),
            Self::SafeInteger(value)
                if (-JAVASCRIPT_MAX_SAFE_INTEGER..=JAVASCRIPT_MAX_SAFE_INTEGER).contains(&value) =>
            {
                Ok(value)
            }
            Self::SafeInteger(_) => Err(E::custom(
                "numeric timestamp pts exceeds the JavaScript safe-integer range; use a decimal string",
            )),
        }
    }
}

#[derive(serde::Deserialize)]
struct TimestampWire {
    pts: TimestampPtsWire,
    timebase: Timebase,
}

impl serde::Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if !serializer.is_human_readable() {
            return serde::Serialize::serialize(
                &TimestampBinaryWire {
                    pts: self.pts,
                    timebase: self.timebase,
                },
                serializer,
            );
        }

        use serde::ser::SerializeStruct;

        let mut wire = serializer.serialize_struct("Timestamp", 2)?;
        wire.serialize_field("pts", &self.pts.to_string())?;
        wire.serialize_field("timebase", &self.timebase)?;
        wire.end()
    }
}

impl<'de> serde::Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            let wire = <TimestampBinaryWire as serde::Deserialize>::deserialize(deserializer)?;
            return Self::try_new(wire.pts, wire.timebase).map_err(serde::de::Error::custom);
        }

        let wire = <TimestampWire as serde::Deserialize>::deserialize(deserializer)?;
        let pts = wire.pts.into_i64::<D::Error>()?;
        Self::try_new(pts, wire.timebase).map_err(serde::de::Error::custom)
    }
}

impl Timestamp {
    /// Creates a timestamp.
    ///
    /// This compatibility constructor does not validate its timebase. New
    /// boundary code should prefer [`Self::try_new`].
    pub const fn new(pts: i64, timebase: Timebase) -> Self {
        Self { pts, timebase }
    }

    /// Returns the timestamp in seconds.
    pub fn seconds(self) -> f64 {
        self.pts as f64 * self.timebase.seconds_per_tick()
    }
}

/// A domain-neutral analysis result located optionally in media time.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisEvent {
    /// Timestamp associated with this event.
    pub timestamp: Option<Timestamp>,
    /// The analyzer that produced the event.
    pub analyzer: String,
    /// The event label.
    pub label: String,
    /// Optional confidence or relevance score.
    pub score: Option<f32>,
}

impl AnalysisEvent {
    /// Creates an event without a timestamp or score.
    pub fn new(analyzer: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            timestamp: None,
            analyzer: analyzer.into(),
            label: label.into(),
            score: None,
        }
    }

    /// Adds a timestamp.
    pub fn at_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Adds a score.
    pub fn score(mut self, score: f32) -> Self {
        self.score = Some(score);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{AnalysisEvent, AudioSampleFormat, DetectError, PixelFormat, Timebase, Timestamp};

    #[test]
    fn timestamp_uses_its_rational_timebase() {
        let timestamp = Timestamp::new(125, Timebase::new(1, 1_000));
        assert_eq!(timestamp.seconds(), 0.125);
    }

    #[test]
    fn timestamp_wire_round_trip_is_lossless() {
        let timestamp = Timestamp::try_new(125, Timebase::try_new(1, 1_000).unwrap()).unwrap();
        let encoded = serde_json::to_value(timestamp).unwrap();

        assert_eq!(
            encoded,
            serde_json::json!({
                "pts": "125",
                "timebase": { "num": 1, "den": 1_000 }
            })
        );
        assert_eq!(
            serde_json::from_value::<Timestamp>(encoded).unwrap(),
            timestamp
        );
    }

    #[test]
    fn timestamp_wire_round_trips_full_i64_range() {
        let timebase = Timebase::try_new(1, 1_000).unwrap();
        for pts in [i64::MIN, i64::MAX] {
            let timestamp = Timestamp::try_new(pts, timebase).unwrap();
            let encoded = serde_json::to_value(timestamp).unwrap();

            assert_eq!(encoded["pts"], pts.to_string());
            assert_eq!(
                serde_json::from_value::<Timestamp>(encoded).unwrap(),
                timestamp
            );
        }
    }

    #[test]
    fn timestamp_wire_accepts_only_safe_legacy_numeric_pts() {
        let safe = serde_json::json!({
            "pts": 9_007_199_254_740_991_i64,
            "timebase": { "num": 1, "den": 1_000 }
        });
        assert_eq!(
            serde_json::from_value::<Timestamp>(safe).unwrap().pts,
            9_007_199_254_740_991
        );

        let unsafe_numeric = serde_json::json!({
            "pts": 9_007_199_254_740_992_i64,
            "timebase": { "num": 1, "den": 1_000 }
        });
        assert!(serde_json::from_value::<Timestamp>(unsafe_numeric).is_err());

        let unsafe_string = serde_json::json!({
            "pts": "9007199254740992",
            "timebase": { "num": 1, "den": 1_000 }
        });
        assert_eq!(
            serde_json::from_value::<Timestamp>(unsafe_string)
                .unwrap()
                .pts,
            9_007_199_254_740_992
        );
    }

    #[test]
    fn timestamp_binary_wire_preserves_legacy_i64_shape() {
        #[derive(serde::Serialize)]
        struct LegacyTimestamp {
            pts: i64,
            timebase: Timebase,
        }

        let timestamp = Timestamp::try_new(i64::MIN, Timebase::try_new(1, 1_000).unwrap()).unwrap();
        let encoded = bincode::serialize(&timestamp).unwrap();
        let legacy = bincode::serialize(&LegacyTimestamp {
            pts: timestamp.pts,
            timebase: timestamp.timebase,
        })
        .unwrap();

        assert_eq!(encoded, legacy);
        assert_eq!(
            bincode::deserialize::<Timestamp>(&encoded).unwrap(),
            timestamp
        );
    }

    #[test]
    fn timestamp_wire_rejects_invalid_timebases() {
        let invalid = serde_json::json!({
            "pts": "125",
            "timebase": { "num": 1, "den": 0 }
        });

        assert!(serde_json::from_value::<Timestamp>(invalid).is_err());
        assert!(serde_json::from_value::<Timebase>(serde_json::json!({
            "num": -1,
            "den": -1_000
        }))
        .is_err());
    }

    #[test]
    fn analysis_event_builders_preserve_neutral_contract_data() {
        let timestamp = Timestamp::new(12, Timebase::new(1, 24));
        let event = AnalysisEvent::new("fixture", "cut")
            .at_timestamp(timestamp)
            .score(0.75);

        assert_eq!(event.timestamp, Some(timestamp));
        assert_eq!(event.analyzer, "fixture");
        assert_eq!(event.label, "cut");
        assert_eq!(event.score, Some(0.75));
    }

    #[test]
    fn shared_error_display_preserves_compatibility_messages() {
        assert_eq!(
            DetectError::UnsupportedPixelFormat(PixelFormat::Bgr24).to_string(),
            "unsupported pixel format: Bgr24"
        );
        assert_eq!(
            DetectError::UnsupportedAudioSampleFormat(AudioSampleFormat::I16).to_string(),
            "unsupported audio sample format: I16"
        );
        assert_eq!(
            DetectError::InvalidFrameBuffer {
                expected: 24,
                actual: 12,
            }
            .to_string(),
            "invalid frame buffer: expected at least 24 bytes, got 12"
        );
        assert_eq!(
            DetectError::InvalidDimensions {
                width: 0,
                height: 12,
            }
            .to_string(),
            "invalid dimensions: 0x12"
        );
        assert_eq!(
            DetectError::InvalidAudioFormat {
                sample_rate: 0,
                channels: 2,
            }
            .to_string(),
            "invalid audio format: sample_rate=0, channels=2"
        );
        assert_eq!(
            DetectError::Source("unavailable".into()).to_string(),
            "video source error: unavailable"
        );
        assert_eq!(
            DetectError::InvalidArgument("bad value".into()).to_string(),
            "invalid argument: bad value"
        );
        assert_eq!(
            DetectError::from(std::io::Error::other("disk")).to_string(),
            "I/O error: disk"
        );
    }
}
