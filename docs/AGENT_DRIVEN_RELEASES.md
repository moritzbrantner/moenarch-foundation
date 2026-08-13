# Agent-driven releases

Issue #110 and the release-control setup issue authorize no publication.
`docs/repository-split/release-plan.json` remains an exact non-publishing
bootstrap inventory: every package retains its source version, `publish` is
false, tags are absent, and `release_issue` is null.

## Destination-local authorization

A future release starts with a separate open issue in
`moritzbrantner/moenarch-foundation`. The issue must carry `release:approved`
and its checked TOML manifest must bind this repository, the issue number, and
the exact 40-character commit from which publication runs. An issue in
`rust-packages` or another repository cannot authorize this publisher.

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
head_sha = "0123456789abcdef0123456789abcdef01234567"
registry = "crates.io"
dependency_order = ["example-core"]
expected_tags = ["example-core-v1.2.3"]

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
release must refer to a unique declared tag. Unknown fields fail closed.

## Publisher guarantees

Before its first publishing effect, the hook validates the clean checkout,
repository, exact head, open issue and label, exactly one matching checked
manifest, repository ownership, Cargo package/version/manifest identity,
crates.io eligibility, dependency order, registry state, tags, and existing
GitHub Releases. It packages every candidate before publication.

Registry-present versions are skipped only after their exact non-yanked record
is verified. Present versions must form a dependency-ordered prefix. The hook
publishes from the first absent version, stops at the first failure, and waits
for each immutable crates.io version before continuing. Only after every
package is registry-verified does it create or resume manifest-declared tags
and GitHub Releases. Existing tags and releases must exactly agree, so reruns
can resume but can never overwrite, delete, republish, infer, or automatically
yank anything. Cargo credentials remain in Cargo's normal credential mechanism
and must never be printed or copied into repository files.

Source removal from `rust-packages` remains a later gate after registry-only
consumer evidence.
