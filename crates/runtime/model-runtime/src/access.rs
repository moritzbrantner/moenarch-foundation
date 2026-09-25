use std::collections::BTreeMap;
use std::path::PathBuf;

use runtime_core::{
    ArtifactId, CancellationToken, OperationId, RuntimeRequirement, SurfaceArtifactExpectation,
    SurfaceExecutionMode, SurfaceExecutionPlan, SurfaceSideEffect,
};
use serde::{Deserialize, Serialize};

use crate::{
    ModelFileRequest, ModelRuntimeBackend, ModelRuntimeError, ModelSource, ModelSpec, Result,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ModelArtifactKind {
    Json,
    Text,
    Binary,
    Other(String),
}

impl ModelArtifactKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Json => "json",
            Self::Text => "text",
            Self::Binary => "binary",
            Self::Other(value) => value.as_str(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelArtifactRef {
    pub id: ArtifactId,
    pub kind: ModelArtifactKind,
    pub media_type: String,
    pub uri: String,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl ModelArtifactRef {
    pub fn new(
        id: impl Into<ArtifactId>,
        kind: ModelArtifactKind,
        media_type: impl Into<String>,
        uri: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            media_type: media_type.into(),
            uri: uri.into(),
            size_bytes: None,
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelAccessKind {
    Download,
    MaterializeBundle,
    ValidateBundle,
    Warmup,
    Inference,
    BatchInference,
    ExternalCommand,
}

impl ModelAccessKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Download => "download",
            Self::MaterializeBundle => "materializeBundle",
            Self::ValidateBundle => "validateBundle",
            Self::Warmup => "warmup",
            Self::Inference => "inference",
            Self::BatchInference => "batchInference",
            Self::ExternalCommand => "externalCommand",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum ModelAccessInput {
    Json(serde_json::Value),
    ModelArtifact(ModelArtifactRef),
    LocalPath(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAccessRequest {
    pub kind: ModelAccessKind,
    pub spec: ModelSpec,
    pub backend: ModelRuntimeBackend,
    #[serde(default)]
    pub inputs: Vec<ModelAccessInput>,
    pub output_artifact_prefix: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelBundlePlan {
    pub spec: ModelSpec,
    pub manifest_path: String,
    pub files_directory: String,
    pub files: Vec<ModelBundlePlanFile>,
    pub artifacts: Vec<ModelArtifactRef>,
    pub downloads_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelBundlePlanFile {
    pub remote_path: String,
    pub local_path: String,
    pub present_locally: bool,
    pub required: bool,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelAccessPlan {
    pub kind: ModelAccessKind,
    pub backend: ModelRuntimeBackend,
    pub metadata: BTreeMap<String, String>,
    pub execution_plan: SurfaceExecutionPlan,
    pub expected_artifacts: Vec<ModelArtifactRef>,
}

pub fn plan_model_bundle(spec: &ModelSpec, local_files: &[String]) -> Result<ModelBundlePlan> {
    validate_model_spec(spec)?;
    let safe_name = spec.safe_name();
    let revision = spec.revision_value().unwrap_or("main");
    let bundle_root = format!("{safe_name}/{revision}");
    let files = resolve_requested_files(&spec.files, local_files);
    let artifacts = files
        .iter()
        .map(|file| {
            let mut artifact = ModelArtifactRef::new(
                format!("model:{}", file.remote_path.replace(['/', '\\'], "_")),
                model_file_kind(&file.remote_path),
                file.media_type.clone(),
                format!("{bundle_root}/{}", file.local_path),
            );
            artifact.metadata = model_metadata(spec, ModelRuntimeBackend::External);
            artifact.metadata.insert(
                "model.fileRole".to_string(),
                model_file_role(&file.remote_path).to_string(),
            );
            artifact
        })
        .collect::<Vec<_>>();
    let downloads_required = files
        .iter()
        .any(|file| file.required && !file.present_locally);
    Ok(ModelBundlePlan {
        spec: spec.clone(),
        manifest_path: format!("{bundle_root}/manifest.json"),
        files_directory: format!("{bundle_root}/files"),
        files,
        artifacts,
        downloads_required,
    })
}

pub fn plan_model_access(request: &ModelAccessRequest) -> Result<ModelAccessPlan> {
    validate_model_spec(&request.spec)?;
    let bundle_plan = plan_model_bundle(&request.spec, &[])?;
    let expected_artifacts = expected_artifacts_for_request(request, &bundle_plan);
    let execution_plan = SurfaceExecutionPlan {
        operation: OperationId::new("model.executionPlan"),
        mode: execution_mode_for_request(request),
        side_effects: side_effects_for_request(request),
        cancellable: matches!(
            request.kind,
            ModelAccessKind::Download
                | ModelAccessKind::MaterializeBundle
                | ModelAccessKind::ExternalCommand
        ),
        progress_unit: None,
        expected_artifacts: expected_artifacts
            .iter()
            .map(|artifact| SurfaceArtifactExpectation {
                id: artifact.id.as_str().to_string(),
                kind: artifact.kind.as_str().to_string(),
                media_type: artifact.media_type.clone(),
                required: true,
                description: Some(format!("Expected model artifact {}", artifact.id.as_str())),
            })
            .collect(),
        requirements: runtime_requirements_for_request(request),
        max_recommended_input_bytes: Some(1_048_576),
    };
    Ok(ModelAccessPlan {
        kind: request.kind,
        backend: request.backend.clone(),
        metadata: request.metadata.clone(),
        execution_plan,
        expected_artifacts,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn download_model_bundle(
    spec: &crate::HuggingFaceModelSpec,
    store: &crate::ModelBundleStore,
    cancellation: Option<&CancellationToken>,
) -> Result<crate::ModelBundle> {
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return Err(ModelRuntimeError::Cancelled);
    }
    let bundle = store.download(spec)?;
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return Err(ModelRuntimeError::Cancelled);
    }
    Ok(bundle)
}

fn validate_model_spec(spec: &ModelSpec) -> Result<()> {
    if spec.name.trim().is_empty() {
        return Err(ModelRuntimeError::InvalidArgument(
            "model name must not be empty".to_string(),
        ));
    }
    for file in &spec.files {
        match file {
            ModelFileRequest::Required(path) | ModelFileRequest::Optional(path) => {
                validate_remote_file_path(path)?;
            }
            ModelFileRequest::FirstAvailable(paths) => {
                if paths.is_empty() {
                    return Err(ModelRuntimeError::InvalidArgument(
                        "first_available model file requests must include at least one path"
                            .to_string(),
                    ));
                }
                for path in paths {
                    validate_remote_file_path(path)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_remote_file_path(path: &str) -> Result<()> {
    if path.trim().is_empty() || path.starts_with('/') || path.contains("..") {
        return Err(ModelRuntimeError::InvalidArgument(format!(
            "model file path `{path}` must be a relative file path"
        )));
    }
    Ok(())
}

fn resolve_requested_files(
    files: &[ModelFileRequest],
    local_files: &[String],
) -> Vec<ModelBundlePlanFile> {
    files
        .iter()
        .filter_map(|request| match request {
            ModelFileRequest::Required(path) => Some((path.clone(), true)),
            ModelFileRequest::Optional(path) => Some((path.clone(), false)),
            ModelFileRequest::FirstAvailable(paths) => paths
                .iter()
                .find(|path| local_files.iter().any(|local| local == *path))
                .or_else(|| paths.first())
                .map(|path| (path.clone(), true)),
        })
        .map(|(remote_path, required)| ModelBundlePlanFile {
            local_path: format!("files/{remote_path}"),
            present_locally: local_files.iter().any(|local| local == &remote_path),
            media_type: model_file_media_type(&remote_path).to_string(),
            remote_path,
            required,
        })
        .collect()
}

fn expected_artifacts_for_request(
    request: &ModelAccessRequest,
    bundle_plan: &ModelBundlePlan,
) -> Vec<ModelArtifactRef> {
    match request.kind {
        ModelAccessKind::Download | ModelAccessKind::MaterializeBundle => {
            let mut artifacts = bundle_plan.artifacts.clone();
            artifacts.push(model_manifest_artifact(request, bundle_plan));
            artifacts
        }
        ModelAccessKind::ValidateBundle => vec![model_manifest_artifact(request, bundle_plan)],
        ModelAccessKind::Warmup => Vec::new(),
        ModelAccessKind::Inference
        | ModelAccessKind::BatchInference
        | ModelAccessKind::ExternalCommand => vec![planned_output_artifact(request)],
    }
}

fn model_manifest_artifact(
    request: &ModelAccessRequest,
    bundle_plan: &ModelBundlePlan,
) -> ModelArtifactRef {
    let mut artifact = ModelArtifactRef::new(
        artifact_id(request, "manifest"),
        ModelArtifactKind::Json,
        "application/json",
        bundle_plan.manifest_path.clone(),
    );
    artifact.metadata = model_metadata(&request.spec, request.backend.clone());
    artifact
}

fn planned_output_artifact(request: &ModelAccessRequest) -> ModelArtifactRef {
    let prefix = request
        .output_artifact_prefix
        .as_deref()
        .unwrap_or("prediction");
    let mut artifact = ModelArtifactRef::new(
        format!("{prefix}:output"),
        ModelArtifactKind::Json,
        "application/json",
        "memory://model/output.json",
    );
    artifact.metadata = model_metadata(&request.spec, request.backend.clone());
    artifact
}

fn execution_mode_for_request(request: &ModelAccessRequest) -> SurfaceExecutionMode {
    if request.kind == ModelAccessKind::ExternalCommand
        || matches!(request.backend, ModelRuntimeBackend::External)
        || matches!(request.spec.source, ModelSource::ExternalCommand { .. })
    {
        SurfaceExecutionMode::ExternalCommand
    } else {
        SurfaceExecutionMode::InMemory
    }
}

fn side_effects_for_request(request: &ModelAccessRequest) -> Vec<SurfaceSideEffect> {
    match request.kind {
        ModelAccessKind::Download => {
            vec![SurfaceSideEffect::Network, SurfaceSideEffect::WritesFiles]
        }
        ModelAccessKind::MaterializeBundle => vec![
            SurfaceSideEffect::ReadsFiles,
            SurfaceSideEffect::WritesFiles,
        ],
        ModelAccessKind::ValidateBundle => vec![SurfaceSideEffect::ReadsFiles],
        ModelAccessKind::ExternalCommand => vec![SurfaceSideEffect::ExternalProcess],
        ModelAccessKind::Warmup | ModelAccessKind::Inference | ModelAccessKind::BatchInference => {
            vec![SurfaceSideEffect::None]
        }
    }
}

fn runtime_requirements_for_request(request: &ModelAccessRequest) -> Vec<RuntimeRequirement> {
    let mut requirements = Vec::new();
    if request.kind == ModelAccessKind::Download {
        requirements.push(RuntimeRequirement {
            name: "network".to_string(),
            description: Some("Model file download requires network access".to_string()),
            required: true,
        });
    }
    if matches!(request.kind, ModelAccessKind::ExternalCommand)
        || matches!(request.backend, ModelRuntimeBackend::External)
    {
        requirements.push(RuntimeRequirement {
            name: "external-command".to_string(),
            description: Some("Execution requires a caller-provided command".to_string()),
            required: true,
        });
    }
    requirements
}

fn model_file_kind(remote_path: &str) -> ModelArtifactKind {
    match model_file_role(remote_path) {
        "config" | "tokenizer" => ModelArtifactKind::Json,
        "vocabulary" => ModelArtifactKind::Text,
        _ => ModelArtifactKind::Binary,
    }
}

fn model_file_media_type(remote_path: &str) -> &'static str {
    if remote_path.ends_with(".json") {
        "application/json"
    } else if remote_path.ends_with(".txt") || remote_path.ends_with(".md") {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}

fn model_file_role(remote_path: &str) -> &'static str {
    let file_name = remote_path.rsplit('/').next().unwrap_or(remote_path);
    if file_name == "config.json" {
        "config"
    } else if file_name.contains("tokenizer") {
        "tokenizer"
    } else if matches!(file_name, "vocab.txt" | "merges.txt") {
        "vocabulary"
    } else if file_name.ends_with(".onnx")
        || file_name.ends_with(".safetensors")
        || file_name.ends_with(".bin")
        || file_name.ends_with(".pt")
    {
        "weights"
    } else {
        "artifact"
    }
}

fn model_metadata(spec: &ModelSpec, backend: ModelRuntimeBackend) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    metadata.insert("model.name".to_string(), spec.name.clone());
    metadata.insert(
        "model.task".to_string(),
        spec.task.as_protocol_str().to_string(),
    );
    metadata.insert("model.source".to_string(), spec.source.kind().to_string());
    metadata.insert("model.runtime".to_string(), backend.as_str().to_string());
    if let Some(revision) = spec.revision_value() {
        metadata.insert("model.revision".to_string(), revision.to_string());
    }
    if let Some(repo_id) = spec.repo_id_value() {
        metadata.insert("model.repoId".to_string(), repo_id.to_string());
    }
    metadata
}

fn artifact_id(request: &ModelAccessRequest, suffix: &str) -> String {
    request
        .output_artifact_prefix
        .as_deref()
        .map(|prefix| format!("{prefix}:{suffix}"))
        .unwrap_or_else(|| format!("model:{suffix}"))
}
