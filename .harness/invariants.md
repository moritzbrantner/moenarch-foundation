# Project invariants

## INV-001 — The approved public Rust surface remains independently usable

- Requirement: Exactly the 60 reviewed packages build, test, document, and package from this checkout while retaining their source names and versions.
- Forbidden behavior: omitted packages, unreviewed packages, broken public behavior, or dependence on another checkout.
- Authority/source: repo:docs/repository-split/package-ownership.json
- Affected surfaces: Cargo.toml, Cargo.lock, crates/**
- Linked tests: repo:scripts/test_check_repository_boundaries.py
- Compatibility promise: Clean-copy bootstrap does not intentionally alter public Rust APIs, serialized shapes, or operation IDs.
- Required evidence: contract, behavioral, static
- Sensitivity: not-required
- Risk dimensions: security=covered:INV-002; recovery=covered:INV-003; persistence=not-applicable:no-storage-contract-change; concurrency=not-applicable:no-concurrency-contract-change; migration=covered:INV-003; partial-failure=covered:INV-003; operational=covered:INV-003

## INV-002 — Foundation has no dependency escape or reverse capability edge

- Requirement: Every local dependency resolves inside this repository and no dependency follows a moving Git branch or imports a target capability repository.
- Forbidden behavior: paths outside the checkout, moving-branch Git dependencies, or foundation-to-capability edges.
- Authority/source: repo:CONTEXT.md
- Affected surfaces: Cargo.toml, Cargo.lock, crates/**/Cargo.toml
- Linked tests: repo:scripts/test_check_repository_boundaries.py
- Compatibility promise: Consumers can build from a clean clone without a sibling repository.
- Required evidence: contract
- Sensitivity: not-required
- Risk dimensions: security=covered:INV-002; recovery=covered:INV-003; persistence=not-applicable:no-storage-contract-change; concurrency=not-applicable:no-concurrency-contract-change; migration=covered:INV-003; partial-failure=covered:INV-003; operational=covered:INV-003

## INV-003 — Bootstrap cannot authorize publication or source removal

- Requirement: Every package remains at its source version with publish=false, no tags, and no release issue until a later exact authorization.
- Forbidden behavior: implicit publication, version bump, tag, release, source removal, or claim of authoritative draft Harness evidence.
- Authority/source: repo:docs/repository-split/release-plan.json
- Affected surfaces: docs/repository-split/**, docs/AGENT_DRIVEN_RELEASES.md, docs/RELEASE_CHECKLIST.md, .harness/**
- Linked tests: repo:scripts/test_check_release_plan.py
- Compatibility promise: rust-packages remains the active source/release owner until later gates complete.
- Required evidence: contract
- Sensitivity: not-required
- Risk dimensions: security=covered:INV-002; recovery=covered:INV-003; persistence=not-applicable:no-state-migration; concurrency=not-applicable:no-concurrent-release; migration=covered:INV-003; partial-failure=covered:INV-003; operational=covered:INV-003
