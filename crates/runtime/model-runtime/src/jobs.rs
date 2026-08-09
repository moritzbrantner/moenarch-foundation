use std::collections::BTreeMap;
#[cfg(feature = "jobs")]
use std::path::Path;
use std::path::PathBuf;
#[cfg(feature = "jobs")]
use std::process::Command;
#[cfg(feature = "jobs")]
use std::sync::{Arc, Mutex};

use jobs_core::{ArtifactKind, ArtifactRef, JobSpec};
#[cfg(feature = "jobs")]
use jobs_core::{BackgroundJobRunner, JobArtifact, JobError, JobProgress};
use runtime_core::{
    Diagnostic, OperationId, RuntimeRequirement, SurfaceArtifactExpectation, SurfaceExecutionMode,
    SurfaceExecutionPlan, SurfaceSideEffect,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "jobs")]
use crate::{ModelBundle, ModelBundleStore};
use crate::{ModelFileRequest, ModelRuntimeBackend, ModelSource, ModelSpec};

/// Long-running model operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelJobKind {
    /// Download remote model files.
    Download,
    /// Materialize a local model bundle.
    MaterializeBundle,
    /// Validate an existing model bundle.
    ValidateBundle,
    /// Warm a runtime/session.
    Warmup,
    /// Run one inference request.
    Inference,
    /// Run batch inference.
    BatchInference,
    /// Run an external model command.
    ExternalCommand,
}

/// Generic request for long-running or side-effectful model access.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAccessJobRequest {
    /// Optional caller-supplied job id.
    pub id: Option<String>,
    /// The model operation kind.
    pub kind: ModelJobKind,
    /// Model spec or preset-derived spec.
    pub spec: ModelSpec,
    /// Runtime backend that will handle the request.
    pub backend: ModelRuntimeBackend,
    /// Input values or artifacts for the job.
    #[serde(default)]
    pub inputs: Vec<ModelJobInput>,
    /// Optional artifact id prefix for outputs.
    pub output_artifact_prefix: Option<String>,
    /// Caller-defined metadata.
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

/// Input accepted by generic model jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum ModelJobInput {
    /// JSON request payload.
    Json(serde_json::Value),
    /// Artifact produced by a previous job.
    Artifact(ArtifactRef),
    /// Local path supplied by an explicit CLI/server/job route.
    LocalPath(PathBuf),
}

/// Generic result returned by model access jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAccessJobResult {
    /// Stable job identifier.
    pub job_id: jobs_core::JobId,
    /// Model operation kind.
    pub kind: ModelJobKind,
    /// Model spec used by the job.
    pub spec: ModelSpec,
    /// Runtime backend used by the job.
    pub backend: ModelRuntimeBackend,
    /// Artifacts produced by the job.
    pub artifacts: Vec<ArtifactRef>,
    /// Diagnostics emitted by the job.
    pub diagnostics: Vec<Diagnostic>,
    /// Optional structured output.
    pub output: Option<serde_json::Value>,
}

/// Pure plan for a model bundle layout before files are materialized.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelBundlePlan {
    pub spec: ModelSpec,
    pub manifest_path: String,
    pub files_directory: String,
    pub files: Vec<ModelBundlePlanFile>,
    pub artifact_refs: Vec<ArtifactRef>,
    pub downloads_required: bool,
}

/// One planned model bundle file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelBundlePlanFile {
    pub remote_path: String,
    pub local_path: String,
    pub present_locally: bool,
    pub required: bool,
    pub media_type: String,
}

/// Pure access plan for a model job request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelAccessPlan {
    pub job_spec: jobs_core::JobSpec,
    pub kind: ModelJobKind,
    pub backend: ModelRuntimeBackend,
    pub execution_plan: SurfaceExecutionPlan,
    pub expected_artifacts: Vec<ArtifactRef>,
}

impl ModelJobKind {
    /// Returns the stable job kind string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Download => "model-download",
            Self::MaterializeBundle => "model-materialize-bundle",
            Self::ValidateBundle => "model-validate-bundle",
            Self::Warmup => "model-warmup",
            Self::Inference => "model-inference",
            Self::BatchInference => "model-batch-inference",
            Self::ExternalCommand => "model-external-command",
        }
    }
}

