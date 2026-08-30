# Canonical foundation ownership cutover

## Status

Accepted for every Rust package named by
`docs/repository-split/package-ownership.json`.

This cutover changes ownership authority. It does not publish a package, change
a version, create a tag or GitHub Release, yank a registry version, or delete
historical source.

## Canonical authority

`moritzbrantner/moenarch-foundation` is the sole authority for:

- source changes and public API evolution;
- package tests and compatibility evidence;
- issue tracking and planning for new behavior;
- version selection and release manifests;
- future registry publication.

GitHub Issues in this repository are the durable execution queue. A release
must originate from this repository and satisfy its destination-local release
issue, manifest, immutable-head, package, registry, and consumer gates.
Canonical ownership alone never authorizes publication.

## Historical source role

Copies remaining in `moritzbrantner/rust-packages` are compatibility and
provenance material only. Their physical presence does not make that repository
a competing implementation, issue, version, or release authority. Ecosystem
coordination may still reference a `rust-packages` issue, but only an authorized
destination-local release issue can permit publication from this repository.

Removing historical source remains a separate destructive migration with its
own scope and verification. A failed or deferred destination publication does
not roll ownership back to `rust-packages`; any reverse ownership migration
requires a separate ADR and explicit migration authority.

## Machine-readable records

The ownership inventory records the extraction baseline and canonical
destination. Its record-level digest preserves the reviewed historical
classification. The top-level `canonical_authority` and
`historical_source_role` fields state the current cutover without rewriting
that provenance.

The non-publishing bootstrap release plan names this repository as
`active_release_owner` while retaining `publication_authorized: false`. The
release-plan validator checks both facts so ownership cannot be confused with
permission to publish.
