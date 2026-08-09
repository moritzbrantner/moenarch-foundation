# Release checklist

The bootstrap plan is non-publishing. Do not run `cargo publish`, create tags or releases, or change package versions for issue #110.

For a future authorized release:

1. Confirm the exact release issue and publishing manifest match package names, versions, owner, source/base commits, dependency order, checks, consumers, and tags.
2. Require a clean exact commit and validate ownership, boundaries, release plan, workspace tests, docs, and every `cargo package --locked` archive.
3. Prove no path escape, local patch, or moving-branch Git dependency exists in packaged manifests.
4. Run candidate consumer checks before publication and isolated registry-only consumer checks afterward.
5. Publish topologically, verify each immutable crates.io version, and only then create its reviewed package tag.
6. Stop on failure and resume from the first unpublished package; never overwrite, delete, or automatically yank a published version.
7. Keep `rust-packages` source until all destination release, consumer, compatibility, and rollback gates pass.
