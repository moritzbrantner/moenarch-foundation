//! Library-owned runtime surface for `math-signal-core`.

use runtime_core::{
    describe_surface_response, parse_surface_input, structured_operation_response,
    surface_operation, validate_max_items, OperationId, PackageSurface, RuntimeCapabilities,
    SurfaceError, SurfaceOperation, SurfaceRequest, SurfaceResponse,
};
use serde::Deserialize;

use crate::{
    apply_fir_mono, design_parametric_biquad, normalize_peak, resample_indices, signal_levels,
    FirKernel1d, FrameStride, InterpolationMode, ParametricBiquadDesign, ResampleSpec, SampleRate,
};

const DEFAULT_PREVIEW: usize = 16;
const MAX_PREVIEW: usize = 256;
const MAX_VALUES: usize = 100_000;

/// Returns the package surface exposed by every transport wrapper.
pub fn package_surface() -> PackageSurface {
    PackageSurface {
        library: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: RuntimeCapabilities::pure_rust(),
        operations: vec![
            operation(
                "describe",
                "Describe package",
                "Shared signal-domain math for windows, frame strides, resampling, and biquad design.",
                serde_json::json!({"includeOperations": true}),
            ),
            operation(
                "signal.frames",
                "Signal frames",
                "Computes frame count and preview mean/RMS summaries for a finite mono sample buffer.",
                serde_json::json!({"samples": [0.0, 1.0, 0.0, -1.0], "frameSize": 2, "hopSize": 1}),
            ),
            operation(
                "signal.resamplePlan",
                "Resample plan",
                "Returns output length and source-position preview indices for a sample-rate conversion.",
                serde_json::json!({"inputRate": 48_000, "outputRate": 16_000, "inputLen": 480}),
            ),
            operation(
                "signal.filterDesign",
                "Biquad filter design",
                "Designs normalized biquad coefficients for supported filter kinds.",
                serde_json::json!({"kind": "lowPass", "sampleRate": 48_000, "frequencyHz": 1_000.0, "q": 0.707}),
            ),
            operation(
                "signal.levels",
                "Signal levels",
                "Computes peak, RMS, mean, and DC offset for a finite mono sample buffer.",
                serde_json::json!({"samples": [0.0, 0.5, -1.0, 0.25]}),
            ),
            operation(
                "signal.filterApply",
                "Apply FIR filter",
                "Applies a centered FIR kernel to a finite mono sample buffer.",
                serde_json::json!({"samples": [0.0, 1.0, 0.0], "kernel": [0.25, 0.5, 0.25]}),
            ),
            operation(
                "signal.normalizePeak",
                "Normalize peak",
                "Scales a finite mono sample buffer to a requested peak amplitude.",
                serde_json::json!({"samples": [0.0, 0.5, -1.0], "targetPeak": 0.5}),
            ),
        ],
    }
}

fn operation(
    id: &str,
    name: &str,
    description: &str,
    example_request: serde_json::Value,
) -> SurfaceOperation {
    surface_operation(id, name, description, example_request)
}

