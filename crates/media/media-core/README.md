# media-core

Neutral media contracts shared across audio, video, text, and future media
consumers.

The crate owns only:

- `Timebase`, the rational duration of one timestamp tick;
- `Timestamp`, presentation ticks paired with their timebase;
- `AnalysisEvent`, a domain-neutral labeled result with optional time and score;
- `PixelFormat` and `AudioSampleFormat`, compact stream-format identifiers
  without frame or buffer ownership;
- `DetectError` and `Result`, the shared media error identity used across
  foundation and capability contracts.

`moenarch-video-analysis-core` re-exports these exact types to preserve its
existing public API and type identity while consumers migrate.

## Ownership audit

The issue #108 contract audit kept media data in its narrowest existing domain:

- video frame and pixel-buffer contracts remain in `video-analysis-core`;
- audio buffer and audio-frame contracts remain in `audio-analysis-core`;
- image contracts remain in `image-analysis-core`;
- transcript and text contracts remain in their text crates;
- source and stream metadata remain in `video-analysis-ingest`;
- detection algorithms and model-lifecycle contracts remain with their current
  domain or foundation owners.

The format enums are metadata identifiers used by error and stream contracts;
they do not own media data. Moving the shared error identity alongside those
identifiers lets non-visual foundation crates stop depending on video ownership
without copying error DTOs or changing downstream result types.

No cross-family range contract existed at the audited source head, so none was
invented for this extraction. No alternate source or stream metadata type is
introduced here.

## Candidate consumer audit

The issue #108 audit inspected the current default-branch heads of the named
candidate consumers. `video-analysis-studio` is the only candidate that imports
these neutral Rust types directly:

| Consumer | Audited commit | Neutral contract use |
| --- | --- | --- |
| `geo-analysis` | `804f802f1459a7b1d0359cc805235715a5419b78` | none |
| `native-whisperx` | `b0ba12342fbb36b057fbe620f62d52c4fde0b36d` | none; its `video-analysis-core` use is error/domain behavior |
| `media-similarity` | `d015b36187a9c3ebd202f81175081608fb307aa3` | none; its imports are frame, scene, detection, and source contracts |
| `youtube-corpus` | `8ab21570348e7d636685a51f110f11fc2eacf363` | none |
| `video-analysis-studio` | `93ceeb1c43764be9d31c35258145604559e0a0aa` | `AnalysisEvent`, `Timebase`, and `Timestamp` |
| `stutter-tracker` | `6c68b7a343ac8470405a79f240263f9e8ca7af80` | none; its imports are video-owned errors |
| `viz-engine` | `29b85cf331701f66a796b89b5263faacf3d8998c` | none; its import is a video-owned error |

A candidate `video-analysis-studio` patch adds `moenarch-media-core`, moves
only those three imports to `media_core`, and leaves video-domain imports on
`video_analysis_core`. Its diff hash is
`40da0d918ab91c6ed2193219f3dc5983aeb1d86866c27e0a6186e9984cb55cd7`;
`git diff --check` and standalone Rust formatting checks pass. The exact-type
compatibility test in this repository proves that the patch is optional until
the consumer migration is scheduled.
