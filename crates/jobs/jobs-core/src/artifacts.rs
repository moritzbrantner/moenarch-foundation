use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use runtime_core::{ArtifactId, Diagnostic, DiagnosticCode, DiagnosticSeverity};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{JobError, JobId, Result};

/// Default directory for job-scoped artifact output.
pub const DEFAULT_WORKFLOW_OUTPUT_DIR: &str = ".workflow-output";

/// Generic artifact reference produced by jobs and domain runtimes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRef {
    /// Stable artifact identifier within the job.
    pub id: ArtifactId,
    /// Generic artifact kind.
    pub kind: ArtifactKind,
    /// Media type for the artifact payload.
    pub media_type: String,
    /// Local or remote URI.
    pub uri: String,
    /// Optional byte size.
    #[serde(default)]
    pub size_bytes: Option<u64>,
    /// SHA-256 checksum as lowercase hex when known.
    #[serde(default)]
    pub sha256: Option<String>,
    /// Creation timestamp for lightweight manifests.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Domain-specific metadata stored as stable string keys.
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl ArtifactRef {
    /// Creates a new artifact reference.
    pub fn new(
        id: impl Into<ArtifactId>,
        kind: ArtifactKind,
        media_type: impl Into<String>,
        uri: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            media_type: media_type.into(),
            uri: uri.into(),
            size_bytes: None,
            sha256: None,
            created_at: None,
            metadata: BTreeMap::new(),
        }
    }

    /// Adds metadata to this artifact reference.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Generic artifact kind. Domain-specific types belong in domain metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactKind {
    File,
    Directory,
    Image,
    Audio,
    Video,
    Text,
    Json,
    Log,
    Archive,
    Binary,
    Other(String),
}

/// Checksum information for artifact validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactChecksum {
    Sha256(String),
}

impl ArtifactChecksum {
    /// Computes a SHA-256 checksum for bytes.
    pub fn sha256(bytes: &[u8]) -> Self {
        Self::Sha256(sha256_hex(bytes))
    }

    /// Returns the checksum algorithm name.
    pub fn algorithm(&self) -> &'static str {
        match self {
            Self::Sha256(_) => "sha256",
        }
    }

    /// Returns the checksum value.
    pub fn value(&self) -> &str {
        match self {
            Self::Sha256(value) => value,
        }
    }
}

/// Generic artifact descriptor before materialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDescriptor {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub media_type: String,
    pub file_name: String,
    pub metadata: BTreeMap<String, String>,
}

/// Request to put bytes into an artifact store.
#[derive(Debug, Clone)]
pub struct PutArtifactRequest {
    pub job_id: JobId,
    pub artifact_id: ArtifactId,
    pub kind: ArtifactKind,
    pub media_type: String,
    pub file_name: String,
    pub bytes: Vec<u8>,
    pub metadata: BTreeMap<String, String>,
}

impl PutArtifactRequest {
    /// Creates a new file artifact put request.
    pub fn new(
        job_id: JobId,
        artifact_id: impl Into<ArtifactId>,
        file_name: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            job_id,
            artifact_id: artifact_id.into(),
            kind: ArtifactKind::File,
            media_type: "application/octet-stream".to_string(),
            file_name: file_name.into(),
            bytes: bytes.into(),
            metadata: BTreeMap::new(),
        }
    }
}

/// Request to download an artifact from a generic source URI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadArtifactRequest {
    pub job_id: JobId,
    pub artifact_id: ArtifactId,
    pub source_uri: String,
    pub file_name: String,
    pub expected_media_type: Option<String>,
    pub expected_sha256: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

/// Artifact validation result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactValidation {
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub checksum: Option<ArtifactChecksum>,
    pub size_bytes: Option<u64>,
}

/// Generic artifact store interface.
pub trait ArtifactStore {
    fn put(&mut self, request: PutArtifactRequest) -> Result<ArtifactRef>;
    fn read(&self, artifact: &ArtifactRef) -> Result<Vec<u8>>;
    fn list(&self, job_id: &JobId) -> Result<Vec<ArtifactRef>>;
}

/// Generic artifact downloader interface.
pub trait ArtifactDownloader {
    fn download(&self, request: &DownloadArtifactRequest) -> Result<ArtifactRef>;
}

/// Generic artifact validator interface.
pub trait ArtifactValidator {
    fn validate(
        &self,
        artifact: &ArtifactRef,
        store: &dyn ArtifactStore,
    ) -> Result<ArtifactValidation>;
}

/// In-memory artifact store for tests and local runtimes.
#[derive(Debug, Default, Clone)]
pub struct MemoryArtifactStore {
    artifacts: BTreeMap<JobId, Vec<StoredArtifact>>,
}

