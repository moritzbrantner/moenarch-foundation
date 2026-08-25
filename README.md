# moenarch-foundation

`moenarch-foundation` is the domain-neutral Rust foundation for Moenarch projects. It contains reusable runtime, jobs, media/time, audio contract, data, math, tensor, graph, geometry, signal, and vector packages plus their focused Rust CLI, server, and WASM adapters.

This repository was bootstrapped as a clean copy from `moritzbrantner/rust-packages`; see [docs/PROVENANCE.md](docs/PROVENANCE.md). The source repository remains the active release owner until separately authorized publication and consumer gates complete.

GitHub Issues are the durable agent queue; see
[the issue-tracker contract](docs/agents/issue-tracker.md) and
[planning workflow](docs/agents/planning-workflow.md). Repository-local Agent
Loop policy lives in `.agent-loop.toml`.

## Source development

Normal implementation work may be validated by downstream repositories against an exact foundation source revision before a crates.io release exists. Consumers keep registry coordinates in their manifests and use the managed source-development configuration provided by `coding-tooling`; publishing remains a separate release task.

See [docs/SOURCE_DEVELOPMENT.md](docs/SOURCE_DEVELOPMENT.md).

## Local verification

```bash
cargo metadata --format-version 1 --no-deps
python3 scripts/check_repository_boundaries.py --check
python3 scripts/check_release_plan.py --check docs/repository-split/release-plan.json
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo doc --workspace --no-deps
python3 scripts/check_release_plan.py --package-all docs/repository-split/release-plan.json
```

No issue or manifest in this bootstrap authorizes publishing, tagging,
releasing, or removing source from `rust-packages`. Future exact releases use
the destination-local, manifest-gated flow in
[docs/AGENT_DRIVEN_RELEASES.md](docs/AGENT_DRIVEN_RELEASES.md).
