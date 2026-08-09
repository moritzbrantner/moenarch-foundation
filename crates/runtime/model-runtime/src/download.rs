use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{ModelRuntimeError, Result};
use hf_hub::api::sync::ApiBuilder;
use hf_hub::{Repo, RepoType};

use crate::{HuggingFaceModelSpec, ModelFileRequest};

#[derive(Debug, Clone)]
/// Data type for downloaded model.
pub struct DownloadedModel {
    /// The spec value.
    pub spec: HuggingFaceModelSpec,
    /// The files value.
    pub files: BTreeMap<String, PathBuf>,
}

impl DownloadedModel {
    /// Returns model dir.
    pub fn model_dir(&self) -> Option<&Path> {
        self.files.values().next().and_then(|path| path.parent())
    }
}

#[derive(Debug, Clone)]
/// Data type for hugging face downloader.
pub struct HuggingFaceDownloader {
    cache_dir: Option<PathBuf>,
    token: Option<String>,
    progress: bool,
    max_retries: usize,
}

impl Default for HuggingFaceDownloader {
    fn default() -> Self {
        Self {
            cache_dir: None,
            token: None,
            progress: true,
            max_retries: 0,
        }
    }
}

impl HuggingFaceDownloader {
    /// Creates a new value.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns cache dir.
    pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(path.into());
        self
    }

    /// Returns token.
    pub fn token(mut self, value: impl Into<String>) -> Self {
        self.token = Some(value.into());
        self
    }

    /// Returns progress.
    pub fn progress(mut self, value: bool) -> Self {
        self.progress = value;
        self
    }

    /// Returns max retries.
    pub fn max_retries(mut self, value: usize) -> Self {
        self.max_retries = value;
        self
    }

    /// Returns download.
    pub fn download(&self, spec: &HuggingFaceModelSpec) -> Result<DownloadedModel> {
        if spec.files.is_empty() {
            return Err(ModelRuntimeError::InvalidArgument(
                "at least one model file must be requested".to_string(),
            ));
        }

        let mut builder = ApiBuilder::from_env()
            .with_progress(self.progress)
            .with_retries(self.max_retries)
            .with_user_agent("video-analysis", env!("CARGO_PKG_VERSION"));
        if let Some(cache_dir) = &self.cache_dir {
            builder = builder.with_cache_dir(cache_dir.clone());
        }
        builder = builder.with_token(self.token.clone());

        let api = builder
            .build()
            .map_err(|err| ModelRuntimeError::Source(format!("huggingface api error: {err}")))?;
        let repo = api.repo(Repo::with_revision(
            spec.repo_id.clone(),
            RepoType::Model,
            spec.revision.clone(),
        ));

        let mut files = BTreeMap::new();
        for request in &spec.files {
            match request {
                ModelFileRequest::Required(path) => {
                    let local = repo.get(path).map_err(|err| {
                        ModelRuntimeError::Source(format!(
                            "failed to download `{path}` from `{}`: {err}",
                            spec.repo_id
                        ))
                    })?;
                    files.insert(path.clone(), local);
                }
                ModelFileRequest::Optional(path) => {
                    if let Ok(local) = repo.get(path) {
                        files.insert(path.clone(), local);
                    }
                }
                ModelFileRequest::FirstAvailable(paths) => {
                    let mut last_error = None;
                    let mut found = None;
                    for path in paths {
                        match repo.get(path) {
                            Ok(local) => {
                                found = Some((path.clone(), local));
                                break;
                            }
                            Err(err) => last_error = Some(err.to_string()),
                        }
                    }
                    if let Some((path, local)) = found {
                        files.insert(path, local);
                    } else {
                        return Err(ModelRuntimeError::Source(format!(
                            "none of the alternative files [{}] could be downloaded from `{}`{}",
                            paths.join(", "),
                            spec.repo_id,
                            last_error
                                .map(|err| format!("; last error: {err}"))
                                .unwrap_or_default()
                        )));
                    }
                }
            }
        }

        Ok(DownloadedModel {
            spec: spec.clone(),
            files,
        })
    }
}

/// Minimal downloader seam for bundle resolution tests and alternate materializers.
pub trait ModelDownloader {
    /// Downloads or otherwise stages the requested model files.
    fn download_model(&self, spec: &HuggingFaceModelSpec) -> Result<DownloadedModel>;
}

impl ModelDownloader for HuggingFaceDownloader {
    fn download_model(&self, spec: &HuggingFaceModelSpec) -> Result<DownloadedModel> {
        self.download(spec)
    }
}
