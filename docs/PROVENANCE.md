# Clean-copy provenance

This repository was created by clean-copy extraction. Git history was not rewritten or filtered.

- Source repository: `moritzbrantner/rust-packages`
- Reviewed Phase-A ownership baseline: `d032ad2890c1df3c6a5b9eff024562f00d017fce`
- Exact extraction commit: `364627c233b314807ba4f21298ada4cf63333bed`
- Extraction issue: `moritzbrantner/rust-packages#110`
- Parent PRD: `moritzbrantner/rust-packages#106`
- Destination: `moritzbrantner/moenarch-foundation`
- History note: original per-file history remains in the source repository; this destination begins with one attributed bootstrap commit.

The byte-identical copied source is exactly the 60 crate directories named by
the `manifest_path` fields in
`docs/repository-split/package-ownership.json` (each manifest's containing
directory, recursively). Directory-by-directory comparison against extraction
commit `364627c233b314807ba4f21298ada4cf63333bed` verified those crate trees before
the bootstrap commit.

The following basic repository inputs were also copied byte-identically:

- `LICENSE-APACHE`, `LICENSE-MIT`, `rust-toolchain.toml`, `.editorconfig`

Destination-authored or materially adapted support is not represented as a
byte-identical copy:

- Root `Cargo.toml` was authored with exactly 60 workspace members,
  destination repository metadata, selected destination workspace dependencies, and
  an exact `ort = "=2.0.0-rc.12"` constraint. The exact constraint preserves
  the source lockfile-compatible API after unconstrained fresh resolution
  selected incompatible release candidate 13.
- `Cargo.lock` was regenerated for this destination-only workspace. Its package
  graph and checksums therefore intentionally differ from the monolith lockfile.
- `README.md`, `AGENTS.md`, `CONTEXT.md`, this provenance record,
  `.github/workflows/workspace-ci.yml`, and `.harness/**` were authored for the
  destination.
- `.gitignore` began as a copy and was adapted with destination Harness receipt
  and Python bytecode exclusions.
- `docs/adr/0012-capability-repository-split-and-agent-releases.md`,
  `docs/AGENT_DRIVEN_RELEASES.md`, and `docs/RELEASE_CHECKLIST.md` were adapted
  to the foundation-only, non-publishing bootstrap contract.
- `docs/repository-split/package-ownership.json` was filtered to the exact 58
  Phase-A foundation Cargo records plus the two append-only contract records.
  `docs/repository-split/release-plan.json` was authored as an exact
  non-publishing inventory.
- `scripts/repository_split.py`, both validators and tests, and the scoped
  fixtures were materially adapted into destination-focused boundary and
  release checks.

All copied Rust sources remain available in `rust-packages` at the extraction commit. Dual-licensed material retains the repository's MIT OR Apache-2.0 terms through `LICENSE-MIT` and `LICENSE-APACHE`. No additional notice file existed in the literal extraction scope. No Bun/npm packages, generated build output, vendored projects, external media, or source history were copied.

The draft Harness profile was detected from the empty destination before
mutation and audited after authoring. Draft Harness audits and runs are
optional, non-authoritative discovery/structural evidence only. They do not
activate policy, imply that Cargo checks ran, or replace the issue-required
repository checks as handoff authority.

The Harness profile binds the source issue and parent PRD as external
requirements. Their bodies and digests are captured only in ignored local
verification state; private requirement bodies are not committed.
