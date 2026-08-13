# Agent-driven releases

Issue #110 and the release-control setup issue authorize no publication.
`docs/repository-split/release-plan.json` remains an exact non-publishing
bootstrap inventory: every package retains its source version, `publish` is
false, tags are absent, and `release_issue` is null.

## Destination-local authorization

A future release starts with a separate open issue in
`moritzbrantner/moenarch-foundation`. The issue must carry `release:approved`
and its checked TOML manifest must bind this repository, the issue number, and
an exact 40-character source commit. The issue body must contain
`Release manifest SHA-256: <digest>` for the checked manifest's exact bytes. An
issue in `rust-packages` or another repository cannot authorize this publisher.

The Agent Loop first verifies the exact head using the ordered commands in
`.agent-loop.toml`. Its receipt-gated master publication action then invokes
`python3 scripts/publish_release.py` with `AGENT_LOOP_REPOSITORY`,
`AGENT_LOOP_ISSUE`, and `AGENT_LOOP_HEAD_SHA`. Calling the hook without all
three bindings fails before external access.

## Exact manifest

Release manifests are checked as `releases/*.toml`. Setup deliberately creates
no manifest. A future manifest uses schema v1:

```toml
schema_version = 1
repository = "moritzbrantner/moenarch-foundation"
issue = 123
source_sha = "0123456789abcdef0123456789abcdef01234567"
registry = "crates.io"
dependency_order = ["example-core"]
expected_tags = ["example-core-v1.2.3"]
required_checks = ["cargo metadata --format-version 1 --no-deps"]
required_consumer_checks = ["scripts/check_example_consumer.sh"]

[[packages]]
name = "example-core"
version = "1.2.3"
owner = "moritzbrantner/moenarch-foundation"
manifest_path = "crates/example/example-core/Cargo.toml"
dependencies = []
tag = "example-core-v1.2.3"

[[github_releases]]
tag = "example-core-v1.2.3"
title = "Release example-core 1.2.3"
notes = "Reviewed release notes."
```

`dependency_order`, the `packages` array, and package dependencies must agree
with Cargo metadata. `expected_tags` must exactly match package tags, and tags
use `<package>-v<version>`. `github_releases` is optional; every declared
release must refer to a unique declared tag. `required_checks` must exactly
match `.agent-loop.toml`; consumer commands run again before publication.
Unknown fields fail closed.

The source and manifest use two commits to avoid a self-referential commit
hash: first commit the exact release source, then add only the release manifest
in a second commit. `source_sha` names the first commit. Publication runs from
the second commit's exact Agent Loop head, and the hook requires the only path
changed between those commits to be the selected manifest. It therefore proves
the package source is exactly `source_sha` while the manifest is itself checked
at the exact publication head.

## Publisher guarantees

Before its first publishing effect, the hook validates the clean checkout,
repository, exact head, open issue and label, exactly one matching checked
manifest, repository ownership, Cargo package/version/manifest identity,
crates.io eligibility, dependency order, registry state and package checksum,
tags, and existing GitHub Releases. It packages every candidate for the
explicit `crates-io` Cargo registry before publication. A temporary local
`[patch.crates-io]` covering the reviewed wave lets downstream archives verify
before their dependencies exist on the registry; it never changes tracked
manifests and is not passed to `cargo publish`.

Immediately before every publish, tag creation, tag push, and GitHub Release
creation, the hook freshly revalidates the exact checkout and re-fetches the
destination-local issue, including its open state, `release:approved` label,
and exact manifest digest. Revocation stops before the next effect.

Registry-present versions are skipped only after their exact non-yanked record
is verified. Present versions must form a dependency-ordered prefix. The hook
publishes from the first absent version, stops at the first failure, and waits
for each immutable crates.io version before continuing. It refreshes and
validates the entire dependency-ordered registry prefix before each publish.
Only after every package is freshly registry-verified does it create or resume
manifest-declared tags. Registry, local and remote tag state are refreshed
before tag effects; registry, remote tag, and GitHub Release state are refreshed
before release effects. Exact concurrent desired state reconciles
idempotently, while yanked/checksum/tag-target/release-metadata conflicts stop
without retrying publication, tag push, or release creation. Existing tags and
releases must exactly agree, so reruns can resume but can never overwrite,
delete, republish, infer, or automatically yank anything. Cargo credentials
remain in Cargo's normal credential mechanism and must never be printed or
copied into repository files.

Source removal from `rust-packages` remains a later gate after registry-only
consumer evidence.
