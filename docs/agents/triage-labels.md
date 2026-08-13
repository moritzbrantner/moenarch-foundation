# Triage Labels

The canonical issue labels are `bug`, `enhancement`, `needs-triage`,
`needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`, and `prd`.

Agent Loop state uses `agent-loop:active`, `agent-loop:blocked`, and
`agent-loop:ready-to-merge`. Native closed issues and merged pull requests are
the completion record.

`release:approved` is a separate, security-sensitive authorization label. It
belongs only on an open destination-local release issue whose reviewed TOML
manifest is bound to the exact immutable source head. Setup and ordinary
implementation issues must never receive it.
