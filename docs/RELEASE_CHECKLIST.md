# Release checklist

The bootstrap plan and release-control setup are non-publishing. Do not run
`cargo publish`, create a release manifest, tag, release, or change package
versions under either setup authorization.

For a future authorized release:

1. Use a separate open release issue in this destination repository. Review
   its exact `releases/*.toml` manifest and only then apply `release:approved`.
2. Confirm repository, issue, immutable head, package names, versions, owners,
   manifest paths, dependency order, tags, optional GitHub Releases, required
   checks, and consumer evidence.
3. Commit the exact package source first, then add only its release manifest in
   the control commit. Put the exact control commit as
   `Release control head SHA: <sha>` and the manifest's digest as
   `Release manifest SHA-256: <digest>` in the issue body. If merging before
   publication, preserve both commits and authorize the exact post-merge head;
   do not squash or rebase away the `source_sha` ancestry and manifest-only diff.
4. Require a clean exact commit and run the ordered repository-wide
   `.agent-loop.toml` verification. Release issues #13 and #17 retain the exact
   reduced preparation gates recorded in their manifests, but those historical
   issue contracts do not replace the recurring exact-head gate for ordinary
   work or a later publication receipt. The structural package gate and
   publisher use reviewed local patches for the exact candidate closure before
   publication; the publisher never passes those patches to `cargo publish`.
5. Run every manifest-declared candidate consumer check before publication.
   An explicitly empty list means no consumer result is required or claimed.
6. Let only the receipt-gated Agent Loop master invoke
   `python3 scripts/agent_loop_local_verification.py publish`; do not call Cargo
   publication or the repository hook by hand.
7. Confirm the hook pins Cargo to `crates-io`, publishes topologically, verifies
   each packaged checksum against the immutable registry version, creates each
   manifest-declared tag explicitly at `source_sha`, verifies that exact remote
   target, and only then creates the tag's manifest-declared GitHub Release.
   The exact control head remains the checkout and issue-authorization binding;
   it is never the package tag or Release target.
8. On failure, preserve published versions and resume from the first absent
   registry version. Never overwrite, delete, republish, or automatically yank.
9. Run isolated registry-only consumer checks and retain historical
   `rust-packages` compatibility/provenance source until every separately
   authorized destination, compatibility, rollback, and consumer gate passes.
   Its continued presence does not retain release authority there. Wave 2
   publication does not itself claim those checks.

The issue #17 preparation PR is strictly ordered after foundation wave 2 issue
#13. It may open for review, but must not merge or publish until wave 2 has
published and shared crates.io capacity is available. Rebase and rebuild the
source/control pair if that ordering changes either exact commit; never reuse
stale issue authorization.