#[derive(Debug, Clone)]
struct StoredArtifact {
    artifact: ArtifactRef,
    bytes: Vec<u8>,
}

impl ArtifactStore for MemoryArtifactStore {
    fn put(&mut self, request: PutArtifactRequest) -> Result<ArtifactRef> {
        let checksum = sha256_hex(&request.bytes);
        let artifact = artifact_ref(
            request.artifact_id,
            request.kind,
            request.media_type,
            format!(
                "memory://{}/{}",
                request.job_id.as_str(),
                safe_file_name(&request.file_name)
            ),
            request.bytes.len() as u64,
            Some(checksum),
            request.metadata,
        );
        self.artifacts
            .entry(request.job_id)
            .or_default()
            .push(StoredArtifact {
                artifact: artifact.clone(),
                bytes: request.bytes,
            });
        Ok(artifact)
    }

    fn read(&self, artifact: &ArtifactRef) -> Result<Vec<u8>> {
        self.artifacts
            .values()
            .flatten()
            .find(|stored| stored.artifact.id == artifact.id && stored.artifact.uri == artifact.uri)
            .map(|stored| stored.bytes.clone())
            .ok_or_else(|| JobError::NotFound(artifact.id.as_str().to_string()))
    }

    fn list(&self, job_id: &JobId) -> Result<Vec<ArtifactRef>> {
        Ok(self
            .artifacts
            .get(job_id)
            .map(|artifacts| {
                artifacts
                    .iter()
                    .map(|stored| stored.artifact.clone())
                    .collect()
            })
            .unwrap_or_default())
    }
}

/// Local filesystem-backed artifact store.
#[derive(Debug, Clone)]
pub struct LocalFileArtifactStore {
    root: PathBuf,
}

impl LocalFileArtifactStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn workflow_output(root: impl AsRef<Path>) -> Self {
        Self::new(root.as_ref().join(DEFAULT_WORKFLOW_OUTPUT_DIR))
    }

    pub fn job_dir(&self, job_id: &JobId) -> PathBuf {
        self.root.join("jobs").join(job_id.as_str())
    }

    pub fn artifacts_dir(&self, job_id: &JobId) -> PathBuf {
        self.job_dir(job_id).join("artifacts")
    }

    pub fn logs_dir(&self, job_id: &JobId) -> PathBuf {
        self.job_dir(job_id).join("logs")
    }

    /// Downloads bytes from a local path or `file://` URI into this store.
    pub fn download_from_file(&mut self, request: &DownloadArtifactRequest) -> Result<ArtifactRef> {
        let source_path = path_from_local_source(&request.source_uri)?;
        let bytes = fs::read(source_path)?;
        if let Some(expected_sha256) = &request.expected_sha256 {
            let actual = sha256_hex(&bytes);
            if !actual.eq_ignore_ascii_case(expected_sha256) {
                return Err(JobError::InvalidArgument(format!(
                    "sha256 mismatch for {}: expected {expected_sha256}, got {actual}",
                    request.source_uri
                )));
            }
        }

        let mut put = PutArtifactRequest::new(
            request.job_id.clone(),
            request.artifact_id.clone(),
            request.file_name.clone(),
            bytes,
        );
        put.media_type = request
            .expected_media_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string());
        put.metadata = request.metadata.clone();
        self.put(put)
    }

    fn write_manifest(&self, job_id: &JobId, new_artifacts: &[ArtifactRef]) -> Result<()> {
        let mut artifacts = self.list(job_id)?;
        artifacts.extend_from_slice(new_artifacts);
        let manifest = self.job_dir(job_id).join("manifest.json");
        let bytes = serde_json::to_vec_pretty(&artifacts)
            .map_err(|error| JobError::InvalidArgument(error.to_string()))?;
        fs::write(manifest, bytes)?;
        Ok(())
    }
}

impl ArtifactStore for LocalFileArtifactStore {
    fn put(&mut self, request: PutArtifactRequest) -> Result<ArtifactRef> {
        let artifacts_dir = self.artifacts_dir(&request.job_id);
        fs::create_dir_all(&artifacts_dir)?;
        fs::create_dir_all(self.logs_dir(&request.job_id))?;
        let path = artifacts_dir.join(safe_file_name(&request.file_name));
        fs::write(&path, &request.bytes)?;
        let artifact = artifact_ref(
            request.artifact_id,
            request.kind,
            request.media_type,
            file_uri(&path),
            request.bytes.len() as u64,
            Some(sha256_hex(&request.bytes)),
            request.metadata,
        );
        self.write_manifest(&request.job_id, std::slice::from_ref(&artifact))?;
        Ok(artifact)
    }

