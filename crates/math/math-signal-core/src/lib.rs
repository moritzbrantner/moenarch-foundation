#![doc = include_str!("../README.md")]

pub mod surface;
use media_core::{DetectError, Result};

fn invalid_argument(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Data type for sample rate.
pub struct SampleRate(pub u32);

impl SampleRate {
    /// Creates a new value.
    pub fn new(value: u32) -> Result<Self> {
        if value == 0 {
            return Err(DetectError::InvalidAudioFormat {
                sample_rate: value,
                channels: 1,
            });
        }
        Ok(Self(value))
    }

    /// Returns get.
    pub fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Data type for resample ratio.
pub struct ResampleRatio {
    /// The input value.
    pub input: SampleRate,
    /// The output value.
    pub output: SampleRate,
}

impl ResampleRatio {
    /// Creates a new value.
    pub fn new(input: SampleRate, output: SampleRate) -> Result<Self> {
        let ratio = Self { input, output };
        ratio.validate()?;
        Ok(ratio)
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        SampleRate::new(self.input.get())?;
        SampleRate::new(self.output.get())?;
        Ok(())
    }

    /// Borrows this value as a f64.
    pub fn as_f64(self) -> f64 {
        self.output.get() as f64 / self.input.get() as f64
    }

    /// Returns source position for output.
    pub fn source_position_for_output(self, output_index: usize) -> f64 {
        output_index as f64 * self.input.get() as f64 / self.output.get() as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Variants describing interpolation mode.
pub enum InterpolationMode {
    /// The nearest variant.
    Nearest,
    /// The linear variant.
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Basic finite mono signal level summary.
pub struct SignalLevels {
    /// Number of samples summarized.
    pub count: usize,
    /// Maximum absolute sample value.
    pub peak: f32,
    /// Root mean square amplitude.
    pub rms: f32,
    /// Arithmetic mean sample value.
    pub mean: f32,
    /// Alias for mean, named for audio DC offset workflows.
    pub dc_offset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Data type for resample spec.
pub struct ResampleSpec {
    /// The input value.
    pub input: SampleRate,
    /// The output value.
    pub output: SampleRate,
    /// The interpolation value.
    pub interpolation: InterpolationMode,
}

impl ResampleSpec {
    /// Creates a new value.
    pub fn new(
        input: SampleRate,
        output: SampleRate,
        interpolation: InterpolationMode,
    ) -> Result<Self> {
        let spec = Self {
            input,
            output,
            interpolation,
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        ResampleRatio::new(self.input, self.output)?;
        Ok(())
    }

    /// Returns ratio.
    pub fn ratio(self) -> ResampleRatio {
        ResampleRatio {
            input: self.input,
            output: self.output,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Variants describing window function.
pub enum WindowFunction {
    /// The rectangular variant.
    Rectangular,
    /// The hann variant.
    Hann,
    /// The hamming variant.
    Hamming,
    /// The blackman variant.
    Blackman,
}

impl WindowFunction {
    /// Returns coefficient.
    pub fn coefficient(self, index: usize, len: usize) -> f32 {
        if len <= 1 {
            return 1.0;
        }
        let phase = 2.0 * std::f32::consts::PI * index as f32 / (len - 1) as f32;
        match self {
            Self::Rectangular => 1.0,
            Self::Hann => 0.5 - 0.5 * phase.cos(),
            Self::Hamming => 0.54 - 0.46 * phase.cos(),
            Self::Blackman => 0.42 - 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos(),
        }
    }

    /// Returns weights.
    pub fn weights(self, len: usize) -> Vec<f32> {
        (0..len).map(|index| self.coefficient(index, len)).collect()
    }

    /// Returns apply.
    pub fn apply(self, samples: &[f32]) -> Vec<f32> {
        samples
            .iter()
            .enumerate()
            .map(|(index, sample)| sample * self.coefficient(index, samples.len()))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Data type for window spec.
pub struct WindowSpec {
    /// The function value.
    pub function: WindowFunction,
    /// The len value.
    pub len: usize,
}

impl WindowSpec {
    /// Creates a new value.
    pub fn new(function: WindowFunction, len: usize) -> Result<Self> {
        let spec = Self { function, len };
        spec.validate()?;
        Ok(spec)
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if self.len == 0 {
            return Err(invalid_argument("window length must be greater than zero"));
        }
        Ok(())
    }

    /// Returns weights.
    pub fn weights(self) -> Vec<f32> {
        self.function.weights(self.len)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Data type for frame stride.
pub struct FrameStride {
    /// The frame size value.
    pub frame_size: usize,
    /// The hop size value.
    pub hop_size: usize,
}

impl FrameStride {
    /// Creates a new value.
    pub fn new(frame_size: usize, hop_size: usize) -> Result<Self> {
        if frame_size == 0 || hop_size == 0 {
            return Err(invalid_argument(
                "frame_size and hop_size must be greater than zero",
            ));
        }
        Ok(Self {
            frame_size,
            hop_size,
        })
    }

    /// Returns frame count.
    pub fn frame_count(self, samples_len: usize) -> usize {
        if samples_len < self.frame_size {
            return 0;
        }
        1 + (samples_len - self.frame_size) / self.hop_size
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Variants describing biquad design.
pub enum BiquadDesign {
    /// The low pass variant.
    LowPass,
    /// The high pass variant.
    HighPass,
    /// The band pass variant.
    BandPass,
    /// The notch variant.
    Notch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Variants describing parametric biquad design.
pub enum ParametricBiquadDesign {
    /// The low pass variant.
    LowPass,
    /// The high pass variant.
    HighPass,
    /// The band pass variant.
    BandPass,
    /// The notch variant.
    Notch,
    /// Peaking equalizer with gain in dB.
    PeakingEq { gain_db: f32 },
    /// Low shelf with gain in dB.
    LowShelf { gain_db: f32 },
    /// High shelf with gain in dB.
    HighShelf { gain_db: f32 },
    /// The all pass variant.
    AllPass,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Data type for biquad coefficients.
pub struct BiquadCoefficients {
    /// The b0 value.
    pub b0: f32,
    /// The b1 value.
    pub b1: f32,
    /// The b2 value.
    pub b2: f32,
    /// The a1 value.
    pub a1: f32,
    /// The a2 value.
    pub a2: f32,
}

impl BiquadCoefficients {
    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if [self.b0, self.b1, self.b2, self.a1, self.a2]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(invalid_argument("biquad coefficients must be finite"));
        }
        Ok(())
    }
}

impl BiquadDesign {
    /// Returns design.
    pub fn design(
        self,
        sample_rate: SampleRate,
        cutoff_hz: f32,
        q: f32,
    ) -> Result<BiquadCoefficients> {
        if !cutoff_hz.is_finite() || cutoff_hz <= 0.0 || !q.is_finite() || q <= 0.0 {
            return Err(invalid_argument(
                "biquad cutoff must be finite and positive, and q must be finite and positive",
            ));
        }
        let nyquist = sample_rate.get() as f32 / 2.0;
        if cutoff_hz >= nyquist {
            return Err(invalid_argument("biquad cutoff must be below Nyquist"));
        }
        let omega = 2.0 * std::f32::consts::PI * cutoff_hz / sample_rate.get() as f32;
        let cos = omega.cos();
        let sin = omega.sin();
        let alpha = sin / (2.0 * q);

        let (b0, b1, b2, a0, a1, a2) = match self {
            Self::LowPass => (
                (1.0 - cos) / 2.0,
                1.0 - cos,
                (1.0 - cos) / 2.0,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
            Self::HighPass => (
                (1.0 + cos) / 2.0,
                -(1.0 + cos),
                (1.0 + cos) / 2.0,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
            Self::BandPass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            Self::Notch => (1.0, -2.0 * cos, 1.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
        };
        BiquadCoefficients {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
        .validate()?;
        Ok(BiquadCoefficients {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        })
    }
}

/// Designs a parametric biquad filter.
pub fn design_parametric_biquad(
    design: ParametricBiquadDesign,
    sample_rate: SampleRate,
    frequency_hz: f32,
    q: f32,
) -> Result<BiquadCoefficients> {
    if !frequency_hz.is_finite() || frequency_hz <= 0.0 || !q.is_finite() || q <= 0.0 {
        return Err(invalid_argument(
            "biquad frequency must be finite and positive, and q must be finite and positive",
        ));
    }
    let nyquist = sample_rate.get() as f32 / 2.0;
    if frequency_hz >= nyquist {
        return Err(invalid_argument("biquad frequency must be below Nyquist"));
    }
    match design {
        ParametricBiquadDesign::LowPass => {
            return BiquadDesign::LowPass.design(sample_rate, frequency_hz, q);
        }
        ParametricBiquadDesign::HighPass => {
            return BiquadDesign::HighPass.design(sample_rate, frequency_hz, q);
        }
        ParametricBiquadDesign::BandPass => {
            return BiquadDesign::BandPass.design(sample_rate, frequency_hz, q);
        }
        ParametricBiquadDesign::Notch => {
            return BiquadDesign::Notch.design(sample_rate, frequency_hz, q);
        }
        _ => {}
    }

    let omega = 2.0 * std::f32::consts::PI * frequency_hz / sample_rate.get() as f32;
    let cos = omega.cos();
    let sin = omega.sin();
    let alpha = sin / (2.0 * q);
    let (b0, b1, b2, a0, a1, a2) = match design {
        ParametricBiquadDesign::PeakingEq { gain_db } => {
            let a = 10.0_f32.powf(gain_db / 40.0);
            (
                1.0 + alpha * a,
                -2.0 * cos,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cos,
                1.0 - alpha / a,
            )
        }
        ParametricBiquadDesign::LowShelf { gain_db } => {
            let a = 10.0_f32.powf(gain_db / 40.0);
            let sqrt_a = a.sqrt();
            let shelf_alpha = sin / 2.0 * ((a + 1.0 / a) * (1.0 / q - 1.0) + 2.0).sqrt();
            (
                a * ((a + 1.0) - (a - 1.0) * cos + 2.0 * sqrt_a * shelf_alpha),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                a * ((a + 1.0) - (a - 1.0) * cos - 2.0 * sqrt_a * shelf_alpha),
                (a + 1.0) + (a - 1.0) * cos + 2.0 * sqrt_a * shelf_alpha,
                -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                (a + 1.0) + (a - 1.0) * cos - 2.0 * sqrt_a * shelf_alpha,
            )
        }
        ParametricBiquadDesign::HighShelf { gain_db } => {
            let a = 10.0_f32.powf(gain_db / 40.0);
            let sqrt_a = a.sqrt();
            let shelf_alpha = sin / 2.0 * ((a + 1.0 / a) * (1.0 / q - 1.0) + 2.0).sqrt();
            (
                a * ((a + 1.0) + (a - 1.0) * cos + 2.0 * sqrt_a * shelf_alpha),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                a * ((a + 1.0) + (a - 1.0) * cos - 2.0 * sqrt_a * shelf_alpha),
                (a + 1.0) - (a - 1.0) * cos + 2.0 * sqrt_a * shelf_alpha,
                2.0 * ((a - 1.0) - (a + 1.0) * cos),
                (a + 1.0) - (a - 1.0) * cos - 2.0 * sqrt_a * shelf_alpha,
            )
        }
        ParametricBiquadDesign::AllPass => (
            1.0 - alpha,
            -2.0 * cos,
            1.0 + alpha,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        ),
        ParametricBiquadDesign::LowPass
        | ParametricBiquadDesign::HighPass
        | ParametricBiquadDesign::BandPass
        | ParametricBiquadDesign::Notch => unreachable!(),
    };
    BiquadCoefficients {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
    .validate()?;
    Ok(BiquadCoefficients {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    })
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for fir kernel1d.
pub struct FirKernel1d {
    values: Vec<f32>,
}

impl FirKernel1d {
    /// Creates a new value.
    pub fn new(values: impl Into<Vec<f32>>) -> Result<Self> {
        let kernel = Self {
            values: values.into(),
        };
        kernel.validate()?;
        Ok(kernel)
    }

    /// Returns values.
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Validates this value.
    pub fn validate(&self) -> Result<()> {
        if self.values.is_empty() {
            return Err(invalid_argument("FIR kernel must not be empty"));
        }
        if self.values.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("FIR kernel values must be finite"));
        }
        Ok(())
    }
}

/// Returns interpolate at.
pub fn interpolate_at(samples: &[f32], position: f64, mode: InterpolationMode) -> Result<f32> {
    if samples.is_empty() {
        return Err(invalid_argument("samples must not be empty"));
    }
    if !position.is_finite() || position < 0.0 {
        return Err(invalid_argument(
            "interpolation position must be finite and non-negative",
        ));
    }
    let max_index = samples.len().saturating_sub(1) as f64;
    let clamped = position.min(max_index);
    Ok(match mode {
        InterpolationMode::Nearest => samples[clamped.round() as usize],
        InterpolationMode::Linear => {
            let left = clamped.floor() as usize;
            let right = clamped.ceil() as usize;
            if left == right {
                samples[left]
            } else {
                let fraction = (clamped - left as f64) as f32;
                samples[left] * (1.0 - fraction) + samples[right] * fraction
            }
        }
    })
}

/// Resamples a mono buffer with the requested interpolation mode.
pub fn resample_mono(
    samples: &[f32],
    input_rate: SampleRate,
    output_rate: SampleRate,
    mode: InterpolationMode,
) -> Result<Vec<f32>> {
    ResampleRatio::new(input_rate, output_rate)?;
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    if samples.iter().any(|sample| !sample.is_finite()) {
        return Err(invalid_argument("samples must be finite"));
    }
    if input_rate == output_rate {
        return Ok(samples.to_vec());
    }
    let ratio = output_rate.get() as f64 / input_rate.get() as f64;
    let output_len = ((samples.len() as f64) * ratio).round().max(1.0) as usize;
    let spec = ResampleSpec::new(input_rate, output_rate, mode)?;
    (0..output_len)
        .map(|index| {
            interpolate_at(
                samples,
                spec.ratio().source_position_for_output(index),
                mode,
            )
        })
        .collect()
}

/// Resamples interleaved audio with independent per-channel interpolation.
pub fn resample_interleaved(
    samples: &[f32],
    channels: u16,
    input_rate: SampleRate,
    output_rate: SampleRate,
    mode: InterpolationMode,
) -> Result<Vec<f32>> {
    if channels == 0 {
        return Err(DetectError::InvalidAudioFormat {
            sample_rate: input_rate.get(),
            channels,
        });
    }
    if !samples.len().is_multiple_of(channels as usize) {
        return Err(invalid_argument(
            "interleaved sample length must be divisible by channel count",
        ));
    }
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    if input_rate == output_rate {
        return Ok(samples.to_vec());
    }
    let channels_usize = channels as usize;
    let frames = samples.len() / channels_usize;
    let ratio = output_rate.get() as f64 / input_rate.get() as f64;
    let output_frames = ((frames as f64) * ratio).round().max(1.0) as usize;
    let spec = ResampleSpec::new(input_rate, output_rate, mode)?;
    let mut output = Vec::with_capacity(output_frames * channels_usize);
    for output_frame in 0..output_frames {
        let position = spec.ratio().source_position_for_output(output_frame);
        for channel in 0..channels_usize {
            output.push(interpolate_interleaved_channel(
                samples,
                channels_usize,
                channel,
                position,
                mode,
            )?);
        }
    }
    Ok(output)
}

fn interpolate_interleaved_channel(
    samples: &[f32],
    channels: usize,
    channel: usize,
    position: f64,
    mode: InterpolationMode,
) -> Result<f32> {
    if !position.is_finite() || position < 0.0 {
        return Err(invalid_argument(
            "interpolation position must be finite and non-negative",
        ));
    }
    let frames = samples.len() / channels;
    let max_index = frames.saturating_sub(1) as f64;
    let clamped = position.min(max_index);
    Ok(match mode {
        InterpolationMode::Nearest => samples[clamped.round() as usize * channels + channel],
        InterpolationMode::Linear => {
            let left = clamped.floor() as usize;
            let right = clamped.ceil() as usize;
            if left == right {
                samples[left * channels + channel]
            } else {
                let fraction = (clamped - left as f64) as f32;
                let left_sample = samples[left * channels + channel];
                let right_sample = samples[right * channels + channel];
                left_sample * (1.0 - fraction) + right_sample * fraction
            }
        }
    })
}

/// Computes peak, RMS, mean, and DC offset for a finite mono signal.
pub fn signal_levels(samples: &[f32]) -> Result<SignalLevels> {
    validate_samples(samples, "samples")?;
    if samples.is_empty() {
        return Err(invalid_argument("samples must not be empty"));
    }
    let count = samples.len();
    let peak = samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, f32::max);
    let mean = samples.iter().sum::<f32>() / count as f32;
    let rms = (samples.iter().map(|sample| sample * sample).sum::<f32>() / count as f32).sqrt();
    Ok(SignalLevels {
        count,
        peak,
        rms,
        mean,
        dc_offset: mean,
    })
}

/// Applies a centered FIR kernel to mono samples, zero-padding beyond boundaries.
pub fn apply_fir_mono(samples: &[f32], kernel: &FirKernel1d) -> Result<Vec<f32>> {
    validate_samples(samples, "samples")?;
    kernel.validate()?;
    let center = kernel.values().len() / 2;
    let mut output = Vec::with_capacity(samples.len());
    for index in 0..samples.len() {
        let mut acc = 0.0;
        for (tap_index, coefficient) in kernel.values().iter().copied().enumerate() {
            let sample_index = index as isize + tap_index as isize - center as isize;
            if (0..samples.len() as isize).contains(&sample_index) {
                acc += samples[sample_index as usize] * coefficient;
            }
        }
        if !acc.is_finite() {
            return Err(invalid_argument("FIR filtering produced non-finite values"));
        }
        output.push(acc);
    }
    Ok(output)
}

/// Scales a finite mono signal to a target peak amplitude.
pub fn normalize_peak(samples: &[f32], target_peak: f32) -> Result<Vec<f32>> {
    validate_samples(samples, "samples")?;
    if !target_peak.is_finite() || target_peak < 0.0 {
        return Err(invalid_argument(
            "target peak must be finite and non-negative",
        ));
    }
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    let peak = signal_levels(samples)?.peak;
    if peak <= f32::EPSILON {
        if target_peak == 0.0 {
            return Ok(samples.to_vec());
        }
        return Err(invalid_argument(
            "cannot normalize a silent signal to a non-zero peak",
        ));
    }
    let scale = target_peak / peak;
    samples
        .iter()
        .map(|sample| {
            let value = sample * scale;
            if value.is_finite() {
                Ok(value)
            } else {
                Err(invalid_argument(
                    "peak normalization produced non-finite values",
                ))
            }
        })
        .collect()
}

/// Converts decibels to a linear amplitude ratio.
pub fn db_to_linear(db: f32) -> Result<f32> {
    if !db.is_finite() {
        return Err(invalid_argument("dB value must be finite"));
    }
    Ok(10.0_f32.powf(db / 20.0))
}

fn validate_samples(samples: &[f32], label: &str) -> Result<()> {
    if samples.iter().any(|sample| !sample.is_finite()) {
        return Err(invalid_argument(format!("{label} must be finite")));
    }
    Ok(())
}

/// Converts a linear amplitude ratio to decibels.
pub fn linear_to_db(linear: f32) -> Result<f32> {
    if !linear.is_finite() || linear <= 0.0 {
        return Err(invalid_argument(
            "linear amplitude must be finite and greater than zero",
        ));
    }
    Ok(20.0 * linear.log10())
}

/// Returns resample indices.
pub fn resample_indices(spec: ResampleSpec, output_len: usize) -> Result<Vec<f64>> {
    spec.validate()?;
    Ok((0..output_len)
        .map(|index| spec.ratio().source_position_for_output(index))
        .collect())
}

/// Returns FFT bin frequency.
pub fn fft_bin_frequency(bin: usize, fft_size: usize, sample_rate: SampleRate) -> Result<f32> {
    if fft_size == 0 {
        return Err(invalid_argument("fft_size must be greater than zero"));
    }
    if bin > fft_size / 2 {
        return Err(invalid_argument("fft bin must not exceed the Nyquist bin"));
    }
    Ok(bin as f32 * sample_rate.get() as f32 / fft_size as f32)
}

/// Returns frequency to FFT bin.
pub fn frequency_to_fft_bin(
    frequency_hz: f32,
    fft_size: usize,
    sample_rate: SampleRate,
) -> Result<usize> {
    if !frequency_hz.is_finite() || frequency_hz < 0.0 {
        return Err(invalid_argument(
            "frequency must be finite and non-negative",
        ));
    }
    if fft_size == 0 {
        return Err(invalid_argument("fft_size must be greater than zero"));
    }
    let nyquist = sample_rate.get() as f32 / 2.0;
    if frequency_hz > nyquist {
        return Err(invalid_argument("frequency must not exceed Nyquist"));
    }
    Ok((frequency_hz * fft_size as f32 / sample_rate.get() as f32).round() as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_lengths_and_symmetry_match_expectations() {
        let weights = WindowFunction::Hann.weights(4);
        assert_eq!(weights.len(), 4);
        assert!((weights[0] - weights[3]).abs() < 1.0e-6);
    }

    #[test]
    fn interpolation_handles_edge_cases() {
        assert_eq!(
            interpolate_at(&[0.0, 10.0], 0.5, InterpolationMode::Linear).unwrap(),
            5.0
        );
        assert_eq!(
            interpolate_at(&[0.0, 10.0], 2.0, InterpolationMode::Nearest).unwrap(),
            10.0
        );
    }

    #[test]
    fn resample_and_filter_descriptors_validate() {
        let ratio = ResampleRatio::new(
            SampleRate::new(48_000).unwrap(),
            SampleRate::new(16_000).unwrap(),
        )
        .unwrap();
        assert!(ratio.as_f64() < 1.0);
        assert!(BiquadDesign::LowPass
            .design(SampleRate::new(48_000).unwrap(), 1_000.0, 0.707)
            .is_ok());
        assert!(FirKernel1d::new([0.25, 0.5, 0.25]).is_ok());
    }

    #[test]
    fn resampling_and_db_helpers_work() {
        let input = SampleRate::new(4).unwrap();
        let output = SampleRate::new(8).unwrap();
        let resampled =
            resample_mono(&[0.0, 1.0, 0.0], input, output, InterpolationMode::Linear).unwrap();
        assert_eq!(resampled.len(), 6);
        assert_eq!(resampled[0], 0.0);
        assert_eq!(*resampled.last().unwrap(), 0.0);

        let interleaved = resample_interleaved(
            &[0.0, 1.0, 1.0, 0.0],
            2,
            SampleRate::new(2).unwrap(),
            SampleRate::new(4).unwrap(),
            InterpolationMode::Linear,
        )
        .unwrap();
        assert_eq!(interleaved.len(), 8);
        assert!((db_to_linear(6.020_600_3).unwrap() - 2.0).abs() < 1.0e-5);
        assert!(linear_to_db(2.0).unwrap() > 6.0);
    }

    #[test]
    fn parametric_design_supports_eq_variants() {
        for design in [
            ParametricBiquadDesign::PeakingEq { gain_db: 3.0 },
            ParametricBiquadDesign::LowShelf { gain_db: -3.0 },
            ParametricBiquadDesign::HighShelf { gain_db: 3.0 },
            ParametricBiquadDesign::AllPass,
        ] {
            assert!(design_parametric_biquad(
                design,
                SampleRate::new(48_000).unwrap(),
                1_000.0,
                0.707
            )
            .is_ok());
        }
    }

    #[test]
    fn fft_frequency_helpers_round_trip() {
        let sample_rate = SampleRate::new(8_000).unwrap();
        let bin = frequency_to_fft_bin(1_000.0, 8, sample_rate).unwrap();
        assert_eq!(bin, 1);
        assert_eq!(fft_bin_frequency(bin, 8, sample_rate).unwrap(), 1_000.0);
    }
}
