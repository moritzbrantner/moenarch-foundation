# moenarch-foundation

`moenarch-foundation` is the domain-neutral Rust foundation for Moenarch projects. It contains reusable runtime, jobs, media/time, audio contract, data, math, tensor, graph, geometry, signal, and vector packages plus their focused Rust CLI, server, and WASM adapters.

This repository was bootstrapped as a clean copy from `moritzbrantner/rust-packages`; see [docs/PROVENANCE.md](docs/PROVENANCE.md). The source repository remains the active release owner until separately authorized publication and consumer gates complete.

## Local verification

```bash
cargo metadata --format-version 1 --no-deps
python3 scripts/check_repository_boundaries.py --check
python3 scripts/check_release_plan.py --check docs/repository-split/release-plan.json
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo doc --workspace --no-deps
```

No issue or manifest in this bootstrap authorizes publishing, tagging, releasing, or removing source from `rust-packages`.
