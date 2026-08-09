//! Library-owned runtime surface for `jobs-core`.

use runtime_core::{
    describe_surface_response, parse_surface_input, set_surface_operation_curation,
    structured_operation_response, surface_operation, surface_operation_with_execution_plan,
    validate_max_items, OperationId, PackageSurface, RuntimeCapabilities,
    SurfaceArtifactExpectation, SurfaceError, SurfaceExecutionMode, SurfaceExecutionPlan,
    SurfaceOperation, SurfaceOperationCuration, SurfaceRequest, SurfaceResponse, SurfaceSideEffect,
};
use serde::Deserialize;

use std::collections::BTreeMap;

use crate::{
    ArtifactRef, JobArtifact, JobEvent, JobEventFilter, JobEventKind, JobId, JobListFilter,
    JobLogLevel, JobManifest, JobProgress, JobSnapshot, JobSpec, JobStatus, JobTracker,
};

const MAX_LIFECYCLE_STEPS: usize = 32;

/// Returns the package surface exposed by every transport wrapper.
pub fn package_surface() -> PackageSurface {
    PackageSurface {
        library: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: RuntimeCapabilities::pure_rust(),
        operations: vec![
            curated(
                surface_operation(
                "describe",
                "Describe package",
                "Reusable long-running job state, cancellation, progress, logs, and artifact primitives.",
                serde_json::json!({"includeOperations": true}),
            ),
                SurfaceOperationCuration::debug(900),
            ),
            curated(
                surface_operation_with_execution_plan(
                "jobs.spec",
                "Validate job spec",
                "Validates a JobSpec input and returns normalized id, name, kind, and metadata.",
                serde_json::json!({"id": "job-1", "name": "Demo job", "kind": "demo", "metadata": {"owner": "surface"}}),
                in_memory_plan("jobs.spec", None, Vec::new()),
            ),
                SurfaceOperationCuration::debug(910),
            ),
            curated(
                surface_operation_with_execution_plan(
                "jobs.progress",
                "Validate job progress",
                "Validates JobProgress input and returns fraction and percent when a total is known.",
                serde_json::json!({"completed": 2, "total": 4, "unit": "steps", "message": "halfway"}),
                in_memory_plan("jobs.progress", Some("steps"), Vec::new()),
            ),
                SurfaceOperationCuration::debug(920),
            ),
            curated(
                surface_operation_with_execution_plan(
                "jobs.lifecycle",
                "Script job lifecycle",
                "Creates an in-memory tracker, applies a short lifecycle script, and returns the final snapshot and events.",
                serde_json::json!({"spec": {"id": "job-1", "name": "Demo job"}, "script": ["running", "progress", "log", "artifact", "succeeded"]}),
                in_memory_plan(
                    "jobs.lifecycle",
                    Some("steps"),
                    vec![SurfaceArtifactExpectation {
                        id: "artifact-1".to_string(),
                        kind: "json".to_string(),
                        media_type: "application/json".to_string(),
                        required: false,
                        description: Some("Inline lifecycle demo artifact".to_string()),
                    }],
                ),
            ),
                SurfaceOperationCuration::workflow(10).primary(),
            ),
            curated(
                surface_operation_with_execution_plan(
                "jobs.manifest",
                "Build job manifest",
                "Builds a deterministic JobManifest from inline spec, progress, artifact, and metadata inputs.",
                serde_json::json!({"operationId": "demo.operation", "spec": {"id": "job-1", "name": "Demo job", "kind": "demo"}, "status": "succeeded", "progress": {"completed": 1, "total": 1, "unit": "steps"}, "artifacts": [{"id": "report", "kind": "json", "mediaType": "application/json", "uri": "memory://job-1/report.json"}], "metadata": {"source": "surface"}}),
                in_memory_plan(
                    "jobs.manifest",
                    Some("steps"),
                    vec![SurfaceArtifactExpectation {
                        id: "report".to_string(),
                        kind: "json".to_string(),
                        media_type: "application/json".to_string(),
                        required: false,
                        description: Some("Manifest artifact projection".to_string()),
                    }],
                ),
            ),
                SurfaceOperationCuration::workflow(20),
            ),
            curated(
                surface_operation_with_execution_plan(
                "jobs.events",
                "Replay job events",
                "Replays and filters a short inline lifecycle script into ordered job events.",
                serde_json::json!({"spec": {"id": "job-1", "name": "Demo job"}, "script": ["running", "progress", "log", "artifact", "succeeded"], "filter": {"afterSequence": 2, "kind": "progress"}}),
                in_memory_plan("jobs.events", Some("steps"), Vec::new()),
            ),
                SurfaceOperationCuration::debug(930),
            ),
            curated(
                surface_operation_with_execution_plan(
                "jobs.artifactValidate",
                "Validate artifact metadata",
                "Validates inline artifact media type, size, checksum, and metadata expectations without reading files.",
                serde_json::json!({"artifact": {"id": "report", "kind": "json", "mediaType": "application/json", "uri": "memory://job-1/report.json", "sizeBytes": 2, "sha256": "abcd"}, "expectedMediaType": "application/json", "expectedSha256": "abcd", "requiredMetadata": {}}),
                in_memory_plan("jobs.artifactValidate", None, Vec::new()),
            ),
                SurfaceOperationCuration::debug(940),
            ),
        ],
    }
}

