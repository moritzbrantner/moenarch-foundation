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
   `Release manifest SHA-256: <digest>` in the issue body.
4. Require a clean exact commit and run the ordered `.agent-loop.toml`
   verification. Package every public crate and prove no path escape, local
   patch, or moving-branch Git dependency survives packaging.
5. Run every manifest-declared candidate consumer check before publication.
6. Let only the receipt-gated Agent Loop master invoke
   `python3 scripts/agent_loop_local_verification.py publish`; do not call Cargo
   publication or the repository hook by hand.
7. Confirm the hook pins Cargo to `crates-io`, publishes topologically, verifies
   each packaged checksum against the immutable registry version, and creates
   only manifest-declared tags and releases afterward.
8. On failure, preserve published versions and resume from the first absent
   registry version. Never overwrite, delete, republish, or automatically yank.
9. Run isolated registry-only consumer checks and retain `rust-packages` source
   until every destination, compatibility, rollback, and consumer gate passes.
