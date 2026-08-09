#![doc = include_str!("../README.md")]

pub use media_core::{AnalysisEvent, AudioSampleFormat, DetectError, Result, Timebase, Timestamp};

#[derive(Debug, Clone, PartialEq)]
/// Variants describing audio buffer.
pub enum AudioBuffer {
    /// The u8 variant.
    U8(Vec<u8>),
    /// The i16 variant.
    I16(Vec<i16>),
    /// The i32 variant.
    I32(Vec<i32>),
    /// The f32 variant.
    F32(Vec<f32>),
}

impl AudioBuffer {
    /// Returns len.
    pub fn len(&self) -> usize {
        match self {
            Self::U8(values) => values.len(),
            Self::I16(values) => values.len(),
            Self::I32(values) => values.len(),
            Self::F32(values) => values.len(),
        }
    }

    /// Returns whether is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns sample format.
    pub fn sample_format(&self) -> AudioSampleFormat {
        match self {
            Self::U8(_) => AudioSampleFormat::U8,
            Self::I16(_) => AudioSampleFormat::I16,
            Self::I32(_) => AudioSampleFormat::I32,
            Self::F32(_) => AudioSampleFormat::F32,
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// Data type for audio frame.
pub struct AudioFrame<'a> {
    /// Timestamp associated with this value.
    pub timestamp: Timestamp,
    /// Sample rate in hertz.
    pub sample_rate: u32,
    /// Number of audio channels.
    pub channels: u16,
    /// Underlying data buffer.
    pub data: &'a AudioBuffer,
}

impl<'a> AudioFrame<'a> {
    /// Creates a new value.
    pub fn new(
        timestamp: Timestamp,
        sample_rate: u32,
        channels: u16,
        data: &'a AudioBuffer,
    ) -> Result<Self> {
        if sample_rate == 0 || channels == 0 {
            return Err(DetectError::InvalidAudioFormat {
                sample_rate,
                channels,
            });
        }
        Ok(Self {
            timestamp,
            sample_rate,
            channels,
            data,
        })
    }

    /// Returns sample format.
    pub fn sample_format(&self) -> AudioSampleFormat {
        self.data.sample_format()
    }

    /// Returns sample count.
    pub fn sample_count(&self) -> usize {
        self.data.len()
    }

    /// Returns samples per channel.
    pub fn samples_per_channel(&self) -> usize {
        self.sample_count() / self.channels as usize
    }

    /// Returns duration seconds.
    pub fn duration_seconds(&self) -> f64 {
        self.samples_per_channel() as f64 / self.sample_rate as f64
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for owned audio frame.
pub struct OwnedAudioFrame {
    /// Timestamp associated with this value.
    pub timestamp: Timestamp,
    /// Sample rate in hertz.
    pub sample_rate: u32,
    /// Number of audio channels.
    pub channels: u16,
    /// Underlying data buffer.
    pub data: AudioBuffer,
}

impl OwnedAudioFrame {
    /// Creates a new value.
    pub fn new(
        timestamp: Timestamp,
        sample_rate: u32,
        channels: u16,
        data: AudioBuffer,
    ) -> Result<Self> {
        AudioFrame::new(timestamp, sample_rate, channels, &data)?;
        Ok(Self {
            timestamp,
            sample_rate,
            channels,
            data,
        })
    }

    /// Borrows this value as a frame.
    pub fn as_frame(&self) -> Result<AudioFrame<'_>> {
        AudioFrame::new(self.timestamp, self.sample_rate, self.channels, &self.data)
    }

    /// Returns sample format.
    pub fn sample_format(&self) -> AudioSampleFormat {
        self.data.sample_format()
    }

    /// Returns samples per channel.
    pub fn samples_per_channel(&self) -> usize {
        if self.channels == 0 {
            return 0;
        }
        self.data.len() / self.channels as usize
    }

    /// Returns duration seconds.
    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.samples_per_channel() as f64 / self.sample_rate as f64
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
/// Data type for audio analysis result.
pub struct AudioAnalysisResult {
    /// The events value.
    pub events: Vec<AnalysisEvent>,
    /// The frames processed value.
    pub frames_processed: u64,
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for audio analysis.
pub struct AudioAnalysis {
    /// Timestamp associated with this value.
    pub timestamp: Timestamp,
    /// The events value.
    pub events: Vec<AnalysisEvent>,
    /// The frames processed value.
    pub frames_processed: u64,
}

/// Trait for audio analyzer implementations.
pub trait AudioAnalyzer {
    /// Returns name.
    fn name(&self) -> &str;

    /// Returns process frame.
    fn process_frame(&mut self, frame: &AudioFrame<'_>) -> Result<Vec<AnalysisEvent>>;

    /// Returns finish.
    fn finish(&mut self, _last_timestamp: Option<Timestamp>) -> Result<Vec<AnalysisEvent>> {
        Ok(Vec::new())
    }
}

/// Data type for audio pipeline.
pub struct AudioPipeline {
    analyzers: Vec<Box<dyn AudioAnalyzer>>,
    state: AudioPipelineState,
}

#[derive(Debug, Default, Clone)]
struct AudioPipelineState {
    events: Vec<AnalysisEvent>,
    last_timestamp: Option<Timestamp>,
    frames_processed: u64,
    finished: bool,
}

impl AudioPipeline {
    /// Returns builder.
    pub fn builder() -> AudioPipelineBuilder {
        AudioPipelineBuilder::default()
    }

    /// Returns process frame.
    pub fn process_frame(&mut self, frame: OwnedAudioFrame) -> Result<AudioAnalysis> {
        if self.state.finished {
            return Err(DetectError::InvalidArgument(
                "cannot process audio frames after finish_analysis; call reset first".to_string(),
            ));
        }
        let frame_ref = frame.as_frame()?;
        self.state.last_timestamp = Some(frame_ref.timestamp);

        let mut events = Vec::new();
        for analyzer in &mut self.analyzers {
            let mut new_events = analyzer.process_frame(&frame_ref)?;
            events.append(&mut new_events);
        }
        self.state.events.extend(events.iter().cloned());
        self.state.frames_processed += 1;

        Ok(AudioAnalysis {
            timestamp: frame_ref.timestamp,
            events,
            frames_processed: self.state.frames_processed,
        })
    }

    /// Returns finish analysis.
    pub fn finish_analysis(&mut self) -> Result<AudioAnalysisResult> {
        if !self.state.finished {
            let mut events = Vec::new();
            for analyzer in &mut self.analyzers {
                let mut new_events = analyzer.finish(self.state.last_timestamp)?;
                events.append(&mut new_events);
            }
            self.state.events.extend(events);
            self.state.finished = true;
        }
        Ok(AudioAnalysisResult {
            events: self.state.events.clone(),
            frames_processed: self.state.frames_processed,
        })
    }

    /// Returns reset.
    pub fn reset(&mut self) {
        self.state = AudioPipelineState::default();
    }

    /// Returns events.
    pub fn events(&self) -> &[AnalysisEvent] {
        &self.state.events
    }

    /// Returns frames processed.
    pub fn frames_processed(&self) -> u64 {
        self.state.frames_processed
    }
}

#[derive(Default)]
/// Data type for audio pipeline builder.
pub struct AudioPipelineBuilder {
    analyzers: Vec<Box<dyn AudioAnalyzer>>,
}

impl AudioPipelineBuilder {
    /// Returns analyzer.
    pub fn analyzer<A: AudioAnalyzer + 'static>(mut self, analyzer: A) -> Self {
        self.analyzers.push(Box::new(analyzer));
        self
    }

    /// Returns build.
    pub fn build(self) -> Result<AudioPipeline> {
        if self.analyzers.is_empty() {
            return Err(DetectError::InvalidArgument(
                "at least one audio analyzer is required".to_string(),
            ));
        }
        Ok(AudioPipeline {
            analyzers: self.analyzers,
            state: AudioPipelineState::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopAnalyzer;

    impl AudioAnalyzer for NoopAnalyzer {
        fn name(&self) -> &str {
            "noop"
        }

        fn process_frame(&mut self, _frame: &AudioFrame<'_>) -> Result<Vec<AnalysisEvent>> {
            Ok(Vec::new())
        }
    }

    fn timestamp() -> Timestamp {
        Timestamp::new(0, Timebase::new(1, 48_000))
    }

    #[test]
    fn audio_buffer_reports_its_shape() {
        let buffer = AudioBuffer::F32(vec![0.0, 0.5, -0.5, 1.0]);

        assert_eq!(buffer.len(), 4);
        assert!(!buffer.is_empty());
        assert_eq!(buffer.sample_format(), AudioSampleFormat::F32);
    }

    #[test]
    fn audio_frame_rejects_an_invalid_format() {
        let buffer = AudioBuffer::I16(vec![0, 1]);

        let error = AudioFrame::new(timestamp(), 0, 1, &buffer).unwrap_err();

        assert!(matches!(
            error,
            DetectError::InvalidAudioFormat {
                sample_rate: 0,
                channels: 1
            }
        ));
    }

    #[test]
    fn pipeline_requires_an_analyzer_and_can_be_reset_after_finish() {
        assert!(matches!(
            AudioPipeline::builder().build().err().unwrap(),
            DetectError::InvalidArgument(message)
                if message == "at least one audio analyzer is required"
        ));

        let mut pipeline = AudioPipeline::builder()
            .analyzer(NoopAnalyzer)
            .build()
            .unwrap();
        let frame = || {
            OwnedAudioFrame::new(timestamp(), 48_000, 1, AudioBuffer::F32(vec![0.0, 0.5])).unwrap()
        };

        let analysis = pipeline.process_frame(frame()).unwrap();
        assert_eq!(analysis.frames_processed, 1);
        assert_eq!(pipeline.finish_analysis().unwrap().frames_processed, 1);
        assert!(matches!(
            pipeline.process_frame(frame()).unwrap_err(),
            DetectError::InvalidArgument(message)
                if message.contains("after finish_analysis")
        ));

        pipeline.reset();
        assert_eq!(pipeline.process_frame(frame()).unwrap().frames_processed, 1);
    }
}