/// Builds a standard model job spec with model/runtime metadata.
pub fn model_job_spec(
    id: impl Into<String>,
    kind: ModelJobKind,
    spec: &ModelSpec,
    backend: ModelRuntimeBackend,
) -> jobs_core::Result<JobSpec> {
    let mut job = JobSpec::new(id, format!("{} {}", kind.as_str(), spec.name))?
        .with_kind(kind.as_str())?
        .with_metadata("model.name", spec.name.clone())?
        .with_metadata("model.task", spec.task.as_protocol_str().to_string())?
        .with_metadata("model.source", spec.source.kind().to_string())?
        .with_metadata("model.runtime", backend.as_str().to_string())?;
    if let Some(revision) = spec.revision_value() {
        job = job.with_metadata("model.revision", revision.to_string())?;
    }
    if let Some(repo_id) = spec.repo_id_value() {
        job = job.with_metadata("model.repoId", repo_id.to_string())?;
    }
    Ok(job)
}

/// Plans a model bundle without downloading or materializing files.
pub fn plan_model_bundle(
    spec: &ModelSpec,
    local_files: &[String],
) -> crate::Result<ModelBundlePlan> {
    validate_model_spec(spec)?;
    let safe_name = spec.safe_name();
    let revision = spec.revision_value().unwrap_or("main");
    let bundle_root = format!("{safe_name}/{revision}");
    let files = resolve_requested_files(&spec.files, local_files);
    let artifact_refs = files
        .iter()
        .map(|file| {
            let mut artifact = ArtifactRef::new(
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
        artifact_refs,
        downloads_required,
    })
}

/// Plans model access as a job without spawning work or invoking model runtimes.
pub fn plan_model_access(request: &ModelAccessJobRequest) -> jobs_core::Result<ModelAccessPlan> {
    validate_model_spec_for_job(&request.spec)?;
    let job_spec = job_spec_for_request(request)?;
    let bundle_plan = plan_model_bundle(&request.spec, &[])
        .map_err(|error| jobs_core::JobError::InvalidArgument(error.to_string()))?;
    let expected_artifacts = expected_artifacts_for_request(request, &bundle_plan);
    let execution_plan = SurfaceExecutionPlan {
        operation: OperationId::new("model.executionPlan"),
        mode: execution_mode_for_request(request),
        side_effects: side_effects_for_request(request),
        cancellable: true,
        progress_unit: Some(progress_unit_for_kind(request.kind).to_string()),
        expected_artifacts: expected_artifacts
            .iter()
            .map(|artifact| SurfaceArtifactExpectation {
                id: artifact.id.as_str().to_string(),
                kind: artifact_kind_name(&artifact.kind),
                media_type: artifact.media_type.clone(),
                required: true,
                description: Some(format!("Expected {} artifact", artifact.id.as_str())),
            })
            .collect(),
        requirements: runtime_requirements_for_request(request),
        max_recommended_input_bytes: Some(1_048_576),
    };
    Ok(ModelAccessPlan {
        job_spec,
        kind: request.kind,
        backend: request.backend.clone(),
        execution_plan,
        expected_artifacts,
    })
}

fn validate_model_spec(spec: &ModelSpec) -> crate::Result<()> {
    if spec.name.trim().is_empty() {
        return Err(crate::ModelRuntimeError::InvalidArgument(
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
                    return Err(crate::ModelRuntimeError::InvalidArgument(
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

fn validate_model_spec_for_job(spec: &ModelSpec) -> jobs_core::Result<()> {
    validate_model_spec(spec)
        .map_err(|error| jobs_core::JobError::InvalidArgument(error.to_string()))
}

fn validate_remote_file_path(path: &str) -> crate::Result<()> {
    if path.trim().is_empty() || path.starts_with('/') || path.contains("..") {
        return Err(crate::ModelRuntimeError::InvalidArgument(format!(
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
    request: &ModelAccessJobRequest,
    bundle_plan: &ModelBundlePlan,
) -> Vec<ArtifactRef> {
    match request.kind {
        ModelJobKind::Download | ModelJobKind::MaterializeBundle => {
            let mut artifacts = bundle_plan.artifact_refs.clone();
            artifacts.push(model_manifest_artifact(request, bundle_plan));
            artifacts
        }
        ModelJobKind::ValidateBundle => vec![model_manifest_artifact(request, bundle_plan)],
        ModelJobKind::Warmup => Vec::new(),
        ModelJobKind::Inference | ModelJobKind::BatchInference | ModelJobKind::ExternalCommand => {
            vec![planned_output_artifact(request)]
        }
    }
}

fn model_manifest_artifact(
    request: &ModelAccessJobRequest,
    bundle_plan: &ModelBundlePlan,
) -> ArtifactRef {
    let mut artifact = ArtifactRef::new(
        artifact_id(request, "manifest"),
        ArtifactKind::Json,
        "application/json",
        bundle_plan.manifest_path.clone(),
    );
    artifact.metadata = model_metadata(&request.spec, request.backend.clone());
    artifact
}

fn planned_output_artifact(request: &ModelAccessJobRequest) -> ArtifactRef {
    let mut artifact = ArtifactRef::new(
        artifact_id(request, "output"),
        ArtifactKind::Json,
        "application/json",
        format!("memory://{}/output.json", default_job_id(request)),
    );
    artifact.metadata = model_metadata(&request.spec, request.backend.clone());
    artifact
}

fn execution_mode_for_request(request: &ModelAccessJobRequest) -> SurfaceExecutionMode {
    if request.kind == ModelJobKind::ExternalCommand
        || matches!(request.backend, ModelRuntimeBackend::External)
        || matches!(request.spec.source, ModelSource::ExternalCommand { .. })
    {
        SurfaceExecutionMode::ExternalCommand
    } else {
        SurfaceExecutionMode::PlannedJob
    }
}

fn side_effects_for_request(request: &ModelAccessJobRequest) -> Vec<SurfaceSideEffect> {
    match request.kind {
        ModelJobKind::Download => vec![SurfaceSideEffect::Network, SurfaceSideEffect::WritesFiles],
        ModelJobKind::MaterializeBundle => vec![
            SurfaceSideEffect::ReadsFiles,
            SurfaceSideEffect::WritesFiles,
        ],
        ModelJobKind::ValidateBundle => vec![SurfaceSideEffect::ReadsFiles],
        ModelJobKind::ExternalCommand => vec![SurfaceSideEffect::ExternalProcess],
        ModelJobKind::Warmup | ModelJobKind::Inference | ModelJobKind::BatchInference => {
            vec![SurfaceSideEffect::None]
        }
    }
}

fn progress_unit_for_kind(kind: ModelJobKind) -> &'static str {
    match kind {
        ModelJobKind::BatchInference => "inputs",
        ModelJobKind::Download | ModelJobKind::MaterializeBundle | ModelJobKind::ValidateBundle => {
            "files"
        }
        ModelJobKind::Warmup | ModelJobKind::Inference | ModelJobKind::ExternalCommand => "steps",
    }
}

fn runtime_requirements_for_request(request: &ModelAccessJobRequest) -> Vec<RuntimeRequirement> {
    let mut requirements = Vec::new();
    if request.kind == ModelJobKind::Download {
        requirements.push(RuntimeRequirement {
            name: "network".to_string(),
            description: Some("Model file download requires network access".to_string()),
            required: true,
        });
    }
    if matches!(request.kind, ModelJobKind::ExternalCommand)
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

fn model_file_kind(remote_path: &str) -> ArtifactKind {
    match model_file_role(remote_path) {
        "config" | "tokenizer" => ArtifactKind::Json,
        "vocabulary" => ArtifactKind::Text,
        _ => ArtifactKind::Binary,
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

fn artifact_kind_name(kind: &ArtifactKind) -> String {
    match kind {
        ArtifactKind::File => "file",
        ArtifactKind::Directory => "directory",
        ArtifactKind::Image => "image",
        ArtifactKind::Audio => "audio",
        ArtifactKind::Video => "video",
        ArtifactKind::Text => "text",
        ArtifactKind::Json => "json",
        ArtifactKind::Log => "log",
        ArtifactKind::Archive => "archive",
        ArtifactKind::Binary => "binary",
        ArtifactKind::Other(value) => value.as_str(),
    }
    .to_string()
}

/// Join handle for model jobs that produce a typed value.
#[cfg(feature = "jobs")]
#[derive(Debug)]
pub struct ModelJobJoinHandle<T> {
    inner: jobs_core::JobJoinHandle,
    value: Arc<Mutex<Option<T>>>,
}

#[cfg(feature = "jobs")]
impl<T: Clone> ModelJobJoinHandle<T> {
    /// Returns the underlying tracked job.
    pub fn job(&self) -> &jobs_core::JobHandle {
        self.inner.job()
    }

    /// Returns the job id.
    pub fn id(&self) -> &jobs_core::JobId {
        self.inner.id()
    }

    /// Requests cancellation.
    pub fn request_cancel(&self) -> jobs_core::Result<()> {
        self.inner.request_cancel()
    }

    /// Waits for completion and returns the produced value.
    pub fn join_result(&mut self) -> jobs_core::Result<T> {
        self.inner.join()?;
        self.value
            .lock()
            .map_err(|_| JobError::StateUnavailable("model job result lock poisoned".to_string()))?
            .clone()
            .ok_or_else(|| JobError::Failed("model job did not produce a result".to_string()))
    }
}

/// Spawns a model download/materialization job.
#[cfg(feature = "jobs")]
pub fn spawn_model_download_job(
    runner: &BackgroundJobRunner,
    spec: ModelSpec,
    store: ModelBundleStore,
) -> jobs_core::Result<ModelJobJoinHandle<ModelBundle>> {
    let value = Arc::new(Mutex::new(None));
    let value_for_job = Arc::clone(&value);
    let job_spec = model_job_spec(
        format!(
            "model-download-{}-{}",
            spec.safe_name(),
            spec.revision_value().unwrap_or("local")
        ),
        ModelJobKind::Download,
        &spec,
        ModelRuntimeBackend::External,
    )?;
    let inner = runner.spawn(job_spec, move |context| {
        context.info(format!("materializing model bundle `{}`", spec.name))?;
        context.progress(
            JobProgress::new(0, Some(2))?
                .unit("steps")?
                .message("starting model download"),
        )?;
        context.check_cancelled()?;
        let bundle = store
            .download(&spec)
            .map_err(|err| JobError::Failed(err.to_string()))?;
        context.check_cancelled()?;
        context.progress(
            JobProgress::new(1, Some(2))?
                .unit("steps")?
                .message("model files materialized"),
        )?;
        context.artifact(
            JobArtifact::new("manifest", "Model bundle manifest")
                .kind("model-bundle")
                .path(bundle.manifest_path()),
        )?;
        *value_for_job.lock().map_err(|_| {
            JobError::StateUnavailable("model job result lock poisoned".to_string())
        })? = Some(bundle);
        context.progress(
            JobProgress::new(2, Some(2))?
                .unit("steps")?
                .message("model bundle ready"),
        )?;
        Ok(())
    })?;
    Ok(ModelJobJoinHandle { inner, value })
}

/// Spawns a model bundle materialization job.
#[cfg(feature = "jobs")]
pub fn spawn_model_materialize_job(
    runner: &BackgroundJobRunner,
    request: ModelAccessJobRequest,
    store: ModelBundleStore,
) -> jobs_core::Result<ModelJobJoinHandle<ModelAccessJobResult>> {
    spawn_access_job(runner, request, move |context, request| {
        context.info(format!(
            "materializing model bundle `{}`",
            request.spec.name
        ))?;
        context.progress(
            JobProgress::new(0, Some(2))?
                .unit("steps")?
                .message("starting model materialization"),
        )?;
        context.check_cancelled()?;
        let bundle = store
            .download(&request.spec)
            .map_err(|err| JobError::Failed(err.to_string()))?;
        context.check_cancelled()?;
        let mut artifacts = bundle.artifact_refs();
        let manifest_artifact = artifact_ref_for_path(
            artifact_id(&request, "manifest"),
            ArtifactKind::Json,
            "application/json",
            &bundle.manifest_path(),
            model_metadata(&request.spec, request.backend.clone()),
        );
        context.artifact(
            JobArtifact::new("manifest", "Model bundle manifest")
                .kind("model-bundle")
                .path(bundle.manifest_path()),
        )?;
        artifacts.push(manifest_artifact);
        context.progress(
            JobProgress::new(2, Some(2))?
                .unit("steps")?
                .message("model bundle materialized"),
        )?;
        Ok(ModelAccessJobResult {
            job_id: context.id().clone(),
            kind: request.kind,
            spec: request.spec,
            backend: request.backend,
            artifacts,
            diagnostics: Vec::new(),
            output: Some(serde_json::json!({
                "bundleRoot": bundle.root,
                "manifestPath": bundle.manifest_path(),
            })),
        })
    })
}

/// Spawns a model bundle validation job.
#[cfg(feature = "jobs")]
pub fn spawn_model_validate_job(
    runner: &BackgroundJobRunner,
    request: ModelAccessJobRequest,
) -> jobs_core::Result<ModelJobJoinHandle<ModelAccessJobResult>> {
    spawn_access_job(runner, request, move |context, request| {
        context.info(format!(
            "validating model access for `{}`",
            request.spec.name
        ))?;
        context.check_cancelled()?;
        let mut artifacts = Vec::new();
        for (index, input) in request.inputs.iter().enumerate() {
            if let ModelJobInput::LocalPath(path) = input {
                if !path.exists() {
                    return Err(JobError::NotFound(format!(
                        "model input path `{}` does not exist",
                        path.display()
                    )));
                }
                let kind = if path.is_dir() {
                    ArtifactKind::Directory
                } else {
                    ArtifactKind::File
                };
                let artifact = artifact_ref_for_path(
                    artifact_id(&request, &format!("input-{index}")),
                    kind,
                    "application/octet-stream",
                    path,
                    model_metadata(&request.spec, request.backend.clone()),
                );
                context.artifact(
                    JobArtifact::new(format!("input-{index}"), "Validated model input").path(path),
                )?;
                artifacts.push(artifact);
            }
        }
        context.check_cancelled()?;
        Ok(ModelAccessJobResult {
            job_id: context.id().clone(),
            kind: request.kind,
            spec: request.spec,
            backend: request.backend,
            artifacts,
            diagnostics: Vec::new(),
            output: Some(serde_json::json!({ "valid": true })),
        })
    })
}

/// Spawns a runtime warmup job.
#[cfg(feature = "jobs")]
pub fn spawn_model_warmup_job(
    runner: &BackgroundJobRunner,
    request: ModelAccessJobRequest,
) -> jobs_core::Result<ModelJobJoinHandle<ModelAccessJobResult>> {
    spawn_access_job(runner, request, move |context, request| {
        context.info(format!(
            "warming model runtime `{}`",
            request.backend.as_str()
        ))?;
        context.check_cancelled()?;
        context.progress(
            JobProgress::new(1, Some(1))?
                .unit("steps")?
                .message("model warmup recorded"),
        )?;
        Ok(empty_access_result(
            context.id().clone(),
            request,
            Some(serde_json::json!({ "warmed": true })),
        ))
    })
}

/// Spawns a single model inference job.
#[cfg(feature = "jobs")]
pub fn spawn_model_inference_job(
    runner: &BackgroundJobRunner,
    request: ModelAccessJobRequest,
) -> jobs_core::Result<ModelJobJoinHandle<ModelAccessJobResult>> {
    spawn_access_job(runner, request, move |context, request| {
        context.info(format!("running model inference `{}`", request.spec.name))?;
        context.check_cancelled()?;
        let output = serde_json::json!({
            "inputCount": request.inputs.len(),
            "backend": request.backend.as_str(),
        });
        context.check_cancelled()?;
        Ok(empty_access_result(
            context.id().clone(),
            request,
            Some(output),
        ))
    })
}

/// Spawns a batch model inference job.
#[cfg(feature = "jobs")]
pub fn spawn_model_batch_inference_job(
    runner: &BackgroundJobRunner,
    request: ModelAccessJobRequest,
) -> jobs_core::Result<ModelJobJoinHandle<ModelAccessJobResult>> {
    spawn_access_job(runner, request, move |context, request| {
        context.info(format!(
            "running batch model inference `{}`",
            request.spec.name
        ))?;
        context.check_cancelled()?;
        let input_count = request.inputs.len();
        context.progress(
            JobProgress::new(input_count as u64, Some(input_count.max(1) as u64))?
                .unit("inputs")?
                .message("batch inputs accepted"),
        )?;
        context.check_cancelled()?;
        Ok(empty_access_result(
            context.id().clone(),
            request,
            Some(serde_json::json!({ "inputCount": input_count })),
        ))
    })
}

/// Spawns an external model command job.
#[cfg(feature = "jobs")]
pub fn spawn_external_model_command_job(
    runner: &BackgroundJobRunner,
    request: ModelAccessJobRequest,
) -> jobs_core::Result<ModelJobJoinHandle<ModelAccessJobResult>> {
    spawn_access_job(runner, request, move |context, request| {
        let command = match &request.spec.source {
            ModelSource::ExternalCommand { command } => command.clone(),
            _ => {
                return Err(JobError::InvalidArgument(
                    "external model command jobs require ModelSource::ExternalCommand".to_string(),
                ));
            }
        };
        context.info(format!(
            "running external model command `{}`",
            command.display()
        ))?;
        context.check_cancelled()?;
        let output = Command::new(&command).output().map_err(|err| {
            JobError::Failed(format!("failed to run `{}`: {err}", command.display()))
        })?;
        context.check_cancelled()?;
        if !output.status.success() {
            return Err(JobError::Failed(format!(
                "`{}` exited with status {}",
                command.display(),
                output.status
            )));
        }
        Ok(empty_access_result(
            context.id().clone(),
            request,
            Some(serde_json::json!({
                "status": output.status.code(),
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr),
            })),
        ))
    })
}

/// Runs a model job request inline for tests and deterministic contract checks.
pub fn run_model_job_inline_for_tests(
    request: ModelAccessJobRequest,
) -> jobs_core::Result<ModelAccessJobResult> {
    let job_id = jobs_core::JobId::new(
        request
            .id
            .clone()
            .unwrap_or_else(|| default_job_id(&request)),
    )?;
    Ok(empty_access_result(
        job_id,
        request,
        Some(serde_json::json!({ "inline": true })),
    ))
}

#[cfg(feature = "jobs")]
fn spawn_access_job<F>(
    runner: &BackgroundJobRunner,
    request: ModelAccessJobRequest,
    run: F,
) -> jobs_core::Result<ModelJobJoinHandle<ModelAccessJobResult>>
where
    F: FnOnce(
            jobs_core::JobContext,
            ModelAccessJobRequest,
        ) -> jobs_core::Result<ModelAccessJobResult>
        + Send
        + 'static,
{
    let value = Arc::new(Mutex::new(None));
    let value_for_job = Arc::clone(&value);
    let job_spec = job_spec_for_request(&request)?;
    let inner = runner.spawn(job_spec, move |context| {
        for (key, value) in model_metadata(&request.spec, request.backend.clone()) {
            context.metadata(key, value)?;
        }
        for (key, value) in &request.metadata {
            context.metadata(key.clone(), value.clone())?;
        }
        let result = run(context, request)?;
        *value_for_job.lock().map_err(|_| {
            JobError::StateUnavailable("model job result lock poisoned".to_string())
        })? = Some(result);
        Ok(())
    })?;
    Ok(ModelJobJoinHandle { inner, value })
}

fn job_spec_for_request(request: &ModelAccessJobRequest) -> jobs_core::Result<JobSpec> {
    let id = request
        .id
        .clone()
        .unwrap_or_else(|| default_job_id(request));
    let mut job = model_job_spec(id, request.kind, &request.spec, request.backend.clone())?;
    for (key, value) in &request.metadata {
        job = job.with_metadata(key.clone(), value.clone())?;
    }
    Ok(job)
}

fn default_job_id(request: &ModelAccessJobRequest) -> String {
    format!(
        "{}-{}-{}",
        request.kind.as_str(),
        request.spec.safe_name(),
        request.spec.revision_value().unwrap_or("local")
    )
}

fn empty_access_result(
    job_id: jobs_core::JobId,
    request: ModelAccessJobRequest,
    output: Option<serde_json::Value>,
) -> ModelAccessJobResult {
    ModelAccessJobResult {
        job_id,
        kind: request.kind,
        spec: request.spec,
        backend: request.backend,
        artifacts: Vec::new(),
        diagnostics: Vec::new(),
        output,
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

fn artifact_id(request: &ModelAccessJobRequest, suffix: &str) -> String {
    match &request.output_artifact_prefix {
        Some(prefix) => format!("{prefix}-{suffix}"),
        None => suffix.to_string(),
    }
}

#[cfg(feature = "jobs")]
fn artifact_ref_for_path(
    id: impl Into<runtime_core::ArtifactId>,
    kind: ArtifactKind,
    media_type: impl Into<String>,
    path: &Path,
    metadata: BTreeMap<String, String>,
) -> ArtifactRef {
    let mut artifact = ArtifactRef::new(id, kind, media_type, file_uri(path));
    artifact.size_bytes = path.metadata().ok().map(|metadata| metadata.len());
    artifact.metadata = metadata;
    artifact
}

#[cfg(feature = "jobs")]
fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}
