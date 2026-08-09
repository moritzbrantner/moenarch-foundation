use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::{ModelRuntimeError, Result};
use jobs_core::{ArtifactKind, ArtifactRef};
use serde::{Deserialize, Serialize};

use crate::{
    DownloadedModel, HuggingFaceDownloader, HuggingFaceModelSpec, ModelDownloader,
    ModelFileRequest, ModelTask,
};

#[derive(Clone)]
/// Data type for model bundle store.
pub struct ModelBundleStore {
    root: PathBuf,
    downloader: Arc<dyn ModelDownloader + Send + Sync>,
    overwrite: bool,
}

impl std::fmt::Debug for ModelBundleStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelBundleStore")
            .field("root", &self.root)
            .field("overwrite", &self.overwrite)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Data type for model bundle manifest.
pub struct ModelBundleManifest {
    /// The schema version value.
    pub schema_version: u32,
    /// Human-readable name for this value.
    pub name: String,
    /// The repo identifier value.
    pub repo_id: String,
    /// The revision value.
    pub revision: String,
    /// The task value.
    pub task: ModelTask,
    /// The files value.
    pub files: BTreeMap<String, ModelBundleFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Data type for model bundle file.
pub struct ModelBundleFile {
    /// The remote path value.
    pub remote_path: String,
    /// The local path value.
    pub local_path: String,
    /// The size bytes value.
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
/// Data type for model bundle.
pub struct ModelBundle {
    /// The root value.
    pub root: PathBuf,
    /// The manifest value.
    pub manifest: ModelBundleManifest,
}

impl ModelBundleStore {
    /// Creates a new value.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            downloader: Arc::new(HuggingFaceDownloader::new()),
            overwrite: false,
        }
    }

    /// Returns downloader.
    pub fn downloader(mut self, downloader: HuggingFaceDownloader) -> Self {
        self.downloader = Arc::new(downloader);
        self
    }

    /// Sets a custom downloader implementation.
    pub fn model_downloader(
        mut self,
        downloader: impl ModelDownloader + Send + Sync + 'static,
    ) -> Self {
        self.downloader = Arc::new(downloader);
        self
    }

    /// Returns overwrite.
    pub fn overwrite(mut self, value: bool) -> Self {
        self.overwrite = value;
        self
    }

    /// Returns root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns bundle dir.
    pub fn bundle_dir(&self, spec: &HuggingFaceModelSpec) -> PathBuf {
        self.root
            .join(safe_bundle_segment(&spec.name))
            .join(safe_bundle_segment(&spec.revision))
    }

    /// Returns download.
    pub fn download(&self, spec: &HuggingFaceModelSpec) -> Result<ModelBundle> {
        let downloaded = self.downloader.download_model(spec)?;
        self.materialize(&downloaded)
    }

    /// Returns materialize.
    pub fn materialize(&self, downloaded: &DownloadedModel) -> Result<ModelBundle> {
        let bundle_root = self.bundle_dir(&downloaded.spec);
        let manifest_path = bundle_root.join("manifest.json");
        for remote_path in downloaded.files.keys() {
            validate_remote_path(remote_path)?;
        }
        if manifest_path.exists() && !self.overwrite {
            return ModelBundle::load(manifest_path);
        }

        let files_dir = bundle_root.join("files");
        fs::create_dir_all(&files_dir)?;

        let mut manifest_files = BTreeMap::new();
        for (remote_path, source_path) in &downloaded.files {
            let relative_file_path = Path::new("files").join(remote_path);
            let destination_path = bundle_root.join(&relative_file_path);
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)?;
            }
            if self.overwrite && fs::symlink_metadata(&destination_path).is_ok() {
                fs::remove_file(&destination_path)?;
            }
            let mut should_materialize = match fs::symlink_metadata(&destination_path) {
                Ok(_) => false,
                Err(err) if err.kind() == ErrorKind::NotFound => true,
                Err(err) => return Err(err.into()),
            };
            if !should_materialize && fs::metadata(&destination_path).is_err() {
                // A stale/dangling symlink should be replaced with fresh materialized bytes.
                fs::remove_file(&destination_path)?;
                should_materialize = true;
            }
            if should_materialize {
                let source_metadata = fs::symlink_metadata(source_path)?;
                let linked = !source_metadata.file_type().is_symlink()
                    && fs::hard_link(source_path, &destination_path).is_ok();
                if !linked {
                    let source_for_copy = if source_metadata.file_type().is_symlink() {
                        fs::canonicalize(source_path)?
                    } else {
                        source_path.clone()
                    };
                    fs::copy(source_for_copy, &destination_path)?;
                }
            }

