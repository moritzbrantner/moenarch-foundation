# Planning Workflow

Substantial work starts as a GitHub PRD issue with explicit acceptance criteria
and out-of-scope boundaries. Apply `prd` and `ready-for-agent` only after it is
decision-complete. Implementation slices use this exact YAML frontmatter:

```yaml
---
parent: 123
blocked_by: []
scope:
  - crates/example/**
---
```

The Agent Loop may parallelize only dependency-ready slices with disjoint,
concrete scopes. Tiny one-shot changes may be implemented directly when a
maintainer explicitly requests that path.
