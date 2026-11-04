#!/usr/bin/env bash
# Candidate consumer gate for the issue #143 media-core 0.2 wave.
#
# Builds a registry-shaped consumer of all fourteen wave crates against the
# exact candidate sources. External kernels that are not yet on crates.io are
# patched to the same exact Git revision the workspace root uses; that patch is
# verification-only and never reaches `cargo publish`.
set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
consumer_manifest="$repository_root/scripts/fixtures/media_core_0_2_consumer/Cargo.toml"
scratch_root=${TMPDIR:-/tmp}
patch_config=$(mktemp "$scratch_root/moenarch-media-core-0-2-patches.XXXXXX.toml")

cleanup() {
  case "$patch_config" in
    "$scratch_root"/moenarch-media-core-0-2-patches.*.toml)
      unlink -- "$patch_config"
      ;;
    *)
      echo "refusing to remove unexpected patch config: $patch_config" >&2
      ;;
  esac
}
trap cleanup EXIT

write_patch() {
  local package=$1
  local relative_path=$2
  printf '"%s" = { path = "%s/%s" }\n' \
    "$package" "$repository_root" "$relative_path"
}

{
  echo '[patch.crates-io]'
  write_patch moenarch-media-core crates/media/media-core
  write_patch moenarch-runtime-core crates/runtime/runtime-core
  write_patch moenarch-audio-contracts crates/audio/audio-contracts
  write_patch moenarch-data-inversion-core crates/data/data-inversion-core
  write_patch moenarch-graph-analysis-core crates/data/graph-analysis-core
  write_patch moenarch-math-geometry-2d crates/math/math-geometry-2d
  write_patch moenarch-numbers-core crates/data/numbers-core
  write_patch moenarch-tensor-data crates/data/tensor-data
  write_patch moenarch-vector-analysis-core crates/vector/vector-analysis-core
  write_patch moenarch-math-signal-core crates/math/math-signal-core
  write_patch moenarch-math-linear crates/math/math-linear
  write_patch moenarch-vector-analysis-index crates/vector/vector-analysis-index
  write_patch moenarch-math-sparse-data crates/math/math-sparse-data
  write_patch moenarch-math-statistics crates/math/math-statistics
  write_patch moenarch-dense-data crates/data/dense-data
  echo 'graph-kernels = { git = "https://github.com/moritzbrantner/rust-kernels", rev = "51ef7878165b83db0d10ad7420fee6b4aba97099" }'
  echo 'search-kernels = { git = "https://github.com/moritzbrantner/rust-kernels", rev = "51ef7878165b83db0d10ad7420fee6b4aba97099" }'
} >"$patch_config"

export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$repository_root/target"}
cargo check \
  --locked \
  --manifest-path "$consumer_manifest" \
  --config "$patch_config"

echo "media-core 0.2 candidate consumer passed"
