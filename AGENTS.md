# Agent instructions

This repository contains 60 domain-neutral Rust packages. Read `CONTEXT.md`, `docs/PROVENANCE.md`, the ADR, ownership map, and release plan before changing package boundaries or release metadata.

GitHub Issues are the durable planning and execution queue. Read
`docs/agents/issue-tracker.md`, `docs/agents/triage-labels.md`,
`docs/agents/domain.md`, and `docs/agents/planning-workflow.md`. PRD slices use
canonical `parent`, `blocked_by`, and `scope` YAML frontmatter.

## Agent startup and local loop

- On a fresh machine or after the declared toolchain/environment contract changes, run `bash scripts/codex-environment.sh setup`. Use `maintenance` for an existing environment when dependency state changes.
- Before starting implementation, run `bash scripts/check-agent-readiness.sh`. It verifies the semantic environment fingerprint through `coding-tooling`, checks locked Cargo metadata, and requires enough free space for the Cargo target directory before an agent spends model time.
- The default free-space floor is 8 GiB. `AGENT_MIN_FREE_GIB` may be raised for a larger workload or lowered only for a deliberately constrained environment; do not lower it to mask an exhausted build filesystem.
- Preserve the cache paths declared in `.repository-environment.toml` across agent runs. Do not put the Cargo target directory on a disposable or quota-constrained filesystem when a persistent workspace is available.
- During implementation, run the narrowest relevant package/test command first. Before ordinary PR handoff, run `bash scripts/check-fast.sh`.
- `scripts/check-fast.sh` is an inner-loop gate, not merge or release evidence. The commands in `.agent-loop.toml` and the exhaustive workspace CI remain authoritative for final handoff and release-oriented work.

## Boundaries

- Keep foundation independent of every capability repository.
- Ordinary feature work is source-first. Do not publish crates, bump package versions, create tags, or start a release train merely to unblock a downstream consumer.
- Downstream consumers may validate exact foundation source revisions through their committed source-development declarations before registry releases exist. For private cross-repository work, prefer local-only source mode with an exact sibling `moenarch-foundation` checkout/worktree owned by the outer coding workspace.
- Do not add repository secrets, personal access tokens, or authenticated Git fallback merely so hosted CI can reproduce an ordinary private multi-repository source workspace.
- Do not add path dependencies that escape this checkout or Git dependencies on moving branches into committed package manifests.
- Keep CLI/server/WASM adapters with their wrapped libraries until a separate usage and semver decision authorizes removal.
- Do not add Bun/npm surfaces here.
- Keep package versions stable during source-development work when compatibility permits; a dedicated release change owns version bumps and registry publication.
- Registry-only consumer verification is release evidence; it is not required before exact source-mode implementation evidence is useful.
- Do not publish, tag, release, or remove source from `rust-packages` without an exact release issue and validated publishing manifest.
- Release authorization must be an open issue in this repository carrying
  `release:approved` and bound by one checked manifest to the exact source head
  and exact publication/control head. Cross-repository issues are dependency
  records, not publication authority.
- The `.harness/` profile is draft. Its structural audit is required for this
  bootstrap, but remains non-authoritative; targeted and full Harness runs are
  optional until maintainers explicitly activate the profile. The
  issue-required repository checks remain the handoff authority.

## Checks

Run the narrowest relevant Cargo check during development, then the issue-required workspace checks. Always run:

```bash
python3 scripts/check_repository_boundaries.py --check
python3 scripts/check_release_plan.py --check docs/repository-split/release-plan.json
python3 -m unittest discover -s scripts -p 'test_*.py'
```

For release-oriented work, package every public crate with `cargo package -p <name> --locked` and follow `docs/RELEASE_CHECKLIST.md`. Never weaken checks or report unrun evidence as passing.