fn curated(
    mut operation: SurfaceOperation,
    curation: SurfaceOperationCuration,
) -> SurfaceOperation {
    set_surface_operation_curation(&mut operation, curation);
    operation
}

fn in_memory_plan(
    operation: &str,
    progress_unit: Option<&str>,
    expected_artifacts: Vec<SurfaceArtifactExpectation>,
) -> SurfaceExecutionPlan {
    SurfaceExecutionPlan {
        operation: OperationId::new(operation),
        mode: SurfaceExecutionMode::InMemory,
        side_effects: vec![SurfaceSideEffect::None],
        cancellable: false,
        progress_unit: progress_unit.map(str::to_string),
        expected_artifacts,
        requirements: Vec::new(),
        max_recommended_input_bytes: Some(1_048_576),
    }
}

/// Runs one library-owned operation.
pub fn run_surface_operation(request: SurfaceRequest) -> Result<SurfaceResponse, String> {
    let surface = package_surface();
    let operation = request.operation.clone();
    let value = match request.operation.as_str() {
        "describe" => return Ok(describe_surface_response(&surface, request)),
        "jobs.spec" => spec_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "jobs.progress" => progress_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "jobs.lifecycle" => lifecycle_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "jobs.manifest" => manifest_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "jobs.events" => events_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "jobs.artifactValidate" => artifact_validate_value(
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
struct LifecycleRequest {
    spec: JobSpecInput,
    #[serde(default = "default_lifecycle_script")]
    script: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestRequest {
    #[serde(default = "default_operation_id")]
    operation_id: String,
    spec: JobSpecInput,
    #[serde(default = "default_status")]
    status: JobStatus,
    progress: Option<JobProgress>,
    #[serde(default)]
    artifacts: Vec<ArtifactRef>,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventsRequest {
    spec: JobSpecInput,
    #[serde(default = "default_lifecycle_script")]
    script: Vec<String>,
    #[serde(default)]
    filter: JobEventFilter,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactValidateRequest {
    artifact: ArtifactRef,
    expected_media_type: Option<String>,
    expected_sha256: Option<String>,
    expected_size_bytes: Option<u64>,
    #[serde(default)]
    required_metadata: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobSpecInput {
    id: String,
    name: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
}

fn spec_value(spec: JobSpecInput) -> Result<serde_json::Value, String> {
    let normalized = build_spec("jobs.spec", spec)?;
    Ok(serde_json::json!({
        "id": normalized.id.as_str(),
        "name": normalized.name,
        "kind": normalized.kind,
        "metadata": normalized.metadata
    }))
}

fn progress_value(progress: JobProgress) -> Result<serde_json::Value, String> {
    progress
        .validate()
        .map_err(|error| invalid_request("jobs.progress", error.to_string()))?;
    Ok(serde_json::json!({
        "completed": progress.completed,
        "total": progress.total,
        "unit": progress.unit,
        "message": progress.message,
        "fraction": progress.fraction(),
        "percent": progress.percent()
    }))
}

fn lifecycle_value(
    operation: &str,
    request: LifecycleRequest,
) -> Result<serde_json::Value, String> {
    let (tracker, job_id) = replay_lifecycle(operation, request.spec, request.script)?;
    let snapshot = tracker
        .snapshot(&job_id)
        .map_err(|error| invalid_request(operation, error.to_string()))?
        .ok_or_else(|| invalid_request(operation, "lifecycle job snapshot missing"))?;
    let events = tracker
        .events(&job_id)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let event_count = events.len();
    let artifact_event_count = events
        .iter()
        .filter(|event| matches!(event.kind, JobEventKind::Artifact(_)))
        .count();
    let log_event_count = events
        .iter()
        .filter(|event| matches!(event.kind, JobEventKind::Log(_)))
        .count();
    Ok(serde_json::json!({
        "snapshot": snapshot,
        "events": events,
        "eventCount": event_count,
        "artifactCount": artifact_event_count,
        "logCount": log_event_count,
        "terminal": snapshot.status.is_terminal(),
        "status": status_name(snapshot.status)
    }))
}

fn replay_lifecycle(
    operation: &str,
    spec: JobSpecInput,
    script: Vec<String>,
) -> Result<(JobTracker, JobId), String> {
    validate_max_items(operation, "script", script.len(), MAX_LIFECYCLE_STEPS)?;
    let tracker = JobTracker::new();
    let handle = tracker
        .create(build_spec(operation, spec)?)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let job_id = handle.id().clone();
    let context = handle.context();
    for step in script {
        match step.as_str() {
            "create" | "queued" => {}
            "running" => handle
                .mark_running()
                .map_err(|error| invalid_request(operation, error.to_string()))?,
            "progress" => context
                .progress(
                    JobProgress::new(1, Some(2))
                        .map_err(|error| invalid_request(operation, error.to_string()))?,
                )
                .map_err(|error| invalid_request(operation, error.to_string()))?,
            "log" => context
                .log(JobLogLevel::Info, "surface lifecycle log")
                .map_err(|error| invalid_request(operation, error.to_string()))?,
            "artifact" => context
                .artifact(
                    JobArtifact::new("artifact-1", "surface artifact")
                        .kind("json")
                        .uri("memory://artifact-1")
                        .content_type("application/json")
                        .bytes(2),
                )
                .map_err(|error| invalid_request(operation, error.to_string()))?,
            "succeeded" => handle
                .mark_succeeded()
                .map_err(|error| invalid_request(operation, error.to_string()))?,
            "failed" => handle
                .mark_failed("surface lifecycle failure")
                .map_err(|error| invalid_request(operation, error.to_string()))?,
            "cancelled" => handle
                .mark_cancelled(Some("surface lifecycle cancelled".to_string()))
                .map_err(|error| invalid_request(operation, error.to_string()))?,
            other => {
                return Err(SurfaceError::unsupported_value(
                    Some(OperationId::new(operation)),
                    "script",
                    other,
                    &[
                        "create",
                        "queued",
                        "running",
                        "progress",
                        "log",
                        "artifact",
                        "succeeded",
                        "failed",
                        "cancelled",
                    ],
                )
                .to_error_string())
            }
        }
    }
    Ok((tracker, job_id))
}

fn manifest_value(operation: &str, request: ManifestRequest) -> Result<serde_json::Value, String> {
    if let Some(progress) = &request.progress {
        progress
            .validate()
            .map_err(|error| invalid_request(operation, error.to_string()))?;
    }
    let spec = build_spec(operation, request.spec)?;
    let timestamp = chrono::DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z")
        .expect("valid timestamp")
        .with_timezone(&chrono::Utc);
    let snapshot = JobSnapshot {
        metadata: spec.metadata.clone(),
        spec,
        status: request.status,
        progress: request.progress,
        logs: Vec::new(),
        artifacts: request
            .artifacts
            .iter()
            .map(JobArtifact::from_artifact_ref)
            .collect(),
        created_at: timestamp,
        started_at: None,
        finished_at: request.status.is_terminal().then_some(timestamp),
        failure: None,
    };
    let manifest = JobManifest::from_snapshot(
        OperationId::new(request.operation_id),
        snapshot,
        request.artifacts,
        request.metadata,
    );
    Ok(serde_json::json!({
        "manifest": manifest,
        "artifactCount": manifest.artifacts.len(),
        "status": status_name(manifest.status),
        "terminal": manifest.status.is_terminal()
    }))
}

fn events_value(operation: &str, request: EventsRequest) -> Result<serde_json::Value, String> {
    let filter = request.filter;
    let (tracker, job_id) = replay_lifecycle(operation, request.spec, request.script)?;
    let snapshots = tracker
        .query_snapshots(JobListFilter::default())
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let events = tracker
        .events_filtered(&job_id, filter)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let compact_events = events.iter().map(compact_event).collect::<Vec<_>>();
    Ok(serde_json::json!({
        "jobId": job_id.as_str(),
        "events": compact_events,
        "eventCount": compact_events.len(),
        "snapshotCount": snapshots.len(),
        "sequences": events.iter().map(|event| event.sequence).collect::<Vec<_>>()
    }))
}

fn artifact_validate_value(
    _operation: &str,
    request: ArtifactValidateRequest,
) -> Result<serde_json::Value, String> {
    let mut diagnostics = Vec::new();
    if let Some(expected) = &request.expected_media_type {
        if request.artifact.media_type != *expected {
            diagnostics.push(serde_json::json!({
                "code": "media_type_mismatch",
                "message": format!("expected media type `{expected}`, got `{}`", request.artifact.media_type)
            }));
        }
    }
    if let Some(expected) = &request.expected_sha256 {
        if request.artifact.sha256.as_deref() != Some(expected.as_str()) {
            diagnostics.push(serde_json::json!({
                "code": "sha256_mismatch",
                "message": "artifact sha256 did not match expectation"
            }));
        }
    }
    if let Some(expected) = request.expected_size_bytes {
        if request.artifact.size_bytes != Some(expected) {
            diagnostics.push(serde_json::json!({
                "code": "size_mismatch",
                "message": format!("expected {expected} bytes, got {:?}", request.artifact.size_bytes)
            }));
        }
    }
    for (key, expected) in &request.required_metadata {
        if request.artifact.metadata.get(key) != Some(expected) {
            diagnostics.push(serde_json::json!({
                "code": "metadata_mismatch",
                "message": format!("metadata `{key}` did not match expectation")
            }));
        }
    }
    Ok(serde_json::json!({
        "valid": diagnostics.is_empty(),
        "artifact": request.artifact,
        "diagnostics": diagnostics,
        "checked": {
            "mediaType": request.expected_media_type.is_some(),
            "sha256": request.expected_sha256.is_some(),
            "sizeBytes": request.expected_size_bytes.is_some(),
            "metadataKeys": request.required_metadata.keys().collect::<Vec<_>>()
        }
    }))
}

fn compact_event(event: &JobEvent) -> serde_json::Value {
    match &event.kind {
        JobEventKind::StatusChanged { status, message } => serde_json::json!({
            "sequence": event.sequence,
            "kind": "status",
            "status": status_name(*status),
            "message": message
        }),
        JobEventKind::Progress(progress) => serde_json::json!({
            "sequence": event.sequence,
            "kind": "progress",
            "completed": progress.completed,
            "total": progress.total,
            "unit": progress.unit
        }),
        JobEventKind::Log(entry) => serde_json::json!({
            "sequence": event.sequence,
            "kind": "log",
            "level": entry.level,
            "message": entry.message
        }),
        JobEventKind::Artifact(artifact) => serde_json::json!({
            "sequence": event.sequence,
            "kind": "artifact",
            "id": artifact.id,
            "mediaType": artifact.content_type
        }),
        JobEventKind::Metadata { key, value } => serde_json::json!({
            "sequence": event.sequence,
            "kind": "metadata",
            "key": key,
            "value": value
        }),
    }
}

fn build_spec(operation: &str, spec: JobSpecInput) -> Result<JobSpec, String> {
    let normalized = JobSpec::new(spec.id, spec.name)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let normalized = if let Some(kind) = spec.kind {
        normalized
            .with_kind(kind)
            .map_err(|error| invalid_request(operation, error.to_string()))?
    } else {
        normalized
    };
    spec.metadata
        .into_iter()
        .try_fold(normalized, |spec, (key, value)| {
            spec.with_metadata(key, value)
                .map_err(|error| invalid_request(operation, error.to_string()))
        })
}

fn status_name(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Cancelling => "cancelling",
        JobStatus::Succeeded => "succeeded",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
    }
}

fn invalid_request(operation: &str, message: impl Into<String>) -> String {
    SurfaceError::invalid_request(Some(OperationId::new(operation)), message).to_error_string()
}

fn default_lifecycle_script() -> Vec<String> {
    vec![
        "running".to_string(),
        "progress".to_string(),
        "log".to_string(),
        "artifact".to_string(),
        "succeeded".to_string(),
    ]
}

fn default_operation_id() -> String {
    "jobs.lifecycle".to_string()
}

fn default_status() -> JobStatus {
    JobStatus::Queued
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_surface_lists_job_operations() {
        let ids = package_surface()
            .operations
            .into_iter()
            .map(|operation| operation.id.0)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"jobs.spec".to_string()));
        assert!(ids.contains(&"jobs.progress".to_string()));
        assert!(ids.contains(&"jobs.lifecycle".to_string()));
        assert!(ids.contains(&"jobs.manifest".to_string()));
        assert!(ids.contains(&"jobs.events".to_string()));
        assert!(ids.contains(&"jobs.artifactValidate".to_string()));
    }

    #[test]
    fn progress_operation_returns_percent() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("jobs.progress"),
            input: serde_json::json!({"completed": 2, "total": 4, "unit": "steps"}),
        })
        .expect("progress");
        assert_eq!(response.value["percent"], 50.0);
    }

    #[test]
    fn lifecycle_operation_returns_succeeded_snapshot() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("jobs.lifecycle"),
            input: serde_json::json!({"spec": {"id": "job-1", "name": "Demo"}}),
        })
        .expect("lifecycle");
        assert_eq!(response.value["status"], "succeeded");
        assert!(response.value["events"].as_array().unwrap().len() >= 5);
    }

    #[test]
    fn lifecycle_rejects_unknown_step() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("jobs.lifecycle"),
            input: serde_json::json!({"spec": {"id": "job-1", "name": "Demo"}, "script": ["wat"]}),
        })
        .expect_err("bad step");
        let parsed = runtime_core::parse_surface_error(&error).expect("typed surface error");
        assert_eq!(parsed.code, "unsupported_value");
        assert_eq!(parsed.details["field"], "script");
    }

    #[test]
    fn manifest_operation_builds_deterministic_manifest() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("jobs.manifest"),
            input: serde_json::json!({
                "operationId": "demo.operation",
                "spec": {"id": "job-1", "name": "Demo", "kind": "demo"},
                "status": "succeeded",
                "progress": {"completed": 1, "total": 1, "unit": "steps"},
                "artifacts": [{
                    "id": "report",
                    "kind": "json",
                    "mediaType": "application/json",
                    "uri": "memory://job-1/report.json"
                }],
                "metadata": {"source": "test"}
            }),
        })
        .expect("manifest");

        assert_eq!(response.value["status"], "succeeded");
        assert_eq!(response.value["artifactCount"], 1);
        assert_eq!(
            response.value["manifest"]["createdAt"],
            "1970-01-01T00:00:00+00:00"
        );
    }

    #[test]
    fn events_operation_filters_compact_events() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("jobs.events"),
            input: serde_json::json!({
                "spec": {"id": "job-1", "name": "Demo"},
                "script": ["running", "progress", "log", "succeeded"],
                "filter": {"kind": "progress"}
            }),
        })
        .expect("events");

        assert_eq!(response.value["eventCount"], 1);
        assert_eq!(response.value["events"][0]["kind"], "progress");
    }

    #[test]
    fn artifact_validate_checks_inline_expectations() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("jobs.artifactValidate"),
            input: serde_json::json!({
                "artifact": {
                    "id": "report",
                    "kind": "json",
                    "mediaType": "application/json",
                    "uri": "memory://job-1/report.json",
                    "sizeBytes": 2,
                    "sha256": "abcd",
                    "metadata": {"role": "summary"}
                },
                "expectedMediaType": "application/json",
                "expectedSha256": "abcd",
                "expectedSizeBytes": 2,
                "requiredMetadata": {"role": "summary"}
            }),
        })
        .expect("artifact validate");

        assert_eq!(response.value["valid"], true);
        assert_eq!(response.value["diagnostics"].as_array().unwrap().len(), 0);
    }
}
