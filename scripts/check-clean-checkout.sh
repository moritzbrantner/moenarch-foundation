#!/usr/bin/env bash
# Clean-checkout verification for the default Foundation workspace.
#
# This is the `[bootstrap].check` command declared in `.repository.toml`. It
# needs only this checkout, the pinned Rust toolchain, Python 3.11+, and the
# locked crates.io dependencies fetched by `cargo fetch --locked`. It never
# reads a sibling repository and never activates source-development mode; see
# docs/CLEAN_CHECKOUT.md. `scripts/check-fast.sh` remains the full inner-loop
# workspace gate, and registry/distribution verification remains a release task.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

if [[ -e .coding-tooling.source-deps.json ]]; then
  echo "error: clean-checkout verification must not depend on source-development declarations" >&2
  exit 1
fi

cargo metadata --locked --offline --format-version 1 --no-deps >/dev/null
python3 scripts/check_repository_boundaries.py --check
python3 scripts/check_release_plan.py --check docs/repository-split/release-plan.json
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo fmt --all -- --check

# Public interchange smoke examples: compile, run as tests, and run as binaries.
cargo test --locked --offline -p moenarch-corpus-core -p moenarch-media-core
cargo run --locked --offline -q -p moenarch-corpus-core --example corpus_round_trip >/dev/null
cargo run --locked --offline -q -p moenarch-media-core --example media_time_round_trip >/dev/null

echo "clean-checkout verification passed without sibling repositories or source-development mode"
