//! Library-owned runtime surface for `model-runtime`.

use runtime_core::{
    describe_surface_response, parse_surface_input, set_surface_operation_curation,
    structured_operation_response, surface_operation, surface_operation_with_execution_plan,
    OperationId, PackageSurface, RuntimeCapabilities, SurfaceError, SurfaceExecutionMode,
    SurfaceExecutionPlan, SurfaceOperation, SurfaceOperationCuration, SurfaceRequest,
    SurfaceResponse, SurfaceSideEffect,
};
use serde::Deserialize;

use crate::{
    plan_model_access, plan_model_bundle, ModelAccessJobRequest, ModelBundlePlan, ModelFileRequest,
    ModelPreset, ModelSource, ModelSpec,
};

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
                "Generic model specs, bundle plans, presets, and job helpers for multimodal runtimes.",
                serde_json::json!({"includeOperations": true}),
            ),
                SurfaceOperationCuration::debug(900),
            ),
            curated(
                surface_operation_with_execution_plan(
                "model.executionPlan",
                "Model execution plan",
                "Validates a model access job request and returns a pure execution plan without spawning jobs or running inference.",
                serde_json::json!({"id": "model-job-1", "kind": "Inference", "spec": {"name": "demo-model", "task": "text_embedding", "source": {"kind": "hugging_face", "repo_id": "demo/model", "revision": "main"}, "files": [{"required": "config.json"}]}, "backend": "heuristic", "inputs": [{"kind": "json", "value": {"text": "hello"}}], "outputArtifactPrefix": "prediction"}),
                surface_plan("model.executionPlan"),
            ),
                SurfaceOperationCuration::workflow(10).primary(),
            ),
            curated(
                surface_operation_with_execution_plan(
                "model.bundlePlan",
                "Model bundle plan",
                "Plans manifest layout and artifact references for a model spec and optional local file list without network or materialization.",
                serde_json::json!({"spec": {"name": "demo-model", "task": "text_embedding", "source": {"kind": "hugging_face", "repo_id": "demo/model", "revision": "main"}, "files": [{"required": "config.json"}]}, "localFiles": ["config.json"]}),
                surface_plan("model.bundlePlan"),
            ),
                SurfaceOperationCuration::workflow(20),
            ),
            curated(
                surface_operation_with_execution_plan(
                "model.jobManifest",
                "Model job manifest",
                "Projects a planned model access job into a deterministic JobManifest without starting a job.",
                serde_json::json!({"id": "model-job-1", "kind": "Inference", "spec": {"name": "demo-model", "task": "text_embedding", "source": {"kind": "hugging_face", "repo_id": "demo/model", "revision": "main"}, "files": [{"required": "config.json"}]}, "backend": "heuristic", "outputArtifactPrefix": "prediction"}),
                surface_plan("model.jobManifest"),
            ),
                SurfaceOperationCuration::workflow(30),
            ),
            curated(
                surface_operation(
                "model.presets",
                "Model presets",
                "Lists model preset ids and derived model spec summaries without downloading models.",
                serde_json::json!({}),
            ),
                SurfaceOperationCuration::debug(910),
            ),
            curated(
                surface_operation(
                "model.spec",
                "Model spec",
                "Validates a ModelSpec-shaped input and returns safe name, task, source, files, and revision.",
                serde_json::json!({"name": "demo-model", "task": "text_embedding", "source": {"kind": "hugging_face", "repo_id": "demo/model", "revision": "main"}, "files": [{"required": "config.json"}]}),
            ),
                SurfaceOperationCuration::debug(920),
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

fn surface_plan(operation: &str) -> SurfaceExecutionPlan {
    SurfaceExecutionPlan {
        operation: OperationId::new(operation),
        mode: SurfaceExecutionMode::PlannedJob,
        side_effects: vec![SurfaceSideEffect::None],
        cancellable: false,
        progress_unit: Some("steps".to_string()),
        expected_artifacts: Vec::new(),
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
        "model.executionPlan" => execution_plan_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "model.bundlePlan" => bundle_plan_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "model.jobManifest" => job_manifest_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "model.presets" => presets_value(),
        "model.spec" => spec_value(
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
struct BundlePlanRequest {
    spec: ModelSpec,
    #[serde(default)]
    local_files: Vec<String>,
}

fn execution_plan_value(
    operation: &str,
    request: ModelAccessJobRequest,
) -> Result<serde_json::Value, String> {
    let plan = plan_model_access(&request)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    Ok(serde_json::json!({
        "plan": plan,
        "jobSpec": plan.job_spec,
        "executionPlan": plan.execution_plan,
        "expectedArtifacts": plan.expected_artifacts,
        "kind": plan.kind.as_str(),
        "backend": plan.backend.as_str(),
        "sideEffects": plan.execution_plan.side_effects
    }))
}

fn bundle_plan_value(
    operation: &str,
    request: BundlePlanRequest,
) -> Result<serde_json::Value, String> {
    let plan = plan_model_bundle(&request.spec, &request.local_files)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    Ok(bundle_plan_json(plan))
}

fn job_manifest_value(
    operation: &str,
    request: ModelAccessJobRequest,
) -> Result<serde_json::Value, String> {
    let plan = plan_model_access(&request)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let timestamp = chrono_like_epoch();
    let snapshot = jobs_core::JobSnapshot {
        metadata: plan.job_spec.metadata.clone(),
        spec: plan.job_spec.clone(),
        status: jobs_core::JobStatus::Queued,
        progress: None,
        logs: Vec::new(),
        artifacts: plan
            .expected_artifacts
            .iter()
            .map(jobs_core::JobArtifact::from_artifact_ref)
            .collect(),
        created_at: timestamp,
        started_at: None,
        finished_at: None,
        failure: None,
    };
    let manifest = jobs_core::JobManifest::from_snapshot(
        OperationId::new("model.executionPlan"),
        snapshot,
        plan.expected_artifacts.clone(),
        serde_json::json!({
            "kind": plan.kind.as_str(),
            "backend": plan.backend.as_str(),
            "model": plan.job_spec.metadata.get("model.name")
        }),
    );
    Ok(serde_json::json!({
        "manifest": manifest,
        "artifactCount": manifest.artifacts.len(),
        "jobKind": plan.kind.as_str(),
        "status": "queued"
    }))
}

fn presets_value() -> serde_json::Value {
    serde_json::json!({
        "presets": ModelPreset::ALL.iter().map(|preset| {
            let spec = preset.spec();
            serde_json::json!({
                "id": preset.as_str(),
                "name": spec.name,
                "safeName": spec.safe_name(),
                "task": spec.task.as_protocol_str(),
                "repoId": spec.repo_id_value(),
                "source": source_json(&spec.source),
                "revision": spec.revision_value(),
                "requestedFiles": file_requests_json(&spec.files),
                "metadata": spec.metadata
            })
        }).collect::<Vec<_>>()
    })
}

fn spec_value(operation: &str, spec: ModelSpec) -> Result<serde_json::Value, String> {
    validate_spec(operation, &spec)?;
    Ok(spec_summary_json(&spec))
}

fn bundle_plan_json(plan: ModelBundlePlan) -> serde_json::Value {
    let files = plan.files.clone();
    serde_json::json!({
        "spec": spec_summary_json(&plan.spec),
        "manifestPath": plan.manifest_path,
        "filesDirectory": plan.files_directory,
        "files": files,
        "manifest": {
            "schemaVersion": 1,
            "name": plan.spec.name,
            "repoId": plan.spec.repo_id_value(),
            "revision": plan.spec.revision_value(),
            "task": plan.spec.task.as_protocol_str(),
            "files": plan.files
        },
        "artifactRefs": plan.artifact_refs,
        "downloadsRequired": plan.downloads_required
    })
}

fn spec_summary_json(spec: &ModelSpec) -> serde_json::Value {
    serde_json::json!({
        "name": spec.name,
        "safeName": spec.safe_name(),
        "task": spec.task.as_protocol_str(),
        "source": source_json(&spec.source),
        "requestedFiles": file_requests_json(&spec.files),
        "revision": spec.revision_value(),
        "repoId": spec.repo_id_value(),
        "metadata": spec.metadata
    })
}

fn validate_spec(operation: &str, spec: &ModelSpec) -> Result<(), String> {
    if spec.name.trim().is_empty() {
        return Err(invalid_request(operation, "model name must not be empty"));
    }
    Ok(())
}

fn source_json(source: &ModelSource) -> serde_json::Value {
    match source {
        ModelSource::HuggingFace { repo_id, revision } => serde_json::json!({
            "kind": source.kind(),
            "repoId": repo_id,
            "revision": revision
        }),
        ModelSource::LocalPath { path } => serde_json::json!({
            "kind": source.kind(),
            "path": path
        }),
        ModelSource::ExternalCommand { command } => serde_json::json!({
            "kind": source.kind(),
            "command": command
        }),
        ModelSource::ComfyUiInventory { root } => serde_json::json!({
            "kind": source.kind(),
            "root": root
        }),
        ModelSource::Custom(kind) => serde_json::json!({
            "kind": kind
        }),
    }
}

fn file_requests_json(files: &[ModelFileRequest]) -> Vec<serde_json::Value> {
    files
        .iter()
        .map(|request| match request {
            ModelFileRequest::Required(path) => {
                serde_json::json!({"kind": "required", "path": path})
            }
            ModelFileRequest::Optional(path) => {
                serde_json::json!({"kind": "optional", "path": path})
            }
            ModelFileRequest::FirstAvailable(paths) => {
                serde_json::json!({"kind": "firstAvailable", "paths": paths})
            }
        })
        .collect()
}

fn invalid_request(operation: &str, message: impl Into<String>) -> String {
    SurfaceError::invalid_request(Some(OperationId::new(operation)), message).to_error_string()
}

fn chrono_like_epoch() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z")
        .expect("valid timestamp")
        .with_timezone(&chrono::Utc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_surface_lists_model_operations() {
        let ids = package_surface()
            .operations
            .into_iter()
            .map(|operation| operation.id.0)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"model.executionPlan".to_string()));
        assert!(ids.contains(&"model.bundlePlan".to_string()));
        assert!(ids.contains(&"model.jobManifest".to_string()));
        assert!(ids.contains(&"model.presets".to_string()));
        assert!(ids.contains(&"model.spec".to_string()));
    }

    #[test]
    fn execution_plan_operation_returns_structured_plan() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("model.executionPlan"),
            input: serde_json::json!({
                "id": "model-job-1",
                "kind": "Inference",
                "spec": {
                    "name": "demo/model",
                    "task": "text_embedding",
                    "source": {"kind": "hugging_face", "repo_id": "demo/model", "revision": "main"},
                    "files": [{"required": "config.json"}]
                },
                "backend": "heuristic",
                "inputs": [{"kind": "json", "value": {"text": "hello"}}],
                "outputArtifactPrefix": "prediction"
            }),
        })
        .expect("execution plan");

        assert_eq!(response.value["operation"], "model.executionPlan");
        assert_eq!(response.value["jobSpec"]["id"], "model-job-1");
        assert_eq!(response.value["executionPlan"]["mode"], "plannedJob");
        assert_eq!(response.value["kind"], "model-inference");
    }

    #[test]
    fn bundle_plan_selects_local_first_available_file() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("model.bundlePlan"),
            input: serde_json::json!({
                "spec": {
                    "name": "demo",
                    "task": "text_embedding",
                    "source": {"kind": "hugging_face", "repo_id": "demo/model", "revision": "main"},
                    "files": [{"first_available": ["model.safetensors", "pytorch_model.bin"]}]
                },
                "localFiles": ["pytorch_model.bin"]
            }),
        })
        .expect("bundle plan");
        assert_eq!(
            response.value["manifest"]["files"][0]["remotePath"],
            "pytorch_model.bin"
        );
        assert_eq!(response.value["downloadsRequired"], false);
    }

    #[test]
    fn job_manifest_operation_returns_manifest_projection() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("model.jobManifest"),
            input: serde_json::json!({
                "id": "model-job-1",
                "kind": "Inference",
                "spec": {
                    "name": "demo/model",
                    "task": "text_embedding",
                    "source": {"kind": "hugging_face", "repo_id": "demo/model", "revision": "main"},
                    "files": [{"required": "config.json"}]
                },
                "backend": "heuristic",
                "outputArtifactPrefix": "prediction"
            }),
        })
        .expect("job manifest");
        assert_eq!(response.value["status"], "queued");
        assert_eq!(response.value["manifest"]["jobId"], "model-job-1");
    }

    #[test]
    fn spec_operation_rejects_empty_name_with_typed_error() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("model.spec"),
            input: serde_json::json!({
                "name": "",
                "task": "text_embedding",
                "source": {"kind": "hugging_face", "repo_id": "demo/model", "revision": "main"},
                "files": []
            }),
        })
        .expect_err("empty name");
        let parsed = runtime_core::parse_surface_error(&error).expect("typed error");
        assert_eq!(parsed.code, "invalid_request");
    }
}
