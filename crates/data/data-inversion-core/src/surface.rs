//! Library-owned runtime surface for `data-inversion-core`.

use runtime_core::{
    OperationId, PackageSurface, RuntimeCapabilities, SurfaceOperation, SurfaceRequest,
    SurfaceResponse,
};
use serde::Deserialize;

use crate::{
    validate_confidence, weaker_fidelity, InformationFidelity, InversionMethod, InversionTrace,
};

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
                "Shared fidelity and inversion trace metadata for generated analysis outputs.",
                serde_json::json!({"includeOperations": true}),
            ),
            operation(
                "inversion.trace",
                "Inversion trace",
                "Builds a deterministic inversion trace from fidelity, confidence, assumptions, and notes.",
                serde_json::json!({"sourceType": "histogram", "targetType": "image", "fidelity": "heuristic", "confidence": 0.35, "assumptions": ["scanline layout"], "notes": [{"field": "pixels", "method": "inferred", "message": "expanded from bins"}]}),
            ),
            operation(
                "inversion.confidence",
                "Validate confidence",
                "Validates an inversion confidence value and returns normalized validity information.",
                serde_json::json!({"confidence": 0.8}),
            ),
            operation(
                "inversion.fidelity",
                "Weaker fidelity",
                "Returns the weaker of two information fidelity values.",
                serde_json::json!({"left": "preserved", "right": "heuristic"}),
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
    SurfaceOperation {
        id: OperationId::new(id),
        name: name.to_string(),
        description: Some(description.to_string()),
        curation: runtime_core::SurfaceOperationCuration::from_operation_id(id),
        input_schema: serde_json::json!({"type": "object", "additionalProperties": true, "xOperationCategory": runtime_core::operation_category(id)}),
        output_schema: serde_json::json!({"type": "object", "xOperationCategory": runtime_core::operation_category(id)}),
        example_request,
        wasm_supported: true,
        server_supported: true,
    }
}

/// Runs one library-owned operation.
pub fn run_surface_operation(request: SurfaceRequest) -> Result<SurfaceResponse, String> {
    let operation = request.operation.clone();
    let value = match request.operation.as_str() {
        "describe" => describe_value(request.input),
        "inversion.trace" => trace_value(parse_input(request.input)?)?,
        "inversion.confidence" => confidence_value(parse_input(request.input)?)?,
        "inversion.fidelity" => fidelity_value(parse_input(request.input)?)?,
        operation => {
            return Err(format!(
                "unsupported operation `{operation}` for {}",
                env!("CARGO_PKG_NAME")
            ));
        }
    };
    Ok(response(operation, value))
}

fn describe_value(input: serde_json::Value) -> serde_json::Value {
    let surface = package_surface();
    serde_json::json!({
        "library": surface.library,
        "version": surface.version,
        "operationCount": surface.operations.len(),
        "operations": surface.operations.iter().map(|operation| operation.id.as_str()).collect::<Vec<_>>(),
        "input": input
    })
}

