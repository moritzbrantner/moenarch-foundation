//! Library-owned runtime surface for `tensor-data`.

use runtime_core::{
    describe_surface_response, parse_surface_input, structured_operation_response,
    surface_operation, validate_max_items, OperationId, PackageSurface, RuntimeCapabilities,
    SurfaceError, SurfaceRequest, SurfaceResponse,
};
use serde::Deserialize;
use std::collections::BTreeMap;

use crate::{F32Tensor, TensorShape};

const MAX_ELEMENTS: usize = 100_000;
const MAX_PREVIEW_VALUES: usize = 256;

/// Describes the tensor operations exposed by transport wrappers.
pub fn package_surface() -> PackageSurface {
    PackageSurface {
        library: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: RuntimeCapabilities::pure_rust(),
        operations: vec![
            surface_operation(
                "describe",
                "Describe package",
                "Small finite f32 tensor contracts and metadata for video-analysis.",
                serde_json::json!({"includeOperations": true}),
            ),
            operation(
                "tensor.validate",
                "Validate tensor",
                "Validates a finite f32 tensor shape and values payload.",
                serde_json::json!({"shape": [2, 2], "values": [0.0, 1.0, 2.0, 3.0], "metadata": {"source": "surface"}}),
            ),
            operation(
                "tensor.summary",
                "Tensor summary",
                "Returns shape metadata and scalar summary statistics for a finite f32 tensor.",
                serde_json::json!({"shape": [2, 2], "values": [0.0, 1.0, 2.0, 3.0], "previewValues": 2}),
            ),
            surface_operation(
                "tensor.reshapePlan",
                "Tensor reshape plan",
                "Checks whether a tensor shape can be reshaped without changing element count.",
                serde_json::json!({"shape": [2, 2], "targetShape": [4, 1]}),
            ),
        ],
    }
}

fn operation(
    id: &str,
    name: &str,
    description: &str,
    example_request: serde_json::Value,
) -> runtime_core::SurfaceOperation {
    let mut operation = surface_operation(id, name, description, example_request);
    if id == "tensor.validate" {
        runtime_core::attach_landscape_contract(
            &mut operation,
            runtime_core::landscape::LandscapeOperationContract::new(
                runtime_core::landscape::LandscapeFunction::new(
                    "tensor.validate",
                    env!("CARGO_PKG_NAME"),
                )
                .input(runtime_core::landscape::LandscapePort::new(
                    "tensor",
                    runtime_core::landscape::well_known::tensor_f32_tensor(),
                ))
                .output(runtime_core::landscape::LandscapePort::new(
                    "tensor",
                    runtime_core::landscape::well_known::tensor_f32_tensor(),
                )),
            ),
        );
    }
    operation
}

