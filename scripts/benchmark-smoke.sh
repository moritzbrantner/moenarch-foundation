#!/usr/bin/env bash
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

artifact_dir="$root/.artifacts/performance-smoke"
target_dir="$root/target/performance-smoke"
candidate_target_dir="$target_dir/candidate-$(git rev-parse HEAD)"
measurement_dir="$target_dir/measurements"
mkdir -p "$artifact_dir" "$target_dir"

{
  printf 'repository=%s\n' "$(git remote get-url origin 2>/dev/null || printf unknown)"
  printf 'candidate=%s\n' "$(git rev-parse HEAD)"
  printf 'baseline=%s\n' "${PERF_BASE_SHA:-none}"
  printf 'rustflags=%s\n' "${RUSTFLAGS:-}"
  printf 'cargo_target_dir=%s\n' "$target_dir"
  printf 'candidate_target_dir=%s\n' "$candidate_target_dir"
  printf 'iai_callgrind_home=%s\n' "$measurement_dir"
  printf 'cargo_lock_sha256=%s\n' "$(sha256sum Cargo.lock | cut -d' ' -f1)"
  printf 'profile=bench (Cargo optimized benchmark profile)\n'
  rustc -vV
  cargo -V
  valgrind --version
  printf 'iai-callgrind-runner=%s\n' '0.16.1'
  uname -srm
} > "$artifact_dir/fingerprint.txt"

# Each suite has independent Callgrind groups and evidence logs.
packages=(moenarch-vector-analysis-core moenarch-numbers-core moenarch-math-geometry-3d)
bench_paths=(
  crates/vector/vector-analysis-core/benches/performance_smoke.rs
  crates/data/numbers-core/benches/performance_smoke.rs
  crates/math/math-geometry-3d/benches/performance_smoke.rs
)
base_sha="${PERF_BASE_SHA:-}"
baseline_dir=""
if [[ -n "$base_sha" ]]; then
  base_sha="$(git rev-parse --verify "$base_sha^{commit}")"
  baseline_target_dir="$target_dir/baseline-$base_sha"
  printf 'baseline_target_dir=%s\n' "$baseline_target_dir" >> "$artifact_dir/fingerprint.txt"
  worktree_parent="$(mktemp -d)"
  baseline_dir="$worktree_parent/base"
  cleanup() {
    git worktree remove --force "$baseline_dir"
    rmdir "$worktree_parent"
  }
  git worktree add --detach "$baseline_dir" "$base_sha" >/dev/null
  trap cleanup EXIT
  if ! cmp -s rust-toolchain.toml "$baseline_dir/rust-toolchain.toml"; then
    printf '%s\n' 'Incompatible benchmark toolchains; review an explicit baseline transition.' >&2
    exit 1
  fi
  sha256sum "$baseline_dir/Cargo.lock" > "$artifact_dir/baseline-lock.sha256"
fi

for index in "${!packages[@]}"; do
  package="${packages[$index]}"
  bench_path="${bench_paths[$index]}"
  suite_artifacts="$artifact_dir/$package"
  mkdir -p "$suite_artifacts"
  sha256sum "$bench_path" > "$suite_artifacts/workload.sha256"
  if [[ -n "$base_sha" ]] && git cat-file -e "$base_sha:$bench_path" 2>/dev/null; then
    # Changed workloads cannot be compared as if they were equivalent.
    git show "$base_sha:$bench_path" > "$suite_artifacts/baseline-workload.rs"
    if ! cmp -s "$bench_path" "$suite_artifacts/baseline-workload.rs"; then
      printf 'Incompatible workload for %s; review an explicit baseline transition.\n' "$package" >&2
      exit 1
    fi
    (
      cd "$baseline_dir"
      # Cargo artifacts must never cross source worktrees: checkout timestamps
      # can otherwise make a candidate reuse compiled baseline dependencies.
      CARGO_TARGET_DIR="$baseline_target_dir" IAI_CALLGRIND_HOME="$measurement_dir" \
        cargo bench --locked -p "$package" --bench performance_smoke -- --save-baseline=pr_base
    ) 2>&1 | tee "$suite_artifacts/baseline.log"
    baseline_args=(--baseline=pr_base)
  else
    printf '%s\n' 'No historical workload exists; seed its first instruction-count baseline.' \
      | tee "$suite_artifacts/baseline.log"
    baseline_args=(--save-baseline=seed)
  fi
  CARGO_TARGET_DIR="$candidate_target_dir" IAI_CALLGRIND_HOME="$measurement_dir" \
    cargo bench --locked -p "$package" --bench performance_smoke -- "${baseline_args[@]}" \
    2>&1 | tee "$suite_artifacts/candidate.log"
done
