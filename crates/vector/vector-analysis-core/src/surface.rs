//! Library-owned runtime surface for `vector-analysis-core`.

use runtime_core::{
    describe_surface_response, parse_surface_input, structured_operation_response,
    surface_operation, validate_matching_lengths, validate_max_items, OperationId, PackageSurface,
    RuntimeCapabilities, SurfaceError, SurfaceRequest, SurfaceResponse,
};
use serde::Deserialize;

use crate::{
    cosine_similarity, dot, euclidean_distance, l2_norm, manhattan_distance, vector_stats,
    DenseVector, VectorMetric,
};

const DEFAULT_PREVIEW: usize = 16;
const MAX_PREVIEW: usize = 256;
const MAX_VALUES: usize = 100_000;

/// Describes the vector operations exposed by transport wrappers.
pub fn package_surface() -> PackageSurface {
    PackageSurface {
        library: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: RuntimeCapabilities::pure_rust(),
        operations: vec![
            surface_operation(
                "describe",
                "Describe package",
                "Dense vector validation and metrics for video-analysis.",
                serde_json::json!({"includeOperations": true}),
            ),
            operation(
                "vector.normalize",
                "Normalize vector",
                "Returns the L2 norm and normalized dense vector values.",
                serde_json::json!({"values": [3.0, 4.0]}),
            ),
            operation(
                "vector.distance",
                "Vector distance",
                "Computes cosine, euclidean, manhattan, or dot distance between two dense vectors.",
                serde_json::json!({"left": [1.0, 0.0], "right": [0.5, 0.5], "metric": "cosine"}),
            ),
            operation(
                "vector.summary",
                "Vector summary",
                "Returns per-dimension mean, min, max, and preview values for dense vectors.",
                serde_json::json!({"vectors": [[1.0, 2.0], [3.0, 4.0]], "previewValues": 2}),
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
    if id == "vector.normalize" {
        runtime_core::attach_landscape_contract(
            &mut operation,
            runtime_core::landscape::LandscapeOperationContract::new(
                runtime_core::landscape::LandscapeFunction::new(
                    "vector.normalize",
                    env!("CARGO_PKG_NAME"),
                )
                .input(runtime_core::landscape::LandscapePort::new(
                    "vector",
                    runtime_core::landscape::well_known::vector_vector(),
                ))
                .output(runtime_core::landscape::LandscapePort::new(
                    "vector",
                    runtime_core::landscape::well_known::vector_vector(),
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
        "vector.normalize" => normalize_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "vector.distance" => distance_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "vector.summary" => summary_value(
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
struct NormalizeRequest {
    values: Vec<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DistanceRequest {
    left: Vec<f32>,
    right: Vec<f32>,
    metric: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SummaryRequest {
    vectors: Vec<Vec<f32>>,
    #[serde(default = "default_preview")]
    preview_values: usize,
}

fn normalize_value(
    operation: &str,
    request: NormalizeRequest,
) -> Result<serde_json::Value, String> {
    validate_value_count(operation, "values", request.values.len())?;
    let vector = DenseVector::new(request.values)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let norm = l2_norm(vector.as_slice())
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let normalized = vector
        .l2_normalized()
        .map_err(|error| invalid_request(operation, error.to_string()))?
        .into_values();
    Ok(serde_json::json!({
        "dimensions": normalized.len(),
        "norm": norm,
        "values": normalized
    }))
}

fn distance_value(operation: &str, request: DistanceRequest) -> Result<serde_json::Value, String> {
    validate_value_count(operation, "left", request.left.len())?;
    validate_value_count(operation, "right", request.right.len())?;
    validate_matching_lengths(
        operation,
        "left",
        request.left.len(),
        "right",
        request.right.len(),
    )?;
    let metric = parse_metric(operation, &request.metric)?;
    let distance = match metric {
        VectorMetric::Cosine => {
            1.0 - cosine_similarity(&request.left, &request.right)
                .map_err(|error| invalid_request(operation, error.to_string()))?
        }
        VectorMetric::Euclidean => euclidean_distance(&request.left, &request.right)
            .map_err(|error| invalid_request(operation, error.to_string()))?,
        VectorMetric::Manhattan => manhattan_distance(&request.left, &request.right)
            .map_err(|error| invalid_request(operation, error.to_string()))?,
        VectorMetric::Dot => -dot(&request.left, &request.right)
            .map_err(|error| invalid_request(operation, error.to_string()))?,
    };
    let mut value = serde_json::json!({
        "metric": request.metric,
        "distance": distance
    });
    if metric == VectorMetric::Cosine {
        value["similarity"] =
            serde_json::json!(cosine_similarity(&request.left, &request.right)
                .map_err(|error| invalid_request(operation, error.to_string()))?);
    }
    if metric == VectorMetric::Dot {
        value["dot"] = serde_json::json!(dot(&request.left, &request.right)
            .map_err(|error| invalid_request(operation, error.to_string()))?);
    }
    Ok(value)
}

fn summary_value(operation: &str, request: SummaryRequest) -> Result<serde_json::Value, String> {
    let total_values = request.vectors.iter().map(Vec::len).sum::<usize>();
    validate_value_count(operation, "vectors", total_values)?;
    validate_max_items(
        operation,
        "previewValues",
        request.preview_values,
        MAX_PREVIEW,
    )?;
    let vectors = request
        .vectors
        .into_iter()
        .map(DenseVector::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let stats =
        vector_stats(&vectors).map_err(|error| invalid_request(operation, error.to_string()))?;
    let preview_values = request.preview_values;
    Ok(serde_json::json!({
        "count": vectors.len(),
        "dimensions": stats.dimensions,
        "mean": stats.mean,
        "min": stats.min,
        "max": stats.max,
        "meanPreview": stats.mean.iter().take(preview_values).copied().collect::<Vec<_>>()
    }))
}

fn parse_metric(operation: &str, metric: &str) -> Result<VectorMetric, String> {
    match metric {
        "cosine" => Ok(VectorMetric::Cosine),
        "euclidean" => Ok(VectorMetric::Euclidean),
        "manhattan" => Ok(VectorMetric::Manhattan),
        "dot" => Ok(VectorMetric::Dot),
        _ => Err(SurfaceError::unsupported_value(
            Some(OperationId::new(operation)),
            "metric",
            metric,
            &["cosine", "euclidean", "manhattan", "dot"],
        )
        .to_error_string()),
    }
}

fn validate_value_count(operation: &str, field: &str, count: usize) -> Result<(), String> {
    validate_max_items(operation, field, count, MAX_VALUES)
}

fn invalid_request(operation: &str, message: impl Into<String>) -> String {
    SurfaceError::invalid_request(Some(OperationId::new(operation)), message).to_error_string()
}

fn default_preview() -> usize {
    DEFAULT_PREVIEW
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_surface_lists_vector_operations() {
        let ids = package_surface()
            .operations
            .into_iter()
            .map(|operation| operation.id.0)
            .collect::<Vec<_>>();

        assert!(ids.contains(&"vector.normalize".to_string()));
        assert!(ids.contains(&"vector.distance".to_string()));
        assert!(ids.contains(&"vector.summary".to_string()));
    }

    #[test]
    fn normalize_returns_norm_and_values() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.normalize"),
            input: serde_json::json!({"values": [3.0, 4.0]}),
        })
        .expect("normalize operation");

        assert_eq!(response.value["dimensions"], 2);
        assert_eq!(response.value["norm"], 5.0);
        let values = response.value["values"].as_array().unwrap();
        assert!((values[0].as_f64().unwrap() - 0.6).abs() < 1.0e-6);
        assert!((values[1].as_f64().unwrap() - 0.8).abs() < 1.0e-6);
    }

    #[test]
    fn cosine_distance_returns_similarity() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.distance"),
            input: serde_json::json!({"left": [1.0, 0.0], "right": [1.0, 0.0], "metric": "cosine"}),
        })
        .expect("distance operation");

        assert_eq!(response.value["distance"], 0.0);
        assert_eq!(response.value["similarity"], 1.0);
    }

    #[test]
    fn summary_returns_vector_stats() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.summary"),
            input: serde_json::json!({"vectors": [[1.0, 3.0], [3.0, 5.0]], "previewValues": 1}),
        })
        .expect("summary operation");

        assert_eq!(response.value["count"], 2);
        assert_eq!(response.value["dimensions"], 2);
        assert_eq!(response.value["mean"], serde_json::json!([2.0, 4.0]));
        assert_eq!(response.value["min"], serde_json::json!([1.0, 3.0]));
        assert_eq!(response.value["max"], serde_json::json!([3.0, 5.0]));
        assert_eq!(response.value["meanPreview"], serde_json::json!([2.0]));
    }

    #[test]
    fn invalid_request_parsing_is_reported() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.normalize"),
            input: serde_json::json!(["not an object"]),
        })
        .expect_err("invalid request");

        assert!(error.contains("invalid request"));
    }

    #[test]
    fn mismatched_dimensions_return_validation_error() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.distance"),
            input: serde_json::json!({"left": [1.0], "right": [1.0, 2.0], "metric": "euclidean"}),
        })
        .expect_err("dimension mismatch");

        let parsed = runtime_core::parse_surface_error(&error).expect("typed surface error");
        assert_eq!(parsed.code, "invalid_request");
        assert!(parsed.message.contains("left"));
        assert!(parsed.message.contains("right"));
    }

    #[test]
    fn unsupported_operation_is_reported() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.missing"),
            input: serde_json::json!({}),
        })
        .expect_err("unsupported operation");

        assert!(error.contains("unsupported operation `vector.missing`"));
    }
}
