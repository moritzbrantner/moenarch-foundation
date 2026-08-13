# Issue Tracker: GitHub

GitHub Issues in `moritzbrantner/moenarch-foundation` are the durable work queue.
Use the `gh` CLI or GitHub connector for issue operations. PRDs are parent
issues; implementation slices carry canonical `parent`, `blocked_by`, and
`scope` YAML frontmatter.

Release authorization must come from an open issue in this same repository.
An issue in `rust-packages` or another repository may record an ecosystem
dependency, but it cannot authorize publication from this checkout.
