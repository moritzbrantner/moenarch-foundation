//! Library-owned runtime surface for `vector-analysis-index`.

use std::collections::BTreeMap;

use runtime_core::{
    OperationId, PackageSurface, RuntimeCapabilities, SurfaceOperation, SurfaceRequest,
    SurfaceResponse,
};
use serde::Deserialize;
use vector_analysis_core::{DenseVector, VectorMetric};

use crate::{
    assign_nearest_centroids, SearchConfig, VectorRecord, VectorRecordMetadata, VectorSearchIndex,
};

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
                "Exact in-memory vector search for video-analysis.",
                serde_json::json!({"includeOperations": true}),
            ),
            operation(
                "vector.index.search",
                "Search vector index",
                "Builds an in-memory index and returns nearest records for a dense query vector.",
                serde_json::json!({
                    "records": [{"id": "a", "vector": [1.0, 0.0]}],
                    "query": [1.0, 0.0],
                    "limit": 1,
                    "metric": "cosine"
                }),
            ),
            operation(
                "vector.index.centroids",
                "Assign centroids",
                "Assigns each dense vector to the nearest centroid using the selected metric.",
                serde_json::json!({
                    "vectors": [[0.0, 0.0], [10.0, 0.0]],
                    "centroids": [[0.0, 0.0], [9.0, 0.0]],
                    "metric": "euclidean"
                }),
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
        "vector.index.search" => search_value(parse_input(request.input)?)?,
        "vector.index.centroids" => centroids_value(parse_input(request.input)?)?,
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
        "operations": surface
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
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
struct SearchRequest {
    records: Vec<RecordRequest>,
    query: Vec<f32>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_metric")]
    metric: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordRequest {
    id: String,
    vector: Vec<f32>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CentroidsRequest {
    vectors: Vec<Vec<f32>>,
    centroids: Vec<Vec<f32>>,
    #[serde(default = "default_metric")]
    metric: String,
}

fn search_value(request: SearchRequest) -> Result<serde_json::Value, String> {
    if request.records.is_empty() {
        return Err("records must not be empty".to_string());
    }
    let record_value_count = request
        .records
        .iter()
        .map(|record| record.vector.len())
        .sum::<usize>();
    validate_value_count(record_value_count + request.query.len())?;
    let metric = parse_metric(&request.metric)?;
    let records = request
        .records
        .into_iter()
        .map(record_from_request)
        .collect::<Result<Vec<_>, _>>()?;
    let index = VectorSearchIndex::from_records(records).map_err(|error| error.to_string())?;
    let query = DenseVector::new(request.query).map_err(|error| error.to_string())?;
    let results = index
        .search(
            &query,
            SearchConfig {
                metric,
                limit: request.limit,
            },
        )
        .map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "recordCount": index.records().len(),
        "dimensions": index.dimensions().unwrap_or(0),
        "limit": request.limit,
        "metric": request.metric,
        "results": results.into_iter().map(|result| serde_json::json!({
            "id": result.id,
            "distance": result.distance,
            "score": result.score
        })).collect::<Vec<_>>()
    }))
}

fn centroids_value(request: CentroidsRequest) -> Result<serde_json::Value, String> {
    if request.vectors.is_empty() {
        return Err("vectors must not be empty".to_string());
    }
    if request.centroids.is_empty() {
        return Err("centroids must not be empty".to_string());
    }
    let value_count = request.vectors.iter().map(Vec::len).sum::<usize>()
        + request.centroids.iter().map(Vec::len).sum::<usize>();
    validate_value_count(value_count)?;
    let metric = parse_metric(&request.metric)?;
    let vectors = dense_vectors(request.vectors)?;
    let centroids = dense_vectors(request.centroids)?;
    let assignments = assign_nearest_centroids(&vectors, &centroids, metric)
        .map_err(|error| error.to_string())?;
    let mut counts = vec![0_usize; centroids.len()];
    for assignment in &assignments {
        counts[*assignment] += 1;
    }
    Ok(serde_json::json!({
        "vectorCount": vectors.len(),
        "centroidCount": centroids.len(),
        "assignments": assignments,
        "counts": counts
    }))
}

