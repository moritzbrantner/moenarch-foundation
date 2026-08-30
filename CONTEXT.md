# Repository context

This is the domain-neutral foundation layer of the Moenarch Rust ecosystem. It must not depend on NLP, audio-analysis implementation, visual-analysis, spatial-analysis, application, prototype, Bun/npm, or compatibility-facade repositories.

The workspace owns the 60 extraction packages plus the approved post-extraction
`moenarch-math-geometry-3d` package recorded in
`docs/repository-split/package-ownership.json`. The stable public crate names
and source versions are retained from the extraction commit. Core libraries
should stay composable and adapters should remain thin wrappers around their
named library.

Canonical ownership has cut over for every package in the foundation ownership
inventory. This repository is the sole authority for source changes, tests,
issues, version selection, release manifests, and future publication of those
packages. Historical copies may remain in `moritzbrantner/rust-packages` for
compatibility and provenance, but physical presence there does not confer
source or release authority.

Ownership is not publication authorization. Every release still requires the
destination-local issue, exact manifest, immutable-head binding, packaging,
registry, and consumer gates documented in `docs/AGENT_DRIVEN_RELEASES.md`.