    fn read(&self, artifact: &ArtifactRef) -> Result<Vec<u8>> {
        let path = path_from_file_uri(&artifact.uri)?;
        fs::read(path).map_err(JobError::from)
    }

    fn list(&self, job_id: &JobId) -> Result<Vec<ArtifactRef>> {
        let manifest = self.job_dir(job_id).join("manifest.json");
        if !manifest.exists() {
            return Ok(Vec::new());
        }
        let bytes = fs::read(manifest)?;
        serde_json::from_slice(&bytes).map_err(|error| JobError::InvalidArgument(error.to_string()))
    }
}

/// Local filesystem downloader backed by a `LocalFileArtifactStore`.
#[derive(Debug)]
pub struct LocalFileArtifactDownloader {
    store: std::sync::Mutex<LocalFileArtifactStore>,
}

impl LocalFileArtifactDownloader {
    pub fn new(store: LocalFileArtifactStore) -> Self {
        Self {
            store: std::sync::Mutex::new(store),
        }
    }
}

impl ArtifactDownloader for LocalFileArtifactDownloader {
    fn download(&self, request: &DownloadArtifactRequest) -> Result<ArtifactRef> {
        self.store
            .lock()
            .map_err(|_| JobError::StateUnavailable("artifact store lock poisoned".to_string()))?
            .download_from_file(request)
    }
}

/// Validates size, media type, and optional SHA-256 checksum for artifacts.
#[derive(Debug, Clone, Default)]
pub struct ExpectedArtifactValidator {
    pub expected_media_type: Option<String>,
    pub expected_sha256: Option<String>,
}

impl ArtifactValidator for ExpectedArtifactValidator {
    fn validate(
        &self,
        artifact: &ArtifactRef,
        store: &dyn ArtifactStore,
    ) -> Result<ArtifactValidation> {
        let bytes = store.read(artifact)?;
        let actual_sha256 = sha256_hex(&bytes);
        let mut diagnostics = Vec::new();

        if let Some(expected) = &self.expected_media_type {
            if artifact.media_type != *expected {
                diagnostics.push(validation_diagnostic(
                    "artifact.mediaTypeMismatch",
                    format!(
                        "expected media type `{expected}`, got `{}`",
                        artifact.media_type
                    ),
                ));
            }
        }

        if let Some(expected) = &self.expected_sha256 {
            if !actual_sha256.eq_ignore_ascii_case(expected) {
                diagnostics.push(validation_diagnostic(
                    "artifact.sha256Mismatch",
                    format!("expected sha256 `{expected}`, got `{actual_sha256}`"),
                ));
            }
        }

        Ok(ArtifactValidation {
            valid: diagnostics.is_empty(),
            diagnostics,
            checksum: Some(ArtifactChecksum::Sha256(actual_sha256)),
            size_bytes: Some(bytes.len() as u64),
        })
    }
}

fn artifact_ref(
    id: ArtifactId,
    kind: ArtifactKind,
    media_type: String,
    uri: String,
    size_bytes: u64,
    sha256: Option<String>,
    metadata: BTreeMap<String, String>,
) -> ArtifactRef {
    ArtifactRef {
        id,
        kind,
        media_type,
        uri,
        size_bytes: Some(size_bytes),
        sha256,
        created_at: Some(now_string()),
        metadata,
    }
}

fn validation_diagnostic(
    code: impl Into<DiagnosticCode>,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::new(DiagnosticSeverity::Error, code, message)
}

