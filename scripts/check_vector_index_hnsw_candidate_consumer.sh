#!/usr/bin/env bash
# Candidate consumer gate for the issue #160 vector-analysis-index 0.2.1 release.
#
# Builds and runs a registry-shaped consumer of the exact candidate sources that
# enables the optional `hnsw` feature beside vector-analysis-core 0.2.1. Only the
# two candidates are patched. Their workspace path dependencies (media-core,
# numbers-core, runtime-core) follow the exact source at their already
# published versions; search-kernels and hnsw_rs resolve from crates.io. The
# publisher separately verifies the packaged archives against the registry.
set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
consumer_manifest="$repository_root/scripts/fixtures/vector_index_hnsw_consumer/Cargo.toml"
scratch_root=${TMPDIR:-/tmp}
patch_config=$(mktemp "$scratch_root/moenarch-vector-index-hnsw-patches.XXXXXX.toml")

cleanup() {
  case "$patch_config" in
    "$scratch_root"/moenarch-vector-index-hnsw-patches.*.toml)
      unlink -- "$patch_config"
      ;;
    *)
      echo "refusing to remove unexpected patch config: $patch_config" >&2
      ;;
  esac
}
trap cleanup EXIT

{
  echo '[patch.crates-io]'
  printf '"%s" = { path = "%s/%s" }\n' \
    moenarch-vector-analysis-core "$repository_root" crates/vector/vector-analysis-core \
    moenarch-vector-analysis-index "$repository_root" crates/vector/vector-analysis-index
} >"$patch_config"

export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$repository_root/target"}
cargo run \
  --locked \
  --quiet \
  --manifest-path "$consumer_manifest" \
  --config "$patch_config"

echo "vector-analysis-index 0.2.1 hnsw candidate consumer passed"
