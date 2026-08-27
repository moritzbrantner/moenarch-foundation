# media-core

Neutral media contracts shared across audio, video, text, and future media
consumers.

The crate owns only:

- `Timebase`, the rational duration of one timestamp tick;
- `Timestamp`, presentation ticks paired with their timebase;
- `AnalysisEvent`, a domain-neutral labeled result with optional time and score;
- `MediaSourceRef`, source identity/URI metadata without source implementation;
- `MediaTimeRange`, a validated finite interval in media seconds;
- `TimedTextContract` and its segment/word/character DTOs for text located on a
  media timeline without NLP behavior;
- `PixelFormat` and `AudioSampleFormat`, compact stream-format identifiers
  without frame or buffer ownership;
- `DetectError` and `Result`, the shared media error identity used across
  foundation and capability contracts.

`moenarch-video-analysis-core` re-exports the original neutral types to preserve
its existing public API and type identity while consumers migrate.

## Ownership boundary

Media data stays in its narrowest domain:

- video frame and pixel-buffer contracts remain in visual/video packages;
- audio buffer and audio-frame contracts remain in audio packages;
- image contracts remain in image packages;
- NLP transcript documents, text-document conversion, SRT/WebVTT/Whisper
  parsing and formatting, linguistic analysis, and text model behavior remain
  in `nlp-stack`;
- detection algorithms and model-lifecycle behavior remain with their current
  domain or foundation owners.

Timed text is intentionally narrower than an NLP transcript document. It is an
interchange DTO for producers and consumers that need text plus media timing,
optional speaker/language/confidence facts, source identity, and opaque
attributes without depending on a text-processing implementation.

The original issue #108 extraction did not invent a cross-family range or
transcript contract. `MediaSourceRef`, `MediaTimeRange`, and the timed-text DTOs
are destination-owned post-extraction architecture evolution documented by ADR
0013. They do not change the clean-copy provenance of the original extraction.

## Candidate consumer audit

The issue #108 audit inspected the then-current default-branch heads of the
named candidate consumers. `video-analysis-studio` was the only candidate that
imported the original neutral Rust types directly:

| Consumer | Audited commit | Neutral contract use |
| --- | --- | --- |
| `geo-analysis` | `804f802f1459a7b1d0359cc805235715a5419b78` | none |
| `native-whisperx` | `b0ba12342fbb36b057fbe620f62d52c4fde0b36d` | none; its `video-analysis-core` use was error/domain behavior |
| `media-similarity` | `d015b36187a9c3ebd202f81175081608fb307aa3` | none; its imports were frame, scene, detection, and source contracts |
| `youtube-corpus` | `8ab21570348e7d636685a51f110f11fc2eacf363` | none |
| `video-analysis-studio` | `93ceeb1c43764be9d31c35258145604559e0a0aa` | `AnalysisEvent`, `Timebase`, and `Timestamp` |
| `stutter-tracker` | `6c68b7a343ac8470405a79f240263f9e8ca7af80` | none; its imports were video-owned errors |
| `viz-engine` | `29b85cf331701f66a796b89b5263faacf3d8998c` | none; its import was a video-owned error |

A candidate `video-analysis-studio` patch adds `moenarch-media-core`, moves
only those original three imports to `media_core`, and leaves video-domain
imports on `video_analysis_core`. Its diff hash is
`40da0d918ab91c6ed2193219f3dc5983aeb1d86866c27e0a6186e9984cb55cd7`;
`git diff --check` and standalone Rust formatting checks passed during the
original audit. The exact-type compatibility test in this repository proves
that patch remains optional until the consumer migration is scheduled.

## Release candidate consumer gate

`release/check_candidate_consumer.sh` reconstructs the applicable consumer in
an OS-temporary sibling checkout. It pins `video-analysis-studio` to
`93ceeb1c43764be9d31c35258145604559e0a0aa` and the compatible pre-extraction
`rust-packages` baseline to `c11c945fc13e532588f768f982c3c80a46ab477c`,
overlays this exact media-core package plus the reviewed #108 compatibility
re-export, applies the studio import patch, updates only the temporary lockfile,
and requires `cargo check -p studio-core --locked` to pass. Release fixtures are
excluded from the published crate archive. Scratch checkouts honor
`MOENARCH_RELEASE_SCRATCH_ROOT`, then `TMPDIR`, and otherwise use the ignored
workspace `target` directory so the gate does not depend on `/tmp` capacity.
