# Agent instructions

This repository contains 60 domain-neutral Rust packages. Read `CONTEXT.md`, `docs/PROVENANCE.md`, the ADR, ownership map, and release plan before changing package boundaries or release metadata.

## Boundaries

- Keep foundation independent of every capability repository.
- Do not add path dependencies that escape this checkout or Git dependencies on moving branches.
- Keep CLI/server/WASM adapters with their wrapped libraries until a separate usage and semver decision authorizes removal.
- Do not add Bun/npm surfaces here.
- Do not publish, tag, release, or remove source from `rust-packages` without an exact release issue and validated publishing manifest.
- Treat `.harness/` as draft policy until maintainers explicitly activate it.

## Checks

Run the narrowest relevant Cargo check during development, then the issue-required workspace checks. Always run:

```bash
python3 scripts/check_repository_boundaries.py --check
python3 scripts/check_release_plan.py --check docs/repository-split/release-plan.json
python3 -m unittest discover -s scripts -p 'test_*.py'
```

For release-oriented work, package every public crate with `cargo package -p <name> --locked` and follow `docs/RELEASE_CHECKLIST.md`. Never weaken checks or report unrun evidence as passing.

<!-- verification-harness:start -->
## Verification harness
Run the installed `moenarch-verification-harness` skill's `audit` command before changing verification surfaces.
Early selection is advisory; `full` remains the handoff gate. See `.harness/README.md`.
<!-- verification-harness:end -->
