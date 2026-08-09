#![doc = include_str!("../README.md")]

pub mod artifacts;
pub mod surface;
pub use artifacts::*;

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};

use chrono::{DateTime, Utc};
pub use runtime_core::ArtifactId;
use runtime_core::{Diagnostic, OperationId};
use serde::{Deserialize, Serialize};

/// Result type used by this crate.
pub type Result<T> = std::result::Result<T, JobError>;

/// Error type used by this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    /// The supplied argument was invalid.
    InvalidArgument(String),
    /// A referenced job or artifact was not found.
    NotFound(String),
    /// A URI or path could not be interpreted safely.
    InvalidUri(String),
    /// An I/O operation failed.
    Io(String),
    /// The job was cancelled.
    Cancelled,
    /// The job failed.
    Failed(String),
    /// Shared job state could not be accessed.
    StateUnavailable(String),
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(f, "invalid argument: {message}"),
            Self::NotFound(message) => write!(f, "not found: {message}"),
            Self::InvalidUri(uri) => write!(f, "invalid uri: {uri}"),
            Self::Io(message) => write!(f, "io error: {message}"),
            Self::Cancelled => write!(f, "job cancelled"),
            Self::Failed(message) => write!(f, "job failed: {message}"),
            Self::StateUnavailable(message) => write!(f, "job state unavailable: {message}"),
        }
    }
}

impl std::error::Error for JobError {}

impl From<std::io::Error> for JobError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

/// Stable identifier for a job.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct JobId(String);

impl JobId {
    /// Creates a new identifier.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(JobError::InvalidArgument(
                "job id must not be empty".to_string(),
            ));
        }
        Ok(Self(value))
    }

    /// Returns this identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// User-facing job definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSpec {
    /// Stable job identifier.
    pub id: JobId,
    /// Human-readable job name.
    pub name: String,
    /// Optional job kind for filtering and routing.
    pub kind: Option<String>,
    /// Caller-defined metadata.
    pub metadata: BTreeMap<String, String>,
}

impl JobSpec {
    /// Creates a new job definition.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(JobError::InvalidArgument(
                "job name must not be empty".to_string(),
            ));
        }
        Ok(Self {
            id: JobId::new(id)?,
            name,
            kind: None,
            metadata: BTreeMap::new(),
        })
    }

    /// Sets the optional job kind.
    pub fn with_kind(mut self, kind: impl Into<String>) -> Result<Self> {
        let kind = kind.into();
        if kind.trim().is_empty() {
            return Err(JobError::InvalidArgument(
                "job kind must not be empty".to_string(),
            ));
        }
        self.kind = Some(kind);
        Ok(self)
    }

    /// Adds caller-defined metadata.
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self> {
        insert_metadata(&mut self.metadata, key, value)?;
        Ok(self)
    }
}

/// Current lifecycle state for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobStatus {
    /// The job has been created but has not started.
    Queued,
    /// The job is actively running.
    Running,
    /// Cancellation was requested and worker code should stop soon.
    Cancelling,
    /// The job completed successfully.
    Succeeded,
    /// The job stopped because it failed.
    Failed,
    /// The job stopped after cancellation.
    Cancelled,
}

impl JobStatus {
    /// Returns whether this status is terminal.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

/// Filter used to list matching job snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobListFilter {
    /// Optional lifecycle status filter.
    pub status: Option<JobStatus>,
    /// Optional job kind filter.
    pub kind: Option<String>,
    /// Metadata key/value pairs that must all match.
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

/// A structured failure record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobFailure {
    /// Failure message.
    pub message: String,
}

/// Progress snapshot for a long-running job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobProgress {
    /// Completed units.
    pub completed: u64,
    /// Optional total units.
    pub total: Option<u64>,
    /// Unit label, for example `files`, `frames`, or `steps`.
    pub unit: String,
    /// Optional display message.
    pub message: Option<String>,
}

