# Agent Domain Context

Read `CONTEXT.md` for the foundation boundary, ADR 0012 for repository and
release ownership, and `docs/AGENT_DRIVEN_RELEASES.md` for the release control
model.

In Agent Loop vocabulary, a PRD is a parent issue, a slice is one independently
implementable child, and declared write scope is the slice's `scope` paths.
GitHub is the durable queue; `.agent-loop/` contains only private resumable
runtime evidence. A release manifest is reviewed authorization data, while the
repository publisher is the fail-closed mechanism that enforces it.
