# jobs-core

Reusable long-running job state and generic artifact handling for Rust
applications.

## Highlights

- Stable job ids and metadata for cross-process APIs
- Status transitions for queued, running, cancelling, succeeded, failed, and cancelled jobs
- Cooperative cancellation token that can be shared with worker code
- Progress snapshots with optional totals and percent calculation
- Structured logs and artifact records
- Generic `OperationResult<T>`, `JobResult<T>`, and `JobManifest` envelopes
- Generic `ArtifactRef` records, memory/local artifact stores, SHA-256
  validation, and downloader/validator traits
- In-memory tracker and std-thread runner for small services, CLIs, and tests

## Runtime Surface

- `jobs.spec` validates and normalizes `JobSpec` input.
- `jobs.progress` validates progress and returns fraction/percent.
- `jobs.lifecycle` applies a short in-memory lifecycle script without spawning
  background threads.
- `jobs.manifest` builds deterministic `JobManifest` values from inline spec,
  progress, artifact, and metadata input.
- `jobs.events` replays and filters a short inline lifecycle script into compact
  ordered events.
- `jobs.artifactValidate` checks inline artifact metadata, size, media type, and
  checksum expectations without reading files.

Surface responses keep the operation-specific fields at the top level and add
the shared `operation`, `title`, `message`, `summary`, and `result` fields for
generic package UIs. Invalid requests, unknown operations, unsupported lifecycle
steps, artifact mismatches, and lifecycle scripts longer than 32 steps return
typed `runtime_core::SurfaceError` JSON. Runtime execution metadata is exposed
through `xExecutionPlan` schema extensions.

## Example

```rust,no_run
use jobs_core::{BackgroundJobRunner, JobArtifact, JobProgress, JobSpec, Result};

fn main() -> Result<()> {
    let runner = BackgroundJobRunner::default();
    let spec = JobSpec::new("render-001", "Render preview")?.with_kind("render")?;

    let mut handle = runner.spawn(spec, |ctx| {
        ctx.info("started render")?;
        ctx.progress(JobProgress::new(1, Some(2))?.message("drawing"))?;
        ctx.check_cancelled()?;
        ctx.artifact(JobArtifact::new("preview", "Preview image").path("out/preview.png"))?;
        Ok(())
    })?;

    handle.join()?;
    let snapshot = runner.tracker().snapshot(handle.id())?.unwrap();
    assert!(snapshot.status.is_terminal());
    Ok(())
}
```

## Related crates

- `video-analysis-core`
- `video-analysis-storage`