fn response(operation: OperationId, value: serde_json::Value) -> SurfaceResponse {
    SurfaceResponse {
        operation,
        value,
        diagnostics: Vec::new(),
        artifacts: Vec::new(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraceRequest {
    source_type: String,
    target_type: String,
    fidelity: String,
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    assumptions: Vec<String>,
    #[serde(default)]
    notes: Vec<NoteRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NoteRequest {
    field: String,
    method: String,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfidenceRequest {
    confidence: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FidelityRequest {
    left: String,
    right: String,
}

fn trace_value(request: TraceRequest) -> Result<serde_json::Value, String> {
    let mut trace = InversionTrace::new(
        request.source_type,
        request.target_type,
        parse_fidelity(&request.fidelity)?,
    );
    if let Some(confidence) = request.confidence {
        trace = trace
            .confidence(confidence)
            .map_err(|error| error.to_string())?;
    }
    for assumption in request.assumptions {
        trace = trace.assumption(assumption);
    }
    for note in request.notes {
        trace = trace.note(note.field, parse_method(&note.method)?, note.message);
    }
    Ok(trace_json(&trace))
}

fn confidence_value(request: ConfidenceRequest) -> Result<serde_json::Value, String> {
    match validate_confidence(request.confidence) {
        Ok(()) => Ok(serde_json::json!({
            "confidence": request.confidence,
            "valid": true,
            "error": null
        })),
        Err(error) => Ok(serde_json::json!({
            "confidence": request.confidence,
            "valid": false,
            "error": error.to_string()
        })),
    }
}

fn fidelity_value(request: FidelityRequest) -> Result<serde_json::Value, String> {
    let left = parse_fidelity(&request.left)?;
    let right = parse_fidelity(&request.right)?;
    let weaker = weaker_fidelity(left, right);
    Ok(serde_json::json!({
        "left": fidelity_name(left),
        "right": fidelity_name(right),
        "weaker": fidelity_name(weaker)
    }))
}

fn trace_json(trace: &InversionTrace) -> serde_json::Value {
    serde_json::json!({
        "sourceType": trace.source_type,
        "targetType": trace.target_type,
        "fidelity": fidelity_name(trace.fidelity),
        "confidence": trace.confidence,
        "assumptions": trace.assumptions,
        "notes": trace.notes.iter().map(|note| serde_json::json!({
            "field": note.field,
            "method": method_name(note.method),
            "message": note.message
        })).collect::<Vec<_>>()
    })
}

fn parse_fidelity(value: &str) -> Result<InformationFidelity, String> {
    match value {
        "exact" => Ok(InformationFidelity::Exact),
        "preserved" => Ok(InformationFidelity::Preserved),
        "quantized" => Ok(InformationFidelity::Quantized),
        "estimated" => Ok(InformationFidelity::Estimated),
        "interpolated" => Ok(InformationFidelity::Interpolated),
        "heuristic" => Ok(InformationFidelity::Heuristic),
        "placeholder" => Ok(InformationFidelity::Placeholder),
        other => Err(format!("unsupported inversion fidelity `{other}`")),
    }
}

fn fidelity_name(value: InformationFidelity) -> &'static str {
    match value {
        InformationFidelity::Exact => "exact",
        InformationFidelity::Preserved => "preserved",
        InformationFidelity::Quantized => "quantized",
        InformationFidelity::Estimated => "estimated",
        InformationFidelity::Interpolated => "interpolated",
        InformationFidelity::Heuristic => "heuristic",
        InformationFidelity::Placeholder => "placeholder",
    }
}

fn parse_method(value: &str) -> Result<InversionMethod, String> {
    match value {
        "preserved" => Ok(InversionMethod::Preserved),
        "defaulted" => Ok(InversionMethod::Defaulted),
        "quantized" => Ok(InversionMethod::Quantized),
        "inferred" => Ok(InversionMethod::Inferred),
        "interpolated" => Ok(InversionMethod::Interpolated),
        "template" => Ok(InversionMethod::Template),
        "omitted" => Ok(InversionMethod::Omitted),
        other => Err(format!("unsupported inversion method `{other}`")),
    }
}

fn method_name(value: InversionMethod) -> &'static str {
    match value {
        InversionMethod::Preserved => "preserved",
        InversionMethod::Defaulted => "defaulted",
        InversionMethod::Quantized => "quantized",
        InversionMethod::Inferred => "inferred",
        InversionMethod::Interpolated => "interpolated",
        InversionMethod::Template => "template",
        InversionMethod::Omitted => "omitted",
    }
}

fn parse_input<T: for<'de> Deserialize<'de>>(input: serde_json::Value) -> Result<T, String> {
    serde_json::from_value(input).map_err(|error| format!("invalid request: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_surface_lists_inversion_operations() {
        let ids = package_surface()
            .operations
            .into_iter()
            .map(|operation| operation.id.0)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"inversion.trace".to_string()));
        assert!(ids.contains(&"inversion.confidence".to_string()));
        assert!(ids.contains(&"inversion.fidelity".to_string()));
    }

    #[test]
    fn trace_operation_builds_trace() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("inversion.trace"),
            input: serde_json::json!({"sourceType": "histogram", "targetType": "image", "fidelity": "heuristic", "notes": [{"field": "pixels", "method": "inferred", "message": "expanded"}]}),
        })
        .expect("trace");
        assert_eq!(response.value["fidelity"], "heuristic");
        assert_eq!(response.value["notes"][0]["method"], "inferred");
    }

    #[test]
    fn fidelity_rejects_unknown_value() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("inversion.fidelity"),
            input: serde_json::json!({"left": "exact", "right": "unknown"}),
        })
        .expect_err("unknown fidelity");
        assert!(error.contains("unsupported inversion fidelity"));
    }
}
