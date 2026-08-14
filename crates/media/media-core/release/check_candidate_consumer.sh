#!/usr/bin/env bash
set -euo pipefail

studio_sha=93ceeb1c43764be9d31c35258145604559e0a0aa
monolith_sha=c11c945fc13e532588f768f982c3c80a46ab477c
expected_lock_diff=79c3a0869389b29b1cf7f51c8d02d49db42f3be31d256d8cb4c9b282b89defb0
repository_root=$(git rev-parse --show-toplevel)
media_root="$repository_root/crates/media/media-core"
scratch_root=${MOENARCH_RELEASE_SCRATCH_ROOT:-${TMPDIR:-"$repository_root/target"}}
mkdir -p -- "$scratch_root"
scratch_root=$(cd -- "$scratch_root" && pwd -P)
scratch=$(mktemp -d "$scratch_root/moenarch-media-core-consumer.XXXXXX")

cleanup() {
  case "$scratch" in
    "$scratch_root"/moenarch-media-core-consumer.*) rm -rf -- "$scratch" ;;
    *) echo "refusing to remove unexpected scratch path: $scratch" >&2 ;;
  esac
}
trap cleanup EXIT

checkout_exact() {
  local repository=$1
  local sha=$2
  local destination=$3
  git init --quiet "$destination"
  git -C "$destination" remote add origin "$repository"
  git -C "$destination" fetch --quiet --depth=1 origin "$sha"
  git -C "$destination" checkout --quiet --detach FETCH_HEAD
  test "$(git -C "$destination" rev-parse HEAD)" = "$sha"
  test -z "$(git -C "$destination" status --porcelain)"
}

checkout_exact \
  https://github.com/moritzbrantner/video-analysis-studio.git \
  "$studio_sha" "$scratch/video-analysis-studio"
checkout_exact \
  https://github.com/moritzbrantner/rust-packages.git \
  "$monolith_sha" "$scratch/rust-packages"

test -f "$scratch/rust-packages/crates/text/text-analysis-features/Cargo.toml"
test -f "$scratch/rust-packages/crates/text/text-analysis-transcription/Cargo.toml"
mkdir -p "$scratch/rust-packages/crates/media"
cp -a "$media_root" "$scratch/rust-packages/crates/media/media-core"
git -C "$scratch/rust-packages" apply --check \
  "$media_root/release/rust-packages-media-overlay.patch"
git -C "$scratch/rust-packages" apply \
  "$media_root/release/rust-packages-media-overlay.patch"
git -C "$scratch/video-analysis-studio" apply --check \
  "$media_root/release/video-analysis-studio.patch"
git -C "$scratch/video-analysis-studio" apply \
  "$media_root/release/video-analysis-studio.patch"

git -C "$scratch/rust-packages" diff --check
git -C "$scratch/video-analysis-studio" diff --check
diff -ru --exclude=release "$media_root" \
  "$scratch/rust-packages/crates/media/media-core"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repository_root/target}"
cargo check --manifest-path "$scratch/video-analysis-studio/Cargo.toml" \
  -p studio-core
lock_diff=$(git -C "$scratch/video-analysis-studio" diff --no-ext-diff --no-color \
  -- Cargo.lock | sha256sum | cut -d' ' -f1)
test "$lock_diff" = "$expected_lock_diff"
cargo check --manifest-path "$scratch/video-analysis-studio/Cargo.toml" \
  -p studio-core --locked

overlay_diff=$(git -C "$scratch/rust-packages" diff --no-ext-diff --no-color -- \
  Cargo.toml crates/video/video-analysis-core/Cargo.toml \
  crates/video/video-analysis-core/src/lib.rs | sha256sum | cut -d' ' -f1)
studio_diff=$(git -C "$scratch/video-analysis-studio" diff --no-ext-diff --no-color -- \
  Cargo.toml crates/studio-core/Cargo.toml crates/studio-core/src/red_cars.rs \
  crates/studio-core/src/youtube.rs | sha256sum | cut -d' ' -f1)
echo "consumer gate passed: studio=$studio_sha monolith=$monolith_sha"
echo "overlay diff sha256: $overlay_diff"
echo "studio diff sha256: $studio_diff"
echo "lock diff sha256: $lock_diff"