fn record_from_request(request: RecordRequest) -> Result<VectorRecord, String> {
    Ok(VectorRecord::with_payload(
        request.id,
        DenseVector::new(request.vector).map_err(|error| error.to_string())?,
        VectorRecordMetadata {
            tags: request.tags,
            metadata: request.metadata,
        },
    ))
}

fn dense_vectors(values: Vec<Vec<f32>>) -> Result<Vec<DenseVector>, String> {
    values
        .into_iter()
        .map(DenseVector::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn parse_metric(metric: &str) -> Result<VectorMetric, String> {
    match metric {
        "cosine" => Ok(VectorMetric::Cosine),
        "euclidean" => Ok(VectorMetric::Euclidean),
        "manhattan" => Ok(VectorMetric::Manhattan),
        "dot" => Ok(VectorMetric::Dot),
        _ => Err(format!("unsupported vector metric `{metric}`")),
    }
}

fn validate_value_count(count: usize) -> Result<(), String> {
    if count > MAX_VALUES {
        return Err(format!("vector values must not exceed {MAX_VALUES}"));
    }
    Ok(())
}

fn parse_input<T: for<'de> Deserialize<'de>>(input: serde_json::Value) -> Result<T, String> {
    serde_json::from_value(input).map_err(|error| format!("invalid request: {error}"))
}

fn default_limit() -> usize {
    SearchConfig::default().limit
}

fn default_metric() -> String {
    "cosine".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_surface_lists_index_operations() {
        let ids = package_surface()
            .operations
            .into_iter()
            .map(|operation| operation.id.0)
            .collect::<Vec<_>>();

        assert!(ids.contains(&"vector.index.search".to_string()));
        assert!(ids.contains(&"vector.index.centroids".to_string()));
    }

    #[test]
    fn search_ranks_nearest_vector_first() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.index.search"),
            input: serde_json::json!({
                "records": [
                    {"id": "x", "vector": [1.0, 0.0]},
                    {"id": "y", "vector": [0.0, 1.0]}
                ],
                "query": [0.9, 0.1],
                "limit": 2,
                "metric": "cosine"
            }),
        })
        .expect("search operation");

        assert_eq!(response.value["recordCount"], 2);
        assert_eq!(response.value["results"][0]["id"], "x");
    }

    #[test]
    fn centroid_assignment_returns_assignments_and_counts() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.index.centroids"),
            input: serde_json::json!({
                "vectors": [[0.0, 0.0], [9.0, 0.0], [10.0, 0.0]],
                "centroids": [[0.0, 0.0], [10.0, 0.0]],
                "metric": "euclidean"
            }),
        })
        .expect("centroids operation");

        assert_eq!(response.value["assignments"], serde_json::json!([0, 1, 1]));
        assert_eq!(response.value["counts"], serde_json::json!([1, 2]));
    }

    #[test]
    fn empty_records_fail() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.index.search"),
            input: serde_json::json!({"records": [], "query": [1.0], "limit": 1}),
        })
        .expect_err("empty records");

        assert!(error.contains("records must not be empty"));
    }

    #[test]
    fn search_limit_zero_fails() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.index.search"),
            input: serde_json::json!({
                "records": [{"id": "x", "vector": [1.0]}],
                "query": [1.0],
                "limit": 0
            }),
        })
        .expect_err("zero limit");

        assert!(error.contains("search limit must be greater than zero"));
    }

    #[test]
    fn mismatched_dimensions_fail() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.index.search"),
            input: serde_json::json!({
                "records": [{"id": "x", "vector": [1.0, 0.0]}],
                "query": [1.0],
                "limit": 1
            }),
        })
        .expect_err("dimension mismatch");

        assert!(error.contains("query dimensions must match the index"));
    }

    #[test]
    fn invalid_request_parsing_is_reported() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.index.search"),
            input: serde_json::json!(null),
        })
        .expect_err("invalid request");

        assert!(error.contains("invalid request"));
    }

    #[test]
    fn unsupported_operation_is_reported() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("vector.index.missing"),
            input: serde_json::json!({}),
        })
        .expect_err("unsupported operation");

        assert!(error.contains("unsupported operation `vector.index.missing`"));
    }
}
