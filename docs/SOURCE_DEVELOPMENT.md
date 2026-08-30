# Foundation source development

`moenarch-foundation` is the canonical source for its checked package inventory.
It is developed as source first and released separately.

A downstream consumer may keep its normal registry dependency declaration while using a committed `.coding-tooling.source-deps.json` to pin the exact foundation revision under development. For private cross-repository work, that declaration should enable `cargo.localOnly`: `coding-tooling source-deps activate` then requires the consumer's sibling `moenarch-foundation` checkout to exist at exactly the declared Git `HEAD`. Missing or mismatched local source is an error; ordinary source development does not fall back to an authenticated remote Git fetch.

The outer coding loop or agent workspace owns the sibling repository/worktree and prepares it at the pinned revision before source activation. This lets feature work cross the repository boundary without publishing intermediate crate versions or making GitHub Actions credentials part of the development contract.

Keep package versions stable during source work when compatibility permits, update the consumer's source declaration when the validated foundation head changes, and do not start a release train merely to make development compile. Hosted CI may remain repository-local when it cannot access the private multi-repository source workspace.

Registry-only verification remains mandatory for distribution. A later release task deactivates source mode, determines the minimal publication closure, performs any required version bumps, publishes in dependency order, and verifies clean consumers against crates.io.

Source mode changes dependency resolution only. It does not authorize
publication, tags, releases, removal of historical compatibility/provenance
source from `rust-packages`, or new package boundaries. It also does not
transfer source or release authority back to that historical checkout.