            let size_bytes = fs::metadata(&destination_path)?.len();
            manifest_files.insert(
                remote_path.clone(),
                ModelBundleFile {
                    remote_path: remote_path.clone(),
                    local_path: path_to_manifest_string(&relative_file_path),
                    size_bytes,
                },
            );
        }

        let manifest = ModelBundleManifest {
            schema_version: 1,
            name: downloaded.spec.name.clone(),
            repo_id: downloaded.spec.repo_id.clone(),
            revision: downloaded.spec.revision.clone(),
            task: downloaded.spec.task.clone(),
            files: manifest_files,
        };
        let encoded = serde_json::to_vec_pretty(&manifest).map_err(|err| {
            ModelRuntimeError::Source(format!("failed to encode model manifest: {err}"))
        })?;
        fs::write(&manifest_path, encoded)?;

        Ok(ModelBundle {
            root: bundle_root,
            manifest,
        })
    }

    /// Returns load.
    pub fn load(&self, name: impl AsRef<str>, revision: impl AsRef<str>) -> Result<ModelBundle> {
        ModelBundle::load(
            self.root
                .join(safe_bundle_segment(name.as_ref()))
                .join(safe_bundle_segment(revision.as_ref()))
                .join("manifest.json"),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Options for resolving a local bundle, downloading it when allowed.
pub struct ModelBundleResolveOptions {
    /// Root directory containing model bundles.
    pub bundle_root: PathBuf,
    /// Whether missing bundles may be downloaded.
    pub auto_download: bool,
    /// Whether downloads should report progress.
    pub download_progress: bool,
    /// Optional Hugging Face token.
    pub hf_token: Option<String>,
    /// Optional Hugging Face cache directory.
    pub cache_dir: Option<PathBuf>,
    /// Maximum download retries.
    pub max_retries: usize,
    /// Whether materialization should overwrite existing files.
    pub overwrite: bool,
}

impl Default for ModelBundleResolveOptions {
    fn default() -> Self {
        Self {
            bundle_root: PathBuf::from(".model-runtime"),
            auto_download: true,
            download_progress: true,
            hf_token: None,
            cache_dir: None,
            max_retries: 1,
            overwrite: false,
        }
    }
}

impl ModelBundleResolveOptions {
    /// Builds the configured Hugging Face downloader.
    pub fn downloader(&self) -> HuggingFaceDownloader {
        let mut downloader = HuggingFaceDownloader::new()
            .progress(self.download_progress)
            .max_retries(self.max_retries);
        if let Some(cache_dir) = &self.cache_dir {
            downloader = downloader.cache_dir(cache_dir.clone());
        }
        if let Some(token) = &self.hf_token {
            downloader = downloader.token(token.clone());
        }
        downloader
    }
}

/// Resolves a bundle from disk, optionally downloading and materializing it first.
pub fn resolve_or_download_bundle(
    spec: &HuggingFaceModelSpec,
    options: &ModelBundleResolveOptions,
) -> Result<ModelBundle> {
    resolve_or_download_bundle_with_downloader(spec, options, options.downloader())
}

/// Resolves a bundle with a caller-provided downloader seam.
pub fn resolve_or_download_bundle_with_downloader(
    spec: &HuggingFaceModelSpec,
    options: &ModelBundleResolveOptions,
    downloader: impl ModelDownloader + Send + Sync + 'static,
) -> Result<ModelBundle> {
    let store = ModelBundleStore::new(options.bundle_root.clone())
        .model_downloader(downloader)
        .overwrite(options.overwrite);
    if let Ok(bundle) = store.load(&spec.name, &spec.revision) {
        return Ok(bundle);
    }
    if !options.auto_download {
        let expected_path = store.bundle_dir(spec).join("manifest.json");
        return Err(ModelRuntimeError::InvalidArgument(format!(
            "missing model bundle `{}` at `{}` and autoDownload is false",
            spec.name,
            expected_path.display()
        )));
    }
    store.download(spec)
}

impl ModelBundle {
    /// Returns manifest path.
    pub fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }

    /// Returns file path.
    pub fn file_path(&self, remote_path: &str) -> Option<PathBuf> {
        self.manifest
            .files
            .get(remote_path)
            .map(|file| self.root.join(&file.local_path))
    }

    /// Returns generic job artifact references for the files in this model bundle.
    pub fn artifact_refs(&self) -> Vec<ArtifactRef> {
        self.manifest
            .files
            .iter()
            .map(|(remote_path, file)| {
                let local_path = self.root.join(&file.local_path);
                let mut artifact = ArtifactRef::new(
                    format!("model:{}", remote_path.replace(['/', '\\'], "_")),
                    model_file_kind(remote_path),
                    model_file_media_type(remote_path),
                    file_uri(&local_path),
                );
                artifact.size_bytes = Some(file.size_bytes);
                artifact
                    .metadata
                    .insert("model.repoId".to_string(), self.manifest.repo_id.clone());
                artifact
                    .metadata
                    .insert("model.revision".to_string(), self.manifest.revision.clone());
                artifact.metadata.insert(
                    "model.task".to_string(),
                    self.manifest.task.as_protocol_str().to_string(),
                );
                artifact.metadata.insert(
                    "model.fileRole".to_string(),
                    model_file_role(remote_path).to_string(),
                );
                artifact
            })
            .collect()
    }

    /// Converts this value to downloaded model.
    pub fn to_downloaded_model(&self) -> DownloadedModel {
        let files = self
            .manifest
            .files
            .iter()
            .map(|(remote_path, file)| {
                (
                    remote_path.clone(),
                    absolute_path(self.root.join(&file.local_path)),
                )
            })
            .collect();
        let mut spec =
            HuggingFaceModelSpec::new(self.manifest.repo_id.clone(), self.manifest.task.clone())
                .name(self.manifest.name.clone())
                .revision(self.manifest.revision.clone());
        spec.files = self
            .manifest
            .files
            .keys()
            .map(|remote_path| ModelFileRequest::required(remote_path.clone()))
            .collect();
        DownloadedModel { spec, files }
    }

    /// Returns load.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let manifest_path = if path.is_dir() {
            path.join("manifest.json")
        } else {
            path.to_path_buf()
        };
        let root = manifest_path.parent().ok_or_else(|| {
            ModelRuntimeError::InvalidArgument(format!(
                "model bundle manifest `{}` has no parent directory",
                manifest_path.display()
            ))
        })?;
        let data = fs::read(&manifest_path)?;
        let manifest = serde_json::from_slice(&data).map_err(|err| {
            ModelRuntimeError::Source(format!(
                "failed to decode model bundle manifest `{}`: {err}",
                manifest_path.display()
            ))
        })?;
        Ok(Self {
            root: root.to_path_buf(),
            manifest,
        })
    }
}

fn safe_bundle_segment(value: &str) -> String {
    let safe = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if safe.is_empty() {
        "_".to_string()
    } else {
        safe
    }
}

fn validate_remote_path(path: &str) -> Result<()> {
    let remote_path = Path::new(path);
    if path.is_empty() || remote_path.is_absolute() {
        return Err(ModelRuntimeError::InvalidArgument(format!(
            "model file path `{path}` must be relative"
        )));
    }
    for component in remote_path.components() {
        match component {
            Component::Normal(_) => {}
            Component::ParentDir => {
                return Err(ModelRuntimeError::InvalidArgument(format!(
                    "model file path `{path}` must not contain `..`"
                )));
            }
            _ => {
                return Err(ModelRuntimeError::InvalidArgument(format!(
                    "model file path `{path}` contains an invalid path component"
                )));
            }
        }
    }
    Ok(())
}

fn path_to_manifest_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn absolute_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else if let Ok(current_dir) = std::env::current_dir() {
        current_dir.join(path)
    } else {
        path
    }
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
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
    } else if remote_path.ends_with(".txt") {
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