/// Runs one library-owned operation.
pub fn run_surface_operation(request: SurfaceRequest) -> Result<SurfaceResponse, String> {
    let surface = package_surface();
    let operation = request.operation.clone();
    let value = match request.operation.as_str() {
        "describe" => return Ok(describe_surface_response(&surface, request)),
        "signal.frames" => frames_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "signal.resamplePlan" => resample_plan_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "signal.filterDesign" => filter_design_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "signal.levels" => levels_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "signal.filterApply" => filter_apply_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "signal.normalizePeak" => normalize_peak_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        operation => {
            return Err(
                SurfaceError::unsupported_operation(operation, env!("CARGO_PKG_NAME"))
                    .to_error_string(),
            );
        }
    };
    Ok(structured_operation_response(&surface, operation, value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FramesRequest {
    samples: Vec<f32>,
    frame_size: usize,
    hop_size: usize,
    #[serde(default = "default_preview")]
    preview_frames: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResamplePlanRequest {
    input_rate: u32,
    output_rate: u32,
    input_len: usize,
    #[serde(default = "default_resample_mode")]
    mode: String,
    #[serde(default = "default_preview")]
    preview_indices: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilterDesignRequest {
    kind: String,
    sample_rate: u32,
    frequency_hz: f32,
    q: f32,
    #[serde(default)]
    gain_db: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SamplesRequest {
    samples: Vec<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilterApplyRequest {
    samples: Vec<f32>,
    kernel: Vec<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NormalizePeakRequest {
    samples: Vec<f32>,
    target_peak: f32,
}

fn frames_value(operation: &str, request: FramesRequest) -> Result<serde_json::Value, String> {
    validate_values(operation, "samples", &request.samples)?;
    let stride = FrameStride::new(request.frame_size, request.hop_size)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let frame_count = stride.frame_count(request.samples.len());
    let preview_count = request.preview_frames.min(MAX_PREVIEW).min(frame_count);
    let frames = (0..preview_count)
        .map(|index| {
            let start = index * request.hop_size;
            let frame = &request.samples[start..start + request.frame_size];
            let mean = frame.iter().sum::<f32>() / frame.len() as f32;
            let rms = (frame.iter().map(|sample| sample * sample).sum::<f32>()
                / frame.len() as f32)
                .sqrt();
            serde_json::json!({
                "start": start,
                "len": frame.len(),
                "mean": mean,
                "rms": rms
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "sampleCount": request.samples.len(),
        "frameSize": request.frame_size,
        "hopSize": request.hop_size,
        "frameCount": frame_count,
        "frames": frames
    }))
}

fn resample_plan_value(
    operation: &str,
    request: ResamplePlanRequest,
) -> Result<serde_json::Value, String> {
    validate_max_items(operation, "inputLen", request.input_len, MAX_VALUES)?;
    let input = SampleRate::new(request.input_rate)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let output = SampleRate::new(request.output_rate)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let mode = parse_interpolation_mode(operation, &request.mode)?;
    let spec = ResampleSpec::new(input, output, mode)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let ratio = spec.ratio().as_f64();
    let output_len = if request.input_len == 0 {
        0
    } else {
        ((request.input_len as f64) * ratio).round().max(1.0) as usize
    };
    let preview_len = request.preview_indices.min(MAX_PREVIEW).min(output_len);
    let indices = resample_indices(spec, preview_len)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    Ok(serde_json::json!({
        "inputRate": request.input_rate,
        "outputRate": request.output_rate,
        "ratio": ratio,
        "inputLen": request.input_len,
        "outputLen": output_len,
        "indicesPreview": indices
    }))
}

fn filter_design_value(
    operation: &str,
    request: FilterDesignRequest,
) -> Result<serde_json::Value, String> {
    let sample_rate = SampleRate::new(request.sample_rate)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let design = parse_filter_kind(operation, &request.kind, request.gain_db)?;
    let coefficients =
        design_parametric_biquad(design, sample_rate, request.frequency_hz, request.q)
            .map_err(|error| invalid_request(operation, error.to_string()))?;
    Ok(serde_json::json!({
        "kind": request.kind,
        "sampleRate": request.sample_rate,
        "frequencyHz": request.frequency_hz,
        "q": request.q,
        "coefficients": {
            "b0": coefficients.b0,
            "b1": coefficients.b1,
            "b2": coefficients.b2,
            "a1": coefficients.a1,
            "a2": coefficients.a2
        }
    }))
}

fn levels_value(operation: &str, request: SamplesRequest) -> Result<serde_json::Value, String> {
    validate_values(operation, "samples", &request.samples)?;
    let levels = signal_levels(&request.samples)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    Ok(serde_json::json!({
        "count": levels.count,
        "peak": levels.peak,
        "rms": levels.rms,
        "mean": levels.mean,
        "dcOffset": levels.dc_offset
    }))
}

fn filter_apply_value(
    operation: &str,
    request: FilterApplyRequest,
) -> Result<serde_json::Value, String> {
    validate_values(operation, "samples", &request.samples)?;
    validate_values(operation, "kernel", &request.kernel)?;
    let kernel = FirKernel1d::new(request.kernel)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let samples = apply_fir_mono(&request.samples, &kernel)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    Ok(serde_json::json!({
        "sampleCount": samples.len(),
        "kernelLen": kernel.values().len(),
        "samples": samples
    }))
}

fn normalize_peak_value(
    operation: &str,
    request: NormalizePeakRequest,
) -> Result<serde_json::Value, String> {
    validate_values(operation, "samples", &request.samples)?;
    let input_peak = if request.samples.is_empty() {
        0.0
    } else {
        signal_levels(&request.samples)
            .map_err(|error| invalid_request(operation, error.to_string()))?
            .peak
    };
    let samples = normalize_peak(&request.samples, request.target_peak)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let output_peak = if samples.is_empty() {
        0.0
    } else {
        signal_levels(&samples)
            .map_err(|error| invalid_request(operation, error.to_string()))?
            .peak
    };
    Ok(serde_json::json!({
        "inputPeak": input_peak,
        "targetPeak": request.target_peak,
        "outputPeak": output_peak,
        "samples": samples
    }))
}

fn parse_interpolation_mode(operation: &str, mode: &str) -> Result<InterpolationMode, String> {
    match mode {
        "nearest" => Ok(InterpolationMode::Nearest),
        "linear" => Ok(InterpolationMode::Linear),
        _ => Err(SurfaceError::unsupported_value(
            Some(OperationId::new(operation)),
            "mode",
            mode,
            &["nearest", "linear"],
        )
        .to_error_string()),
    }
}

fn parse_filter_kind(
    operation: &str,
    kind: &str,
    gain_db: Option<f32>,
) -> Result<ParametricBiquadDesign, String> {
    match kind {
        "lowPass" => Ok(ParametricBiquadDesign::LowPass),
        "highPass" => Ok(ParametricBiquadDesign::HighPass),
        "bandPass" => Ok(ParametricBiquadDesign::BandPass),
        "notch" => Ok(ParametricBiquadDesign::Notch),
        "peakingEq" => Ok(ParametricBiquadDesign::PeakingEq {
            gain_db: finite_gain(operation, gain_db)?,
        }),
        "lowShelf" => Ok(ParametricBiquadDesign::LowShelf {
            gain_db: finite_gain(operation, gain_db)?,
        }),
        "highShelf" => Ok(ParametricBiquadDesign::HighShelf {
            gain_db: finite_gain(operation, gain_db)?,
        }),
        "allPass" => Ok(ParametricBiquadDesign::AllPass),
        _ => Err(SurfaceError::unsupported_value(
            Some(OperationId::new(operation)),
            "kind",
            kind,
            &[
                "lowPass",
                "highPass",
                "bandPass",
                "notch",
                "peakingEq",
                "lowShelf",
                "highShelf",
                "allPass",
            ],
        )
        .to_error_string()),
    }
}

fn finite_gain(operation: &str, gain_db: Option<f32>) -> Result<f32, String> {
    let gain_db = gain_db.unwrap_or(0.0);
    if !gain_db.is_finite() {
        return Err(invalid_request(operation, "gainDb must be finite"));
    }
    Ok(gain_db)
}

fn validate_values(operation: &str, field: &str, values: &[f32]) -> Result<(), String> {
    validate_max_items(operation, field, values.len(), MAX_VALUES)?;
    if values.iter().any(|value| !value.is_finite()) {
        return Err(invalid_request(
            operation,
            format!("{field} must be finite"),
        ));
    }
    Ok(())
}

fn invalid_request(operation: &str, message: impl Into<String>) -> String {
    SurfaceError::invalid_request(Some(OperationId::new(operation)), message).to_error_string()
}

fn default_preview() -> usize {
    DEFAULT_PREVIEW
}

fn default_resample_mode() -> String {
    "linear".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_return_preview_stats() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("signal.frames"),
            input: serde_json::json!({"samples": [0.0, 1.0, 0.0], "frameSize": 2, "hopSize": 1}),
        })
        .expect("frames operation");

        assert_eq!(response.value["sampleCount"], 3);
        assert_eq!(response.value["frameCount"], 2);
        assert_eq!(response.value["frames"][0]["start"], 0);
    }

    #[test]
    fn resample_plan_returns_indices() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("signal.resamplePlan"),
            input: serde_json::json!({"inputRate": 4, "outputRate": 8, "inputLen": 3, "previewIndices": 3}),
        })
        .expect("resample plan operation");

        assert_eq!(response.value["outputLen"], 6);
        assert_eq!(
            response.value["indicesPreview"],
            serde_json::json!([0.0, 0.5, 1.0])
        );
    }

    #[test]
    fn filter_design_returns_coefficients() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("signal.filterDesign"),
            input: serde_json::json!({"kind": "lowPass", "sampleRate": 48_000, "frequencyHz": 1_000.0, "q": 0.707}),
        })
        .expect("filter design operation");

        assert_eq!(response.value["kind"], "lowPass");
        assert!(response.value["coefficients"]["b0"].is_number());
    }

    #[test]
    fn new_signal_operations_run() {
        for operation in [
            "signal.levels",
            "signal.filterApply",
            "signal.normalizePeak",
        ] {
            let surface_operation = package_surface()
                .operations
                .into_iter()
                .find(|candidate| candidate.id.as_str() == operation)
                .expect("operation metadata");
            let response = run_surface_operation(SurfaceRequest {
                operation: surface_operation.id,
                input: surface_operation.example_request,
            })
            .unwrap_or_else(|error| panic!("{operation} failed: {error}"));
            assert!(response.value.is_object());
        }
    }
}
