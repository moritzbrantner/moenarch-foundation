#!/usr/bin/env bash
# Candidate consumer gate for the issue #149 runtime-onnx 0.1.2 release.
#
# Runs a registry-shaped consumer of the exact candidate source with no usable
# ONNX Runtime dylib. It must exit with a typed RuntimeUnavailable error within
# the timeout; published 0.1.1 (ort 2.0.0-rc.12) hung forever here. All other
# dependencies, including ort 2.0.0-rc.15, resolve from crates.io.
set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
consumer_manifest="$repository_root/scripts/fixtures/runtime_onnx_0_1_2_consumer/Cargo.toml"
scratch_root=${TMPDIR:-/tmp}
patch_config=$(mktemp "$scratch_root/moenarch-runtime-onnx-0-1-2-patches.XXXXXX.toml")

cleanup() {
  case "$patch_config" in
    "$scratch_root"/moenarch-runtime-onnx-0-1-2-patches.*.toml)
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
    moenarch-runtime-onnx "$repository_root" crates/runtime/runtime-onnx
} >"$patch_config"

export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$repository_root/target"}
cargo build \
  --locked \
  --manifest-path "$consumer_manifest" \
  --config "$patch_config"
# Build first so the timeout bounds only the load attempt, not compilation.
timeout 120 cargo run \
  --locked \
  --quiet \
  --manifest-path "$consumer_manifest" \
  --config "$patch_config"

echo "runtime-onnx 0.1.2 candidate consumer passed"
