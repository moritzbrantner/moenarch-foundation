# Project invariants

## INV-001 — Foundation wave 2 metadata is exact

- Requirement: The three selected crates declare version 0.1.0 explicitly and every workspace path dependency resolves with an exact registry-safe version requirement.
- Forbidden behavior: inherited package versions, permissive internal version requirements, path escape, or moving-branch Git dependencies.
- Authority/source: issue:#13
- Affected surfaces: Cargo.toml, Cargo.lock, crates/data/dense-data/Cargo.toml, crates/data/graph-analysis-core/Cargo.toml, crates/math/math-statistics/Cargo.toml
- Compatibility promise: Release preparation changes package metadata only and does not change crate source behavior.
- Required evidence: static
- Sensitivity: optional
- Risk dimensions: security=covered:INV-002; recovery=covered:INV-002; persistence=not-applicable:no-storage-change; concurrency=not-applicable:no-concurrency-change; migration=covered:INV-002; partial-failure=covered:INV-002; operational=covered:INV-003

## INV-002 — The release contract authorizes only the exact wave

- Requirement: The checked manifest binds destination issue #13, the exact source commit, the three selected packages at 0.1.0, their dependency order, and the restructuring-first command set.
- Forbidden behavior: extra packages, another issue or repository, a mutable or unrelated source, behavioral gate claims, or source/control drift outside the manifest.
- Authority/source: issue:#12
- Affected surfaces: .agent-loop.toml, releases/foundation-wave-2.toml, scripts/check_release_plan.py, scripts/publish_release.py
- Compatibility promise: Existing completed release manifests retain their historical gates while wave 2 remains independently authorized.
- Required evidence: contract
- Sensitivity: optional
- Risk dimensions: security=covered:INV-002; recovery=covered:INV-002; persistence=not-applicable:no-storage-change; concurrency=not-applicable:publication-is-serialized; migration=covered:INV-002; partial-failure=covered:INV-002; operational=covered:INV-003

## INV-003 — Only selected crates enter the crates.io archive gate

- Requirement: The structural archive gate runs `cargo package --locked --registry crates-io` for graph-analysis-core, math-statistics, and dense-data in dependency order using only the reviewed candidate closure as temporary local patches.
- Forbidden behavior: packaging every workspace crate, selecting another registry, passing local patches to publication, or claiming consumer or behavioral evidence.
- Authority/source: issue:#13
- Affected surfaces: .agent-loop.toml, releases/foundation-wave-2.toml, scripts/check_release_plan.py
- Compatibility promise: Archive preparation remains side-effect free for crates.io and changes only ignored build output.
- Required evidence: integration
- Sensitivity: optional
- Risk dimensions: security=covered:INV-002; recovery=covered:INV-002; persistence=not-applicable:no-registry-write; concurrency=not-applicable:no-publication-effect; migration=covered:INV-002; partial-failure=covered:INV-003; operational=covered:INV-003
