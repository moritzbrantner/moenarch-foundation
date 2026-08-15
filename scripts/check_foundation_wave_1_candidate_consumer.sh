#!/usr/bin/env bash
set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
consumer_manifest="$repository_root/scripts/fixtures/foundation_wave_1_consumer/Cargo.toml"
scratch_root=${TMPDIR:-/tmp}
patch_config=$(mktemp "$scratch_root/moenarch-foundation-wave-1-patches.XXXXXX.toml")

cleanup() {
  case "$patch_config" in
    "$scratch_root"/moenarch-foundation-wave-1-patches.*.toml)
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
  write_patch moenarch-runtime-onnx crates/runtime/runtime-onnx
  write_patch moenarch-jobs-core crates/jobs/jobs-core
  write_patch moenarch-math-geometry-2d crates/math/math-geometry-2d
  write_patch moenarch-numbers-core crates/data/numbers-core
  write_patch moenarch-tensor-data crates/data/tensor-data
  write_patch moenarch-vector-analysis-core crates/vector/vector-analysis-core
  write_patch moenarch-data-inversion-core crates/data/data-inversion-core
  write_patch moenarch-model-runtime crates/runtime/model-runtime
  write_patch moenarch-math-linear crates/math/math-linear
  write_patch moenarch-math-signal-core crates/math/math-signal-core
  write_patch moenarch-vector-analysis-index crates/vector/vector-analysis-index
  write_patch moenarch-math-sparse-data crates/math/math-sparse-data
} >"$patch_config"

export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$repository_root/target"}
cargo check \
  --locked \
  --manifest-path "$consumer_manifest" \
  --config "$patch_config"

echo "foundation wave 1 candidate consumer passed"
