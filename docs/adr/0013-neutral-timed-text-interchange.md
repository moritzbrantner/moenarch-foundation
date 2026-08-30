# ADR 0013: Neutral Timed-Text Interchange

## Status

Accepted.

This ADR refines the narrow NLP-contract exception in ADR 0012. It moves only
deterministic, format-only timed-text parsing and rendering into foundation; it
does not move NLP parsing, transcript enrichment, product output policy, or
model behavior.

## Context

Audio transcription and visual/media consumers currently exchange transcript-shaped data through `nlp-stack`. That makes domain implementations depend sideways on NLP even when they only need text, timing, speaker labels, confidence, and source identity.

The result is unnecessary coordinated source development: audio and visual capability repositories must track NLP revisions to exchange media-timeline data.

## Decision

`moenarch-media-core` owns a small domain-neutral interchange surface:

- `MediaSourceRef` for source identity and URI metadata;
- `MediaTimeRange` for finite start/end seconds;
- `TimedTextContract` and segment/word/character DTOs for text located on a media timeline.
- deterministic SRT, WebVTT, plain-text, TSV, and Audacity-label projections
  over that contract.

These types may carry language tags, speaker labels, confidence values, finality, and opaque string attributes because those are producer/consumer interchange facts. They do not define linguistic interpretation.

Foundation's projections preserve generic format syntax only. Timed output
requires explicit complete ranges and never invents timing. Speaker styling,
word highlighting, wrapping, language-specific joining, and provider defaults
remain in product-owned mapping layers. Audacity label output is text-only;
speaker markers such as `[[speaker]]` are product conventions rather than label
format syntax.

Subtitle parsing is intentionally a canonical projection rather than a
lossless document representation. Numeric SRT cue identifiers become segment
indices. WebVTT string identifiers, cue settings, and `NOTE`, `STYLE`, and
`REGION` metadata are syntax-level inputs that are validated and discarded.
`NOTE` comment bodies and `STYLE` CSS bodies remain opaque after container and
forbidden-arrow validation; foundation does not interpret comments or CSS. The
neutral contract retains cue text and timing.

`nlp-stack` or the relevant capability continues to own:

- transcript document semantics beyond the neutral interchange DTO;
- text-document conversion and annotation;
- Whisper/WhisperX and other provider-specific formats;
- transcript heuristics and linguistic analysis;
- text model runtimes and NLP-specific validation/enrichment.

`audio-analysis` and other media producers should emit the neutral foundation contract at their public domain boundary. NLP consumers may convert that contract into their richer transcript document when NLP behavior is selected.

The intended graph becomes:

```text
                  moenarch-foundation
                 /        |          \
                /         |           \
       audio-analysis  nlp-stack  visual-analysis
                \         |           /
                 \        |          /
                  application/adapters
```

A dedicated cross-domain adapter may depend on both domain repositories when it adds real behavior. Foundation must never depend upward on those adapters or capability repositories.

## Compatibility

This is additive source development in the existing `moenarch-media-core` crate. It does not authorize a version bump or publication. Existing NLP transcript contracts remain available while consumers migrate.

Source-mode consumers may validate the new surface against an exact foundation revision. Registry-only release proof remains a separate release task.

## Consequences

- Audio and visual repositories no longer need NLP solely for transcript-shaped interchange.
- NLP retains ownership of actual text/transcript processing behavior.
- Applications can compose audio, visual, and NLP independently.
- The neutral contract must remain intentionally small; provider-specific fields belong in opaque attributes or in the owning domain, not as new foundation behavior.
