# Project invariants

## INV-001 — Audio contracts metadata is exact

- Requirement: `moenarch-audio-contracts` declares version 0.1.0 explicitly and its `moenarch-media-core` path dependency resolves with the exact registry-safe requirement `=0.1.0`.
- Forbidden behavior: inherited package versions, permissive internal version requirements, path escape, or moving-branch Git dependencies.
- Authority/source: issue:#17
- Affected surfaces: Cargo.toml, Cargo.lock, crates/audio/audio-contracts/Cargo.toml
- Compatibility promise: Release preparation changes package metadata only and does not change crate source behavior.
- Required evidence: static
- Sensitivity: optional
- Risk dimensions: security=covered:INV-002; recovery=covered:INV-002; persistence=not-applicable:no-storage-change; concurrency=not-applicable:no-concurrency-change; migration=covered:INV-002; partial-failure=covered:INV-002; operational=covered:INV-003

## INV-002 — The release contract authorizes only audio contracts

- Requirement: The checked manifest binds destination issue #17, the exact source commit, only `moenarch-audio-contracts` at 0.1.0, its registry prerequisite, and the restructuring-first command set.
- Forbidden behavior: extra packages, another issue or repository, a mutable or unrelated source, behavioral gate claims, or source/control drift outside the manifest.
- Authority/source: issue:#16
- Affected surfaces: .agent-loop.toml, releases/foundation-audio-contracts.toml, scripts/check_release_plan.py, scripts/publish_release.py
- Compatibility promise: Existing release manifests retain their historical gates; wave 2 issue #13 must publish before this PR can merge or issue #17 can publish.
- Required evidence: contract
- Sensitivity: optional
- Risk dimensions: security=covered:INV-002; recovery=covered:INV-002; persistence=not-applicable:no-storage-change; concurrency=not-applicable:publication-is-serialized; migration=covered:INV-002; partial-failure=covered:INV-002; operational=covered:INV-003

## INV-003 — Only audio contracts enters the crates.io archive gate

- Requirement: The structural archive gate runs `cargo package --locked --registry crates-io` for only `moenarch-audio-contracts`, using its reviewed candidate closure as a temporary local patch.
- Forbidden behavior: packaging every workspace crate, selecting another registry, passing local patches to publication, or claiming consumer or behavioral evidence.
- Authority/source: issue:#17
- Affected surfaces: .agent-loop.toml, releases/foundation-audio-contracts.toml, scripts/check_release_plan.py
- Compatibility promise: Archive preparation remains side-effect free for crates.io and changes only ignored build output.
- Required evidence: integration
- Sensitivity: optional
- Risk dimensions: security=covered:INV-002; recovery=covered:INV-002; persistence=not-applicable:no-registry-write; concurrency=not-applicable:no-publication-effect; migration=covered:INV-002; partial-failure=covered:INV-003; operational=covered:INV-003
