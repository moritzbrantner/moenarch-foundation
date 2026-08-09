# Agent-driven releases

Issue #110 authorizes no publication. `docs/repository-split/release-plan.json` is an exact non-publishing bootstrap inventory: every package retains its source version, `publish` is false, tags are absent, and `release_issue` is null.

A future release requires a separate exact issue in this destination repository, an independently reviewed publishing manifest, a clean immutable commit, all required package and consumer checks, dependency-order publication, crates.io verification, and package-specific immutable tags. Cargo credentials remain in Cargo's normal credential mechanism and must never be printed or copied into repository files.

Partial publication must stop at the first failure. Published versions are never overwritten, deleted, silently skipped, automatically yanked, or inferred beyond the reviewed manifest. Source removal from `rust-packages` is a later gate after registry-only consumer evidence.
