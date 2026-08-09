# Audio Contracts

`moenarch-audio-contracts` is the cycle-free canonical owner for shared audio
buffers, frames, analyzers, and pipeline contracts.

The crate depends only on `moenarch-media-core`. It deliberately contains no
audio runtime, ingest, visual, NLP, tensor, math, model, or implementation
dependencies. `AnalysisEvent`, time, sample-format, and error contracts remain
owned by `moenarch-media-core` and are re-exported here for audio consumers.

`moenarch-video-analysis-core` re-exports these types for compatibility, so
existing public paths retain exact Rust type identity.
