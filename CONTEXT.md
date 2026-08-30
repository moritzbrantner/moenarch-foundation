# Repository context

This is the domain-neutral foundation layer of the Moenarch Rust ecosystem. It must not depend on NLP, audio-analysis implementation, visual-analysis, spatial-analysis, application, prototype, Bun/npm, or compatibility-facade repositories.

The workspace owns the 60 extraction packages plus the approved post-extraction
`moenarch-math-geometry-3d` package recorded in
`docs/repository-split/package-ownership.json`. The stable public crate names
and source versions are retained from the extraction commit. Core libraries
should stay composable and adapters should remain thin wrappers around their
named library.

Repository movement is additive at this stage. `moritzbrantner/rust-packages` continues to contain and own active source and releases until a later exact release issue authorizes publication and downstream clean-checkout gates pass.
