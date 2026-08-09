# Clean-copy provenance

This repository was created by clean-copy extraction. Git history was not rewritten or filtered.

- Source repository: `moritzbrantner/rust-packages`
- Reviewed Phase-A ownership baseline: `d032ad2890c1df3c6a5b9eff024562f00d017fce`
- Exact extraction commit: `364627c233b314807ba4f21298ada4cf63333bed`
- Extraction issue: `moritzbrantner/rust-packages#110`
- Parent PRD: `moritzbrantner/rust-packages#106`
- Destination: `moritzbrantner/moenarch-foundation`
- History note: original per-file history remains in the source repository; this destination begins with one attributed bootstrap commit.

The copied source paths are exactly the 60 crate directories named by the `manifest_path` fields in `docs/repository-split/package-ownership.json` (each manifest's containing directory, recursively), plus these scoped support inputs:

- `LICENSE-APACHE`, `LICENSE-MIT`, `rust-toolchain.toml`, `.gitignore`, `.editorconfig`
- `docs/adr/0012-capability-repository-split-and-agent-releases.md`
- `docs/AGENT_DRIVEN_RELEASES.md`, `docs/RELEASE_CHECKLIST.md`
- `scripts/repository_split.py`, both focused validators and tests, and their scoped fixture directories

Destination-authored files are `Cargo.toml`, `Cargo.lock`, `README.md`, `AGENTS.md`, `CONTEXT.md`, `.github/workflows/workspace-ci.yml`, `.harness/**`, this provenance record, and adapted ownership/release/agent guidance.

All copied Rust sources remain available in `rust-packages` at the extraction commit. Dual-licensed material retains the repository's MIT OR Apache-2.0 terms through `LICENSE-MIT` and `LICENSE-APACHE`. No additional notice file existed in the literal extraction scope. No Bun/npm packages, generated build output, vendored projects, external media, or source history were copied.

The draft Harness profile was detected from the empty destination before mutation and audited after authoring. Detection and audit are discovery/structural evidence only; they are not authoritative handoff evidence and do not activate policy or imply that Cargo checks ran.