impl JobProgress {
    /// Creates a new progress snapshot.
    pub fn new(completed: u64, total: Option<u64>) -> Result<Self> {
        let progress = Self {
            completed,
            total,
            unit: "steps".to_string(),
            message: None,
        };
        progress.validate()?;
        Ok(progress)
    }

    /// Sets the progress unit.
    pub fn unit(mut self, unit: impl Into<String>) -> Result<Self> {
        let unit = unit.into();
        if unit.trim().is_empty() {
            return Err(JobError::InvalidArgument(
                "progress unit must not be empty".to_string(),
            ));
        }
        self.unit = unit;
        Ok(self)
    }

    /// Sets a display message.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Returns the completed fraction when a total is known.
    pub fn fraction(&self) -> Option<f64> {
        let total = self.total?;
        if total == 0 {
            return None;
        }
        Some(self.completed as f64 / total as f64)
    }

    /// Returns the completed percent when a total is known.
    pub fn percent(&self) -> Option<f64> {
        self.fraction().map(|fraction| fraction * 100.0)
    }

    /// Validates this value.
    pub fn validate(&self) -> Result<()> {
        if let Some(total) = self.total {
            if total == 0 {
                return Err(JobError::InvalidArgument(
                    "progress total must be greater than zero".to_string(),
                ));
            }
            if self.completed > total {
                return Err(JobError::InvalidArgument(
                    "progress completed must not exceed total".to_string(),
                ));
            }
        }
        if self.unit.trim().is_empty() {
            return Err(JobError::InvalidArgument(
                "progress unit must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Serializable manifest for a job-scoped operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobManifest {
    /// Stable job identifier.
    pub job_id: JobId,
    /// Runtime operation associated with this job.
    pub operation_id: OperationId,
    /// Current lifecycle status.
    pub status: JobStatus,
    /// Latest progress value.
    pub progress: Option<JobProgress>,
    /// Diagnostics emitted by the operation.
    pub diagnostics: Vec<Diagnostic>,
    /// Artifacts produced by the operation.
    pub artifacts: Vec<ArtifactRef>,
    /// Caller or runtime metadata.
    pub metadata: serde_json::Value,
    /// Creation timestamp for lightweight manifests.
    pub created_at: Option<String>,
    /// Last update timestamp for lightweight manifests.
    pub updated_at: Option<String>,
}

impl JobManifest {
    /// Builds a manifest projection from a tracked job snapshot.
    pub fn from_snapshot(
        operation_id: OperationId,
        snapshot: JobSnapshot,
        artifacts: Vec<ArtifactRef>,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            job_id: snapshot.spec.id.clone(),
            operation_id,
            status: snapshot.status,
            progress: snapshot.progress,
            diagnostics: Vec::new(),
            artifacts,
            metadata,
            created_at: Some(snapshot.created_at.to_rfc3339()),
            updated_at: snapshot
                .finished_at
                .or(snapshot.started_at)
                .map(|timestamp| timestamp.to_rfc3339())
                .or_else(|| Some(snapshot.created_at.to_rfc3339())),
        }
    }
}

/// Serializable operation result envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult<T> {
    /// Operation value when one was produced.
    pub value: Option<T>,
    /// Diagnostics emitted during execution.
    pub diagnostics: Vec<Diagnostic>,
    /// Artifacts produced during execution.
    pub artifacts: Vec<ArtifactRef>,
}

impl<T> OperationResult<T> {
    /// Creates a result with a value and no diagnostics or artifacts.
    pub fn value(value: T) -> Self {
        Self {
            value: Some(value),
            diagnostics: Vec::new(),
            artifacts: Vec::new(),
        }
    }

    /// Creates an empty result envelope.
    pub fn empty() -> Self {
        Self {
            value: None,
            diagnostics: Vec::new(),
            artifacts: Vec::new(),
        }
    }
}

/// Serializable job result envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobResult<T> {
    /// Stable job identifier.
    pub job_id: JobId,
    /// Terminal or current job status.
    pub status: JobStatus,
    /// Operation result payload.
    pub result: OperationResult<T>,
}

/// Severity for a job log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobLogLevel {
    /// Verbose diagnostic message.
    Debug,
    /// Informational message.
    Info,
    /// Warning message.
    Warn,
    /// Error message.
    Error,
}