/// Runs one library-owned operation.
pub fn run_surface_operation(request: SurfaceRequest) -> Result<SurfaceResponse, String> {
    let surface = package_surface();
    let operation = request.operation.clone();
    let value = match request.operation.as_str() {
        "describe" => return Ok(describe_surface_response(&surface, request)),
        "tensor.validate" => validate_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "tensor.summary" => summary_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "tensor.reshapePlan" => reshape_plan_value(
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
struct TensorRequest {
    shape: Vec<usize>,
    values: Vec<f32>,
    #[serde(default = "default_preview_values")]
    preview_values: usize,
    #[serde(default)]
    metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReshapePlanRequest {
    shape: Vec<usize>,
    target_shape: Vec<usize>,
    #[serde(default)]
    values: Option<Vec<f32>>,
}

fn validate_value(operation: &str, request: TensorRequest) -> Result<serde_json::Value, String> {
    let tensor = tensor_from_request(operation, &request)?;
    Ok(serde_json::json!({
        "valid": true,
        "dtype": "f32",
        "shape": tensor.shape().dimensions(),
        "rank": tensor.shape().rank(),
        "elementCount": tensor.shape().element_count().map_err(|error| invalid_request(operation, error.to_string()))?,
        "valueCount": tensor.values().len(),
        "metadataKeys": tensor.metadata().keys().collect::<Vec<_>>()
    }))
}

fn summary_value(operation: &str, request: TensorRequest) -> Result<serde_json::Value, String> {
    validate_max_items(
        operation,
        "previewValues",
        request.preview_values,
        MAX_PREVIEW_VALUES,
    )?;
    let tensor = tensor_from_request(operation, &request)?;
    let values = tensor.values();
    let min = values
        .iter()
        .copied()
        .reduce(f32::min)
        .ok_or_else(|| invalid_request(operation, "tensor values must not be empty"))?;
    let max = values
        .iter()
        .copied()
        .reduce(f32::max)
        .ok_or_else(|| invalid_request(operation, "tensor values must not be empty"))?;
    let sum = values.iter().map(|value| f64::from(*value)).sum::<f64>();
    Ok(serde_json::json!({
        "dtype": "f32",
        "shape": tensor.shape().dimensions(),
        "rank": tensor.shape().rank(),
        "elementCount": tensor.shape().element_count().map_err(|error| invalid_request(operation, error.to_string()))?,
        "min": min,
        "max": max,
        "sum": sum,
        "mean": sum / values.len() as f64,
        "preview": values.iter().take(request.preview_values).copied().collect::<Vec<_>>(),
        "metadataKeys": tensor.metadata().keys().collect::<Vec<_>>()
    }))
}

fn reshape_plan_value(
    operation: &str,
    request: ReshapePlanRequest,
) -> Result<serde_json::Value, String> {
    let shape = TensorShape::new(request.shape)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    validate_max_items(
        operation,
        "elementCount",
        shape
            .element_count()
            .map_err(|error| invalid_request(operation, error.to_string()))?,
        MAX_ELEMENTS,
    )?;
    if let Some(values) = request.values {
        validate_max_items(operation, "values", values.len(), MAX_ELEMENTS)?;
        F32Tensor::new(shape.clone(), values)
            .map_err(|error| invalid_request(operation, error.to_string()))?;
    }
    let target = shape
        .reshape(request.target_shape)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    Ok(serde_json::json!({
        "compatible": true,
        "fromShape": shape.dimensions(),
        "toShape": target.dimensions(),
        "rankChange": target.rank() as isize - shape.rank() as isize,
        "elementCount": shape.element_count().map_err(|error| invalid_request(operation, error.to_string()))?,
        "dataMovement": "metadata-only"
    }))
}

fn tensor_from_request(operation: &str, request: &TensorRequest) -> Result<F32Tensor, String> {
    validate_max_items(operation, "values", request.values.len(), MAX_ELEMENTS)?;
    let shape = TensorShape::new(request.shape.clone())
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    validate_max_items(
        operation,
        "elementCount",
        shape
            .element_count()
            .map_err(|error| invalid_request(operation, error.to_string()))?,
        MAX_ELEMENTS,
    )?;
    let mut tensor = F32Tensor::new(shape, request.values.clone())
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    for (key, value) in &request.metadata {
        tensor.set_metadata(key.clone(), value.clone());
    }
    Ok(tensor)
}

fn invalid_request(operation: &str, message: impl Into<String>) -> String {
    SurfaceError::invalid_request(Some(OperationId::new(operation)), message).to_error_string()
}

fn default_preview_values() -> usize {
    8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_surface_lists_tensor_operations() {
        let ids = package_surface()
            .operations
            .into_iter()
            .map(|operation| operation.id.0)
            .collect::<Vec<_>>();

        assert!(ids.contains(&"tensor.validate".to_string()));
        assert!(ids.contains(&"tensor.summary".to_string()));
        assert!(ids.contains(&"tensor.reshapePlan".to_string()));
    }

    #[test]
    fn describe_operation_returns_surface_summary() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("describe"),
            input: serde_json::json!({"includeOperations": true}),
        })
        .expect("describe operation");

        assert_eq!(response.operation.as_str(), "describe");
        assert_eq!(response.value["library"], env!("CARGO_PKG_NAME"));
    }

    #[test]
    fn validate_operation_accepts_matching_shape_and_values() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("tensor.validate"),
            input: serde_json::json!({"shape": [2, 2], "values": [0.0, 1.0, 2.0, 3.0]}),
        })
        .expect("validate operation");

        assert_eq!(response.value["valid"], true);
        assert_eq!(response.value["elementCount"], 4);
    }

    #[test]
    fn summary_operation_returns_scalar_stats() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("tensor.summary"),
            input: serde_json::json!({"shape": [2], "values": [1.0, 3.0], "previewValues": 1}),
        })
        .expect("summary operation");

        assert_eq!(response.value["mean"], 2.0);
        assert_eq!(response.value["preview"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn reshape_plan_operation_checks_element_count() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("tensor.reshapePlan"),
            input: serde_json::json!({"shape": [2, 2], "targetShape": [4, 1]}),
        })
        .expect("reshape plan operation");

        assert_eq!(response.value["compatible"], true);
        assert_eq!(response.value["elementCount"], 4);
    }

    #[test]
    fn invalid_request_parsing_is_reported() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("tensor.validate"),
            input: serde_json::json!({"shape": "not an array", "values": []}),
        })
        .expect_err("invalid request");

        assert!(error.contains("invalid request"));
    }

    #[test]
    fn unsupported_operation_is_reported() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("tensor.missing"),
            input: serde_json::json!({}),
        })
        .expect_err("unsupported operation");

        assert!(error.contains("unsupported operation `tensor.missing`"));
    }
}
