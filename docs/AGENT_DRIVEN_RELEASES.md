# Agent-driven releases

Issue #110 and the release-control setup issue authorize no publication.
`docs/repository-split/release-plan.json` remains an exact non-publishing
bootstrap inventory: every package retains its source version, `publish` is
false, tags are absent, and `release_issue` is null.

The draft Harness profile has one required bootstrap-only structural audit for
setup issue #3 and PR #5. Capture the ignored external-requirements bundle and
run:

```bash
python3 scripts/repository_split.py --harness-audit \
  --base-ref fa25668b2598be34d6c86de2234961969cebfb9b
```

Record the exact candidate head and valid audit result on the pull request.
This structural result is non-authoritative and does not replace the recurring
`.agent-loop.toml` checks. Because the profile remains draft and the reviewed
base is bootstrap-specific, the command is deliberately not a recurring Agent
Loop or CI gate; do not substitute `origin/main` or a moving placeholder.

## Destination-local authorization

A future release starts with a separate open issue in
`moritzbrantner/moenarch-foundation`. The issue must carry `release:approved`
and its checked TOML manifest must bind this repository, the issue number, and
an exact 40-character source commit. The issue body must contain
`Release control head SHA: <sha>` for the exact publication/control commit and
`Release manifest SHA-256: <digest>` for the checked manifest's exact bytes.
Both lines are revalidated before every release effect. An issue in
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
release must refer to a unique declared tag. Active manifests bind the exact
`.agent-loop.toml` gate. Completed historical manifests retain the gate that
authorized them. `required_consumer_checks` may be empty when the destination
release issue explicitly removes consumer verification. Declared consumer
commands run again before publication. Unknown fields fail closed.

## Restructuring-first wave 2

Destination issue #13 authorizes preparation of only
`moenarch-graph-analysis-core`, `moenarch-math-statistics`, and
`moenarch-dense-data` at 0.1.0. Its deliberate verification gate is Cargo
metadata, exact manifest/dependency/order validation, and locked package
archives for those three crates. It does not run or claim workspace, unit,
integration, consumer, Clippy, documentation, or all-package suites.

The publisher still enforces a clean exact head, destination-local issue and
manifest binding, registry-safe exact dependency requirements, registry
absence or immutable checksum/non-yanked agreement, idempotent dependency-order
resume, and tags fixed to `source_sha`. Preparation alone does not authorize an
effect: while another release owns crates.io capacity, issue #13 must remain
without `release:approved` and without exact control-head authorization.

## Audio contracts release preparation

Destination issue #17 prepares only `moenarch-audio-contracts` 0.1.0. Its
deliberately reduced gate is Cargo metadata, exact manifest/dependency
validation, and one locked `crates-io` package archive for that crate. It does
not run or claim workspace, unit, integration, consumer, Clippy,
documentation, build, or all-package evidence. The publisher retains the
clean-head, destination-local authority, exact dependency, immutable registry,
idempotent resume, checksum, and source-tag safeguards described below.

This preparation is ordered after foundation wave 2 issue #13. The audio
contracts PR may be reviewed while wave 2 is pending, but it must not merge or
publish until issue #13 has published. Because main's active Agent Loop and
draft Harness configuration currently serves wave 2, rebase the audio
contracts source/control pair after wave 2 if its merge changes either commit.
Keep issue #17 unapproved and without control-head authorization until that
ordering requirement and the shared crates.io-capacity constraint are clear.

The source and manifest use two commits to avoid a self-referential commit
hash: first commit the exact release source, then add only the release manifest
in a second commit. `source_sha` names the first commit. Publication runs from
the second commit's exact Agent Loop head, and the hook requires the only path
changed between those commits to be the selected manifest. It therefore proves
the package source is exactly `source_sha` while the manifest is itself checked
at the exact publication head. Identical manifest bytes on another control
commit are not authorized: that commit needs its own exact control-head line in
the still-open destination issue.

The control head is publication authority, not a release artifact target.
Every manifest-declared annotated tag explicitly targets `source_sha`, and its
GitHub Release identifies that same source through the already-pushed tag. A
local or remote tag at the control head is therefore an immutable conflict,
even though that head is the required clean checkout for publication.

When a release PR must merge before publication, preserve the two release
commits with a merge strategy that leaves `source_sha` as an ancestor of the
final control head and leaves only the selected manifest changed between them.
Authorize the exact post-merge control head and manifest digest. Do not squash
or rebase the two commits after writing the manifest: rewriting can remove the
recorded source ancestor or combine source and manifest. If either SHA changes,
stop and prepare a new source commit followed by a manifest-only control commit,
then update the issue authorization and rerun verification before any effect.

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
before tag effects. Each tag is created explicitly at `source_sha`, its local
target is verified, destination-local control-head authority is refreshed, and
only then is the tag pushed and its remote source target verified. Before a
GitHub Release effect, the hook again refreshes the registry and verifies the
remote tag still targets `source_sha`; it then refreshes control-head authority,
creates the Release with the existing tag, and verifies both the remote source
target and Release metadata. Exact concurrent desired state reconciles
idempotently, while yanked/checksum/tag-target/release-metadata conflicts stop
without retrying publication, tag push, or release creation. Existing tags and
releases must exactly agree, so reruns can resume but can never overwrite,
delete, republish, infer, or automatically yank anything. Cargo credentials
remain in Cargo's normal credential mechanism and must never be printed or
copied into repository files.

Source removal from `rust-packages` remains a later gate after registry-only
consumer evidence.