fn safe_file_name(file_name: &str) -> String {
    file_name
        .chars()
        .map(|character| match character {
            '/' | '\\' => '_',
            _ => character,
        })
        .collect()
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

fn path_from_local_source(uri: &str) -> Result<PathBuf> {
    if uri.starts_with("file://") {
        path_from_file_uri(uri)
    } else if uri.contains("://") {
        Err(JobError::InvalidUri(uri.to_string()))
    } else {
        Ok(PathBuf::from(uri))
    }
}

fn path_from_file_uri(uri: &str) -> Result<PathBuf> {
    uri.strip_prefix("file://")
        .map(PathBuf::from)
        .ok_or_else(|| JobError::InvalidUri(uri.to_string()))
}

fn now_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("unix:{seconds}")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job_id() -> JobId {
        JobId::new("job-1").expect("job id")
    }

    #[test]
    fn memory_store_round_trips_artifact_bytes() {
        let mut store = MemoryArtifactStore::default();
        let request = PutArtifactRequest::new(job_id(), "artifact-1", "result.txt", b"hello");

        let artifact = store.put(request).expect("put artifact");
        let bytes = store.read(&artifact).expect("read artifact");
        let artifacts = store.list(&job_id()).expect("list artifacts");

        assert_eq!(bytes, b"hello");
        assert_eq!(
            artifact.sha256.as_deref(),
            Some(sha256_hex(b"hello").as_str())
        );
        assert_eq!(artifacts, vec![artifact]);
    }

    #[test]
    fn local_file_store_writes_manifest_and_reads_bytes() {
        let root = temp_root("jobs-core-artifacts-local");
        let mut store = LocalFileArtifactStore::new(&root);
        let request = PutArtifactRequest::new(job_id(), "artifact-1", "result.txt", b"hello");

        let artifact = store.put(request).expect("put artifact");
        let bytes = store.read(&artifact).expect("read artifact");
        let artifacts = store.list(&job_id()).expect("list artifacts");

        assert_eq!(bytes, b"hello");
        assert_eq!(artifacts, vec![artifact]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_file_store_appends_manifest_entries() {
        let root = temp_root("jobs-core-artifacts-manifest");
        let mut store = LocalFileArtifactStore::new(&root);

        store
            .put(PutArtifactRequest::new(
                job_id(),
                "artifact-1",
                "one.txt",
                b"one",
            ))
            .expect("put artifact 1");
        store
            .put(PutArtifactRequest::new(
                job_id(),
                "artifact-2",
                "two.txt",
                b"two",
            ))
            .expect("put artifact 2");

        assert_eq!(store.list(&job_id()).expect("list artifacts").len(), 2);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_file_store_sanitizes_file_names() {
        let root = temp_root("jobs-core-artifacts-safe");
        let mut store = LocalFileArtifactStore::new(&root);
        let artifact = store
            .put(PutArtifactRequest::new(
                job_id(),
                "artifact-1",
                "../result.txt",
                b"hello",
            ))
            .expect("put artifact");

        assert!(
            artifact.uri.ends_with("_.._result.txt") || artifact.uri.ends_with(".._result.txt")
        );
        assert!(store.read(&artifact).is_ok());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checksum_validation_reports_success_and_failure() {
        let mut store = MemoryArtifactStore::default();
        let artifact = store
            .put(PutArtifactRequest::new(
                job_id(),
                "artifact-1",
                "result.txt",
                b"hello",
            ))
            .expect("put artifact");

        let ok = ExpectedArtifactValidator {
            expected_media_type: None,
            expected_sha256: Some(sha256_hex(b"hello")),
        }
        .validate(&artifact, &store)
        .expect("validate success");
        assert!(ok.valid);

        let failed = ExpectedArtifactValidator {
            expected_media_type: None,
            expected_sha256: Some(sha256_hex(b"goodbye")),
        }
        .validate(&artifact, &store)
        .expect("validate failure");
        assert!(!failed.valid);
        assert_eq!(failed.diagnostics.len(), 1);
    }

    #[test]
    fn invalid_uri_and_missing_artifact_are_rejected() {
        let store = LocalFileArtifactStore::new(temp_root("jobs-core-artifacts-invalid"));
        let artifact = ArtifactRef::new(
            "missing",
            ArtifactKind::File,
            "application/octet-stream",
            "https://example.invalid/artifact",
        );

        assert!(matches!(
            store.read(&artifact),
            Err(JobError::InvalidUri(_))
        ));

        let memory = MemoryArtifactStore::default();
        assert!(matches!(memory.read(&artifact), Err(JobError::NotFound(_))));
    }

    #[test]
    fn local_file_downloader_materializes_file_uri() {
        let source_root = temp_root("jobs-core-artifacts-source");
        fs::create_dir_all(&source_root).expect("create source root");
        let source = source_root.join("source.bin");
        fs::write(&source, b"hello").expect("write source");

        let target_root = temp_root("jobs-core-artifacts-download");
        let mut store = LocalFileArtifactStore::new(&target_root);
        let request = DownloadArtifactRequest {
            job_id: job_id(),
            artifact_id: "artifact-1".into(),
            source_uri: file_uri(&source),
            file_name: "download.bin".to_string(),
            expected_media_type: Some("application/octet-stream".to_string()),
            expected_sha256: Some(sha256_hex(b"hello")),
            metadata: BTreeMap::new(),
        };

        let artifact = store
            .download_from_file(&request)
            .expect("download artifact");
        assert_eq!(store.read(&artifact).expect("read artifact"), b"hello");

        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(target_root);
    }

    fn temp_root(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
