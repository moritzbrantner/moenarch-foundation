# Foundation source development

`moenarch-foundation` is developed as source first and released separately.

A downstream consumer may keep its normal registry dependency declaration while using a committed `.coding-tooling.source-deps.json` to pin the exact foundation revision under development. `coding-tooling source-deps activate` materializes a managed, ignored Cargo patch configuration. A sibling checkout is accepted only when its Git `HEAD` equals the declared revision; otherwise the exact Git revision is used.

This lets feature work cross the repository boundary without publishing intermediate crate versions. Keep package versions stable during source work when compatibility permits, update the consumer's source declaration when the validated foundation head changes, and do not start a release train merely to make development compile.

Registry-only verification remains mandatory for distribution. A later release task deactivates source mode, determines the minimal publication closure, performs any required version bumps, publishes in dependency order, and verifies clean consumers against crates.io.

Source mode changes dependency resolution only. It does not authorize publication, tags, releases, source removal from `rust-packages`, or new package boundaries.
