# Foundation neutrality and clean-checkout contract

Issue #108 makes the Foundation boundary executable. This page names the
machine-readable declaration, the check that enforces it, and the verification
path for a fresh clone.

## Declaration

[`.repository.toml`](../.repository.toml) is the repository contract consumed by
`coding-tooling repository contract` (`coding-tooling/repository-contract/v1`):

| Field | Foundation value |
| --- | --- |
| `summary` | One-sentence purpose. |
| `depends_on` | Upstream repositories whose crates may enter the dependency graph: only `rust-kernels`. |
| `consumed_by` | Capability and product repositories that consume Foundation, including `audio-analysis`, `visual-analysis`, `nlp-stack` and `scenedetect-rs`. Consumers are never implementation dependencies. |
| `[architecture].owns` | Domain-neutral contracts and mechanisms Foundation owns. |
| `[architecture].must_not_own` | Explicit exclusions: ASR, OCR, scene detection, NLP interpretation, product workflow, corpus policy, belief/philosophy semantics, and consumer domain semantics. |
| `[bootstrap]` | `cargo fetch --locked`, then `bash scripts/check-clean-checkout.sh`. |
| `[pages]` | `status = "not-applicable"` with a reason. There is no Pages workflow; this stays N/A until a real Foundation site is intentionally introduced. |

## Boundary check

`python3 scripts/check_repository_boundaries.py --check` validates the
declaration and the Cargo graph together. In addition to the existing ownership
inventory rules, it fails when:

- the purpose is missing or not one sentence, a required exclusion is dropped,
  an owned capability falls inside an exclusion, or Pages is not declared N/A;
- `audio-analysis`, `visual-analysis`, `nlp-stack` or `scenedetect-rs` is
  removed from `consumed_by` or listed in `depends_on`;
- any workspace dependency of any kind (normal, dev or build), any package in
  `Cargo.lock`, or any workspace `[patch]` entry names a package published by
  those capability repositories, or a Moenarch package outside the Foundation
  inventory;
- any Git source points at a consumer repository, or at any other
  `moritzbrantner` repository not declared in `depends_on` (this covers
  application repositories generically);
- any dependency or patch path escapes this checkout.

`scripts/test_check_repository_boundaries.py` covers each rule.

## Clean-checkout verification

A fresh clone needs only the pinned Rust toolchain (`rust-toolchain.toml`),
Python 3.11 or newer, Git, and crates.io access for the first fetch. No sibling
repository and no source-development declaration is read:

```bash
git clone https://github.com/moritzbrantner/moenarch-foundation.git
cd moenarch-foundation
cargo fetch --locked
bash scripts/check-clean-checkout.sh
```

The check runs offline after the fetch: locked metadata, the boundary and
release-plan checks, the script unit tests, formatting, and the
`corpus-core`/`media-core` public interchange examples:

- `crates/corpus/corpus-core/examples/corpus_round_trip.rs`: source, asset,
  segment, representation and provenance records round-trip through JSON with
  their relationships intact.
- `crates/media/media-core/examples/media_time_round_trip.rs`: exact rational
  timestamps and ranges, exact rescaling, timed-text JSON and WebVTT, and a
  media-located annotation round-trip.

Both examples are declared with `test = true`, so ordinary
`cargo test --workspace` also runs them. `coding-tooling repository contract
--root . --execute` runs the same bootstrap in a disposable clone.

## Separate verification modes

These modes stay independent:

1. **Clean checkout** (`scripts/check-clean-checkout.sh`): this repository
   alone, at its exact commit. It never needs another checkout.
2. **Full workspace gate** (`scripts/check-fast.sh`, `release-gate.toml`, CI):
   every package, still repository-local.
3. **Source development** ([SOURCE_DEVELOPMENT.md](SOURCE_DEVELOPMENT.md)):
   optional and explicit, declared by a downstream consumer that pins an exact
   Foundation revision. Foundation's own validation never requires it.
4. **Registry/distribution verification**
   ([AGENT_DRIVEN_RELEASES.md](AGENT_DRIVEN_RELEASES.md)): a separate release
   task against crates.io. Clean-checkout and source-mode evidence are not
   release evidence.
