from pathlib import Path

path = Path("crates/jobs/jobs-core/src/lib.rs")
text = path.read_text()

replacements = [
    (
        '''    fn emit(&self, id: &JobId, kind: JobEventKind) -> Result<()> {
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
''',
        '''    fn emit(&self, id: &JobId, kind: JobEventKind) -> Result<()> {
        self.lock()?.emit(id, kind)
    }

    fn request_cancel(&self, id: &JobId) -> Result<()> {
        let mut inner = self.lock()?;
        let status = inner
            .jobs
            .get(id)
            .ok_or_else(|| JobError::InvalidArgument(format!("unknown job `{id}`")))?
            .snapshot
            .status;

        match status {
            JobStatus::Queued => inner.emit(
                id,
                JobEventKind::StatusChanged {
                    status: JobStatus::Cancelled,
                    message: Some("cancelled before start".to_string()),
                },
            ),
            JobStatus::Running => inner.emit(
                id,
                JobEventKind::StatusChanged {
                    status: JobStatus::Cancelling,
                    message: Some("cancellation requested".to_string()),
                },
            ),
            JobStatus::Cancelling
            | JobStatus::Succeeded
            | JobStatus::Failed
            | JobStatus::Cancelled => Ok(()),
        }
    }

    fn mark_running(&self, id: &JobId, token: &CancellationToken) -> Result<()> {
        let mut inner = self.lock()?;
        let status = inner
            .jobs
            .get(id)
            .ok_or_else(|| JobError::InvalidArgument(format!("unknown job `{id}`")))?
            .snapshot
            .status;

        match status {
            JobStatus::Queued if token.is_cancelled() => {
                inner.emit(
                    id,
                    JobEventKind::StatusChanged {
                        status: JobStatus::Cancelled,
                        message: Some("cancelled before start".to_string()),
                    },
                )?;
                Err(JobError::Cancelled)
            }
            JobStatus::Queued => inner.emit(
                id,
                JobEventKind::StatusChanged {
                    status: JobStatus::Running,
                    message: None,
                },
            ),
            JobStatus::Cancelling => {
                inner.emit(
                    id,
                    JobEventKind::StatusChanged {
                        status: JobStatus::Cancelled,
                        message: Some("cancelled before start".to_string()),
                    },
                )?;
                Err(JobError::Cancelled)
            }
            JobStatus::Cancelled => Err(JobError::Cancelled),
            JobStatus::Running => Ok(()),
            JobStatus::Succeeded | JobStatus::Failed => Err(JobError::InvalidArgument(format!(
                "cannot start terminal job `{id}`"
            ))),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, JobTrackerInner>> {
''',
    ),
    (
        '''impl JobTrackerInner {
    fn new_event(&mut self, job_id: JobId, kind: JobEventKind) -> JobEvent {
''',
        '''impl JobTrackerInner {
    fn emit(&mut self, id: &JobId, kind: JobEventKind) -> Result<()> {
        let event = self.new_event(id.clone(), kind);
        let record = self
            .jobs
            .get_mut(id)
            .ok_or_else(|| JobError::InvalidArgument(format!("unknown job `{id}`")))?;
        apply_event(&mut record.snapshot, &event.kind, event.timestamp)?;
        record.events.push(event);
        Ok(())
    }

    fn new_event(&mut self, job_id: JobId, kind: JobEventKind) -> JobEvent {
''',
    ),
    (
        '''    /// Requests cancellation for this job.
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
''',
        '''    /// Requests cancellation for this job.
    pub fn request_cancel(&self) -> Result<()> {
        self.token.cancel();
        self.tracker.request_cancel(&self.id)
    }

    /// Marks this job as running.
    pub fn mark_running(&self) -> Result<()> {
        self.tracker.mark_running(&self.id, &self.token)
    }
''',
    ),
    (
        '''mod tests {
    use std::time::Duration;

    use super::*;
''',
        '''mod tests {
    use std::sync::{Arc, Barrier};

    use super::*;
''',
    ),
    (
        '''    #[test]
    fn tracker_captures_progress_logs_and_artifacts() {
''',
        '''    #[test]
    fn cancelling_queued_job_is_terminal_before_start() {
        let tracker = JobTracker::new();
        let job = tracker
            .create(JobSpec::new("queued-cancel", "Queued cancellation").unwrap())
            .unwrap();

        job.request_cancel().unwrap();

        let snapshot = tracker.snapshot(job.id()).unwrap().unwrap();
        assert_eq!(snapshot.status, JobStatus::Cancelled);
        assert!(snapshot.started_at.is_none());
        assert!(snapshot.finished_at.is_some());
        assert_eq!(job.mark_running(), Err(JobError::Cancelled));
        assert_eq!(
            tracker.snapshot(job.id()).unwrap().unwrap().status,
            JobStatus::Cancelled
        );

        let statuses = tracker
            .events(job.id())
            .unwrap()
            .into_iter()
            .filter_map(|event| match event.kind {
                JobEventKind::StatusChanged { status, .. } => Some(status),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(statuses, vec![JobStatus::Queued, JobStatus::Cancelled]);
    }

    #[test]
    fn tracker_captures_progress_logs_and_artifacts() {
''',
    ),
    (
        '''    #[test]
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
''',
        '''    #[test]
    fn runner_supports_cooperative_cancellation() {
        let runner = BackgroundJobRunner::default();
        let entered_worker = Arc::new(Barrier::new(2));
        let release_worker = Arc::new(Barrier::new(2));
        let worker_entered = entered_worker.clone();
        let worker_release = release_worker.clone();
        let mut handle = runner
            .spawn(
                JobSpec::new("cancel-001", "Cancel").unwrap(),
                move |context| {
                    worker_entered.wait();
                    worker_release.wait();
                    context.check_cancelled()
                },
            )
            .unwrap();

        entered_worker.wait();
        handle.request_cancel().unwrap();
        assert_eq!(
            runner.tracker().snapshot(handle.id()).unwrap().unwrap().status,
            JobStatus::Cancelling
        );
        release_worker.wait();
        assert_eq!(handle.join(), Err(JobError::Cancelled));
        let snapshot = runner.tracker().snapshot(handle.id()).unwrap().unwrap();
        assert_eq!(snapshot.status, JobStatus::Cancelled);
    }
''',
    ),
]

for old, new in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one exact replacement, found {count}")
    text = text.replace(old, new, 1)

path.write_text(text)