/// Structured log entry emitted by a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobLogEntry {
    /// Timestamp for the entry.
    pub timestamp: DateTime<Utc>,
    /// Entry severity.
    pub level: JobLogLevel,
    /// Log message.
    pub message: String,
}

impl JobLogEntry {
    /// Creates a new log entry.
    pub fn new(level: JobLogLevel, message: impl Into<String>) -> Result<Self> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(JobError::InvalidArgument(
                "log message must not be empty".to_string(),
            ));
        }
        Ok(Self {
            timestamp: Utc::now(),
            level,
            message,
        })
    }
}

/// Artifact produced by a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobArtifact {
    /// Stable artifact identifier within the job.
    pub id: String,
    /// Human-readable artifact name.
    pub name: String,
    /// Optional artifact kind, for example `image`, `report`, or `log`.
    pub kind: Option<String>,
    /// Optional local filesystem path.
    pub path: Option<PathBuf>,
    /// Optional URI for remote artifacts.
    pub uri: Option<String>,
    /// Optional media type.
    pub content_type: Option<String>,
    /// Optional byte size.
    pub bytes: Option<u64>,
    /// Caller-defined metadata.
    pub metadata: BTreeMap<String, String>,
}

impl JobArtifact {
    /// Creates a new artifact.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: None,
            path: None,
            uri: None,
            content_type: None,
            bytes: None,
            metadata: BTreeMap::new(),
        }
    }

    /// Creates a lightweight job artifact from a generic artifact reference.
    pub fn from_artifact_ref(reference: &ArtifactRef) -> Self {
        let mut artifact = Self::new(reference.id.as_str(), reference.id.as_str())
            .kind(artifact_kind_name(&reference.kind))
            .uri(reference.uri.clone())
            .content_type(reference.media_type.clone());
        artifact.bytes = reference.size_bytes;
        artifact.metadata = reference.metadata.clone();
        artifact
    }

    /// Sets the artifact kind.
    pub fn kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    /// Sets the local path.
    pub fn path(mut self, path: impl AsRef<Path>) -> Self {
        self.path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets the remote URI.
    pub fn uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    /// Sets the content type.
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Sets the byte size.
    pub fn bytes(mut self, bytes: u64) -> Self {
        self.bytes = Some(bytes);
        self
    }

    /// Adds caller-defined metadata.
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Result<Self> {
        insert_metadata(&mut self.metadata, key, value)?;
        Ok(self)
    }

    /// Validates this value.
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            return Err(JobError::InvalidArgument(
                "artifact id must not be empty".to_string(),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(JobError::InvalidArgument(
                "artifact name must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Change kind recorded in a job event stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobEventKind {
    /// Lifecycle status changed.
    StatusChanged {
        /// New status.
        status: JobStatus,
        /// Optional status message.
        message: Option<String>,
    },
    /// Progress was updated.
    Progress(JobProgress),
    /// Log entry was appended.
    Log(JobLogEntry),
    /// Artifact was appended.
    Artifact(JobArtifact),
    /// Metadata was added or replaced.
    Metadata { key: String, value: String },
}

/// Ordered event emitted for a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobEvent {
    /// Job identifier.
    pub job_id: JobId,
    /// Monotonic tracker-local sequence number.
    pub sequence: u64,
    /// Event timestamp.
    pub timestamp: DateTime<Utc>,
    /// Event payload.
    pub kind: JobEventKind,
}

/// Filter used to replay a subset of a job event stream.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobEventFilter {
    /// Return events after this sequence number.
    pub after_sequence: Option<u64>,
    /// Optional event kind name such as `status`, `progress`, `log`, `artifact`, or `metadata`.
    pub kind: Option<String>,
}

/// Snapshot of tracked job state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSnapshot {
    /// Job definition.
    pub spec: JobSpec,
    /// Current status.
    pub status: JobStatus,
    /// Latest progress value.
    pub progress: Option<JobProgress>,
    /// Recent log entries retained by this tracker.
    pub logs: Vec<JobLogEntry>,
    /// Artifact records produced by this job.
    pub artifacts: Vec<JobArtifact>,
    /// Job creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Job start timestamp.
    pub started_at: Option<DateTime<Utc>>,
    /// Terminal timestamp.
    pub finished_at: Option<DateTime<Utc>>,
    /// Failure details when status is failed.
    pub failure: Option<JobFailure>,
    /// Mutable caller-defined metadata.
    pub metadata: BTreeMap<String, String>,
}

/// Cooperative cancellation token.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a new token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Returns `JobError::Cancelled` when cancellation has been requested.
    pub fn check_cancelled(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(JobError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
struct JobRecord {
    snapshot: JobSnapshot,
    events: Vec<JobEvent>,
}

#[derive(Debug, Default)]
struct JobTrackerInner {
    jobs: BTreeMap<JobId, JobRecord>,
    next_sequence: u64,
}

/// In-memory tracker for job snapshots and event history.
#[derive(Debug, Clone, Default)]
pub struct JobTracker {
    inner: Arc<Mutex<JobTrackerInner>>,
}

impl JobTracker {
    /// Creates an empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates and tracks a queued job.
    pub fn create(&self, spec: JobSpec) -> Result<JobHandle> {
        let mut inner = self.lock()?;
        if inner.jobs.contains_key(&spec.id) {
            return Err(JobError::InvalidArgument(format!(
                "job `{}` already exists",
                spec.id
            )));
        }

        let token = CancellationToken::new();
        let now = Utc::now();
        let snapshot = JobSnapshot {
            metadata: spec.metadata.clone(),
            spec: spec.clone(),
            status: JobStatus::Queued,
            progress: None,
            logs: Vec::new(),
            artifacts: Vec::new(),
            created_at: now,
            started_at: None,
            finished_at: None,
            failure: None,
        };
        let event = inner.new_event(
            spec.id.clone(),
            JobEventKind::StatusChanged {
                status: JobStatus::Queued,
                message: None,
            },
        );
        inner.jobs.insert(
            spec.id.clone(),
            JobRecord {
                snapshot,
                events: vec![event],
            },
        );

        Ok(JobHandle {
            id: spec.id,
            token,
            tracker: self.clone(),
        })
    }

    /// Returns a snapshot for a job.
    pub fn snapshot(&self, id: &JobId) -> Result<Option<JobSnapshot>> {
        Ok(self
            .lock()?
            .jobs
            .get(id)
            .map(|record| record.snapshot.clone()))
    }

    /// Lists all tracked job snapshots.
    pub fn snapshots(&self) -> Result<Vec<JobSnapshot>> {
        Ok(self
            .lock()?
            .jobs
            .values()
            .map(|record| record.snapshot.clone())
            .collect())
    }

    /// Lists snapshots matching a status, kind, and metadata filter.
    pub fn query_snapshots(&self, filter: JobListFilter) -> Result<Vec<JobSnapshot>> {
        Ok(self
            .lock()?
            .jobs
            .values()
            .filter(|record| snapshot_matches_filter(&record.snapshot, &filter))
            .map(|record| record.snapshot.clone())
            .collect())
    }

    /// Returns the ordered event history for a job.
    pub fn events(&self, id: &JobId) -> Result<Vec<JobEvent>> {
        Ok(self
            .lock()?
            .jobs
            .get(id)
            .map(|record| record.events.clone())
            .unwrap_or_default())
    }

    /// Returns a filtered ordered event history for a job.
    pub fn events_filtered(&self, id: &JobId, filter: JobEventFilter) -> Result<Vec<JobEvent>> {
        Ok(self
            .events(id)?
            .into_iter()
            .filter(|event| {
                filter
                    .after_sequence
                    .is_none_or(|sequence| event.sequence > sequence)
            })
            .filter(|event| {
                filter
                    .kind
                    .as_deref()
                    .is_none_or(|kind| event_kind_name(&event.kind) == kind)
            })
            .collect())
    }

    fn emit(&self, id: &JobId, kind: JobEventKind) -> Result<()> {
        let mut inner = self.lock()?;
        let event = inner.new_event(id.clone(), kind);
        let record = inner
            .jobs
            .get_mut(id)
            .ok_or_else(|| JobError::InvalidArgument(format!("unknown job `{id}`")))?;
        apply_event(&mut record.snapshot, &event.kind, event.timestamp)?;
        record.events.push(event);
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, JobTrackerInner>> {
        self.inner
            .lock()
            .map_err(|_| JobError::StateUnavailable("job tracker lock poisoned".to_string()))
    }
}

impl JobTrackerInner {
    fn new_event(&mut self, job_id: JobId, kind: JobEventKind) -> JobEvent {
        self.next_sequence += 1;
        JobEvent {
            job_id,
            sequence: self.next_sequence,
            timestamp: Utc::now(),
            kind,
        }
    }
}

/// Handle for controlling one tracked job.
#[derive(Debug, Clone)]
pub struct JobHandle {
    id: JobId,
    token: CancellationToken,
    tracker: JobTracker,
}

impl JobHandle {
    /// Returns the job id.
    pub fn id(&self) -> &JobId {
        &self.id
    }

    /// Returns the cancellation token.
    pub fn cancellation_token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// Creates a reporting context for worker code.
    pub fn context(&self) -> JobContext {
        JobContext {
            id: self.id.clone(),
            token: self.token.clone(),
            tracker: self.tracker.clone(),
        }
    }

    /// Requests cancellation for this job.
    pub fn request_cancel(&self) -> Result<()> {
        self.token.cancel();
        let status = self
            .tracker
            .snapshot(&self.id)?
            .map(|snapshot| snapshot.status);
        if matches!(status, Some(status) if !status.is_terminal()) {
            self.tracker.emit(
                &self.id,
                JobEventKind::StatusChanged {
                    status: JobStatus::Cancelling,
                    message: Some("cancellation requested".to_string()),
                },
            )?;
        }
        Ok(())
    }

    /// Marks this job as running.
    pub fn mark_running(&self) -> Result<()> {
        self.tracker.emit(
            &self.id,
            JobEventKind::StatusChanged {
                status: JobStatus::Running,
                message: None,
            },
        )
    }

    /// Marks this job as succeeded.
    pub fn mark_succeeded(&self) -> Result<()> {
        self.tracker.emit(
            &self.id,
            JobEventKind::StatusChanged {
                status: JobStatus::Succeeded,
                message: None,
            },
        )
    }

    /// Marks this job as cancelled.
    pub fn mark_cancelled(&self, message: Option<String>) -> Result<()> {
        self.tracker.emit(
            &self.id,
            JobEventKind::StatusChanged {
                status: JobStatus::Cancelled,
                message,
            },
        )
    }

    /// Marks this job as failed.
    pub fn mark_failed(&self, message: impl Into<String>) -> Result<()> {
        self.tracker.emit(
            &self.id,
            JobEventKind::StatusChanged {
                status: JobStatus::Failed,
                message: Some(message.into()),
            },
        )
    }
}

/// Reporting context passed to worker code.
#[derive(Debug, Clone)]
pub struct JobContext {
    id: JobId,
    token: CancellationToken,
    tracker: JobTracker,
}

impl JobContext {
    /// Returns the job id.
    pub fn id(&self) -> &JobId {
        &self.id
    }

    /// Returns whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Returns `JobError::Cancelled` when cancellation has been requested.
    pub fn check_cancelled(&self) -> Result<()> {
        self.token.check_cancelled()
    }

    /// Updates progress.
    pub fn progress(&self, progress: JobProgress) -> Result<()> {
        progress.validate()?;
        self.tracker
            .emit(&self.id, JobEventKind::Progress(progress))
    }

    /// Appends a log entry.
    pub fn log(&self, level: JobLogLevel, message: impl Into<String>) -> Result<()> {
        self.tracker.emit(
            &self.id,
            JobEventKind::Log(JobLogEntry::new(level, message)?),
        )
    }

    /// Appends a debug log entry.
    pub fn debug(&self, message: impl Into<String>) -> Result<()> {
        self.log(JobLogLevel::Debug, message)
    }

    /// Appends an informational log entry.
    pub fn info(&self, message: impl Into<String>) -> Result<()> {
        self.log(JobLogLevel::Info, message)
    }

    /// Appends a warning log entry.
    pub fn warn(&self, message: impl Into<String>) -> Result<()> {
        self.log(JobLogLevel::Warn, message)
    }

    /// Appends an error log entry.
    pub fn error(&self, message: impl Into<String>) -> Result<()> {
        self.log(JobLogLevel::Error, message)
    }

    /// Appends an artifact record.
    pub fn artifact(&self, artifact: JobArtifact) -> Result<()> {
        artifact.validate()?;
        self.tracker
            .emit(&self.id, JobEventKind::Artifact(artifact))
    }

    /// Adds or replaces metadata.
    pub fn metadata(&self, key: impl Into<String>, value: impl Into<String>) -> Result<()> {
        let mut metadata = BTreeMap::new();
        insert_metadata(&mut metadata, key, value)?;
        let (key, value) = metadata.into_iter().next().expect("metadata was inserted");
        self.tracker
            .emit(&self.id, JobEventKind::Metadata { key, value })
    }
}

/// Std-thread backed runner for simple background execution.
#[derive(Debug, Clone, Default)]
pub struct BackgroundJobRunner {
    tracker: JobTracker,
}

impl BackgroundJobRunner {
    /// Creates a runner using the supplied tracker.
    pub fn new(tracker: JobTracker) -> Self {
        Self { tracker }
    }

    /// Returns the backing tracker.
    pub fn tracker(&self) -> JobTracker {
        self.tracker.clone()
    }

    /// Creates a job and runs it on a new thread.
    pub fn spawn<F>(&self, spec: JobSpec, run: F) -> Result<JobJoinHandle>
    where
        F: FnOnce(JobContext) -> Result<()> + Send + 'static,
    {
        let job = self.tracker.create(spec)?;
        let thread_job = job.clone();
        let join = thread::spawn(move || {
            thread_job.mark_running()?;
            let context = thread_job.context();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(context)));
            match result {
                Ok(Ok(())) => {
                    thread_job.mark_succeeded()?;
                    Ok(())
                }
                Ok(Err(JobError::Cancelled)) => {
                    let _ = thread_job.mark_cancelled(Some("cancelled".to_string()));
                    Err(JobError::Cancelled)
                }
                Ok(Err(error)) => {
                    let _ = thread_job.mark_failed(error.failure_message());
                    Err(error)
                }
                Err(_) => {
                    let message = "job panicked".to_string();
                    let _ = thread_job.mark_failed(message.clone());
                    Err(JobError::Failed(message))
                }
            }
        });

        Ok(JobJoinHandle {
            job,
            join: Some(join),
        })
    }
}

/// Join handle returned by `BackgroundJobRunner`.
#[derive(Debug)]
pub struct JobJoinHandle {
    job: JobHandle,
    join: Option<JoinHandle<Result<()>>>,
}

impl JobJoinHandle {
    /// Returns the job id.
    pub fn id(&self) -> &JobId {
        self.job.id()
    }

    /// Returns the tracked job handle.
    pub fn job(&self) -> &JobHandle {
        &self.job
    }

    /// Requests cancellation.
    pub fn request_cancel(&self) -> Result<()> {
        self.job.request_cancel()
    }

    /// Waits for the worker thread to finish.
    pub fn join(&mut self) -> Result<()> {
        let join = self
            .join
            .take()
            .ok_or_else(|| JobError::InvalidArgument("job already joined".to_string()))?;
        join.join()
            .map_err(|_| JobError::Failed("job thread panicked".to_string()))?
    }
}

fn apply_event(
    snapshot: &mut JobSnapshot,
    kind: &JobEventKind,
    timestamp: DateTime<Utc>,
) -> Result<()> {
    match kind {
        JobEventKind::StatusChanged { status, message } => {
            snapshot.status = *status;
            match status {
                JobStatus::Running => {
                    snapshot.started_at.get_or_insert(timestamp);
                }
                JobStatus::Failed => {
                    snapshot.finished_at.get_or_insert(timestamp);
                    snapshot.failure = Some(JobFailure {
                        message: message.clone().unwrap_or_else(|| "job failed".to_string()),
                    });
                }
                JobStatus::Succeeded | JobStatus::Cancelled => {
                    snapshot.finished_at.get_or_insert(timestamp);
                }
                JobStatus::Queued | JobStatus::Cancelling => {}
            }
        }
        JobEventKind::Progress(progress) => {
            progress.validate()?;
            snapshot.progress = Some(progress.clone());
        }
        JobEventKind::Log(entry) => {
            snapshot.logs.push(entry.clone());
        }
        JobEventKind::Artifact(artifact) => {
            artifact.validate()?;
            snapshot.artifacts.push(artifact.clone());
        }
        JobEventKind::Metadata { key, value } => {
            if key.trim().is_empty() {
                return Err(JobError::InvalidArgument(
                    "metadata key must not be empty".to_string(),
                ));
            }
            snapshot.metadata.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}

fn snapshot_matches_filter(snapshot: &JobSnapshot, filter: &JobListFilter) -> bool {
    if filter
        .status
        .is_some_and(|status| snapshot.status != status)
    {
        return false;
    }
    if filter
        .kind
        .as_deref()
        .is_some_and(|kind| snapshot.spec.kind.as_deref() != Some(kind))
    {
        return false;
    }
    filter
        .metadata
        .iter()
        .all(|(key, value)| snapshot.metadata.get(key) == Some(value))
}

fn event_kind_name(kind: &JobEventKind) -> &'static str {
    match kind {
        JobEventKind::StatusChanged { .. } => "status",
        JobEventKind::Progress(_) => "progress",
        JobEventKind::Log(_) => "log",
        JobEventKind::Artifact(_) => "artifact",
        JobEventKind::Metadata { .. } => "metadata",
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

fn insert_metadata(
    metadata: &mut BTreeMap<String, String>,
    key: impl Into<String>,
    value: impl Into<String>,
) -> Result<()> {
    let key = key.into();
    if key.trim().is_empty() {
        return Err(JobError::InvalidArgument(
            "metadata key must not be empty".to_string(),
        ));
    }
    metadata.insert(key, value.into());
    Ok(())
}

impl JobError {
    fn failure_message(&self) -> String {
        match self {
            Self::InvalidArgument(message)
            | Self::NotFound(message)
            | Self::InvalidUri(message)
            | Self::Io(message)
            | Self::Failed(message)
            | Self::StateUnavailable(message) => message.clone(),
            Self::Cancelled => "cancelled".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn tracker_captures_progress_logs_and_artifacts() {
        let tracker = JobTracker::new();
        let spec = JobSpec::new("render-001", "Render preview")
            .unwrap()
            .with_kind("render")
            .unwrap()
            .with_metadata("project", "demo")
            .unwrap();
        let job = tracker.create(spec).unwrap();

        job.mark_running().unwrap();
        let context = job.context();
        context
            .progress(
                JobProgress::new(2, Some(4))
                    .unwrap()
                    .unit("frames")
                    .unwrap()
                    .message("halfway"),
            )
            .unwrap();
        context.info("rendered preview frame").unwrap();
        context
            .artifact(
                JobArtifact::new("preview", "Preview image")
                    .kind("image")
                    .path("out/preview.png")
                    .content_type("image/png"),
            )
            .unwrap();
        context.metadata("priority", "normal").unwrap();
        job.mark_succeeded().unwrap();

        let snapshot = tracker.snapshot(job.id()).unwrap().unwrap();
        assert_eq!(snapshot.status, JobStatus::Succeeded);
        assert_eq!(snapshot.progress.unwrap().percent(), Some(50.0));
        assert_eq!(snapshot.logs.len(), 1);
        assert_eq!(snapshot.artifacts.len(), 1);
        assert_eq!(snapshot.metadata["project"], "demo");
        assert_eq!(snapshot.metadata["priority"], "normal");
        assert!(snapshot.started_at.is_some());
        assert!(snapshot.finished_at.is_some());

        let events = tracker.events(job.id()).unwrap();
        assert!(events.len() >= 6);
        assert!(events
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence));
    }

    #[test]
    fn runner_records_failures() {
        let runner = BackgroundJobRunner::default();
        let mut handle = runner
            .spawn(JobSpec::new("fail-001", "Fail").unwrap(), |_context| {
                Err(JobError::Failed("boom".to_string()))
            })
            .unwrap();

        assert_eq!(handle.join(), Err(JobError::Failed("boom".to_string())));
        let snapshot = runner.tracker().snapshot(handle.id()).unwrap().unwrap();
        assert_eq!(snapshot.status, JobStatus::Failed);
        assert_eq!(snapshot.failure.unwrap().message, "boom");
    }

    #[test]
    fn runner_supports_cooperative_cancellation() {
        let runner = BackgroundJobRunner::default();
        let mut handle = runner
            .spawn(JobSpec::new("cancel-001", "Cancel").unwrap(), |context| {
                while !context.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                context.check_cancelled()
            })
            .unwrap();

        handle.request_cancel().unwrap();
        assert_eq!(handle.join(), Err(JobError::Cancelled));
        let snapshot = runner.tracker().snapshot(handle.id()).unwrap().unwrap();
        assert_eq!(snapshot.status, JobStatus::Cancelled);
    }

    #[test]
    fn progress_rejects_completed_above_total() {
        assert!(JobProgress::new(5, Some(4)).is_err());
    }

    #[test]
    fn operation_result_serializes_contract_fields() {
        let result = OperationResult::value(42_u32);
        let json = serde_json::to_string(&result).expect("serialize result");

        assert!(json.contains("\"value\":42"));
        assert!(json.contains("\"diagnostics\":[]"));
        assert!(json.contains("\"artifacts\":[]"));
    }

    #[test]
    fn job_manifest_uses_camel_case_json() {
        let manifest = JobManifest {
            job_id: JobId::new("job-1").unwrap(),
            operation_id: OperationId::new("describe"),
            status: JobStatus::Succeeded,
            progress: Some(JobProgress::new(1, Some(1)).unwrap()),
            diagnostics: Vec::new(),
            artifacts: Vec::new(),
            metadata: serde_json::json!({"kind": "test"}),
            created_at: Some("unix:1".to_string()),
            updated_at: Some("unix:2".to_string()),
        };

        let json = serde_json::to_string(&manifest).expect("serialize manifest");

        assert!(json.contains("\"jobId\":\"job-1\""));
        assert!(json.contains("\"operationId\":\"describe\""));
        assert!(json.contains("\"status\":\"succeeded\""));
        assert!(json.contains("\"createdAt\":\"unix:1\""));
    }
}
