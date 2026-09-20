"""Exercise the blocking benchmark driver with a controlled Cargo process."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from repository_split import ROOT


class BenchmarkSmokeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        (self.root / "Cargo.lock").write_text("# fixture lockfile\n")
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel="1.95.0"\n')
        self.paths = [
            "crates/vector/vector-analysis-core/benches/performance_smoke.rs",
            "crates/data/numbers-core/benches/performance_smoke.rs",
            "crates/math/math-geometry-3d/benches/performance_smoke.rs",
        ]
        for path in self.paths:
            target = self.root / path
            target.parent.mkdir(parents=True)
            target.write_text("// fixed workload\n")
        (self.root / "scripts").mkdir()
        shutil.copy2(ROOT / "scripts/benchmark-smoke.sh", self.root / "scripts")
        cargo = self.bin / "cargo"
        cargo.write_text("""#!/usr/bin/env python3
import json, os, sys
if sys.argv[1:] == ['-V']:
    print('cargo fixture')
    sys.exit(0)
with open(os.environ['BENCH_CALLS'], 'a') as stream:
    stream.write(' '.join(sys.argv[1:]) + '\\n')
with open(os.environ['BENCH_CALLS'] + '.env', 'a') as stream:
    stream.write(json.dumps({key: os.environ.get(key) for key in ['CARGO_TARGET_DIR', 'IAI_CALLGRIND_HOME']}) + '\\n')
if os.environ.get('BENCH_FAIL', '') in sys.argv and os.environ.get('BENCH_FAIL'):
    print('simulated instruction-count regression', file=sys.stderr)
    sys.exit(17)
""")
        cargo.chmod(0o755)
        for name in ["valgrind", "rustc"]:
            tool = self.bin / name
            tool.write_text("#!/bin/sh\necho fixture-version\n")
            tool.chmod(0o755)
        self.calls = self.root / "calls.log"
        self.env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}", BENCH_CALLS=str(self.calls))
        self.git("init", "-q")
        self.git("add", ".")
        self.git("-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false", "commit", "-qm", "baseline")
        self.base = self.git("rev-parse", "HEAD").strip()

    def git(self, *args: str) -> str:
        return subprocess.check_output(["git", *args], cwd=self.root, text=True, stderr=subprocess.STDOUT)

    def run_gate(self, base: str = "", fail: str = "") -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["bash", "scripts/benchmark-smoke.sh"], cwd=self.root,
            env=dict(self.env, PERF_BASE_SHA=base, BENCH_FAIL=fail),
            capture_output=True, text=True, timeout=30,
        )

    def test_every_suite_compares_candidate_against_its_baseline(self) -> None:
        result = self.run_gate(self.base)
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls.read_text().splitlines()
        self.assertEqual(len(calls), 6)
        for index, package in enumerate(["moenarch-vector-analysis-core", "moenarch-numbers-core", "moenarch-math-geometry-3d"]):
            self.assertIn(f"-p {package}", calls[index * 2])
            self.assertIn("--save-baseline=pr_base", calls[index * 2])
            self.assertIn(f"-p {package}", calls[index * 2 + 1])
            self.assertIn("--baseline=pr_base", calls[index * 2 + 1])

    def test_initial_adoption_seeds_every_suite(self) -> None:
        result = self.run_gate()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls.read_text().splitlines()
        self.assertEqual(len(calls), 3)
        self.assertTrue(all("--save-baseline=seed" in call for call in calls))

    def test_builds_are_isolated_while_measurements_are_shared(self) -> None:
        result = self.run_gate(self.base)
        self.assertEqual(result.returncode, 0, result.stderr)
        environments = [json.loads(line) for line in Path(f"{self.calls}.env").read_text().splitlines()]
        for baseline, candidate in zip(environments[::2], environments[1::2]):
            self.assertNotEqual(baseline["CARGO_TARGET_DIR"], candidate["CARGO_TARGET_DIR"])
            self.assertIn(f"baseline-{self.base}", baseline["CARGO_TARGET_DIR"])
            self.assertIn(f"candidate-{self.base}", candidate["CARGO_TARGET_DIR"])
            self.assertTrue(baseline["IAI_CALLGRIND_HOME"])
            self.assertEqual(baseline["IAI_CALLGRIND_HOME"], candidate["IAI_CALLGRIND_HOME"])

    def test_candidate_regression_is_blocking(self) -> None:
        result = self.run_gate(self.base, "--baseline=pr_base")
        self.assertEqual(result.returncode, 17, result.stdout + result.stderr)

    def test_failed_baseline_is_not_treated_as_initial_adoption(self) -> None:
        result = self.run_gate(self.base, "--save-baseline=pr_base")
        self.assertEqual(result.returncode, 17, result.stdout + result.stderr)

    def test_changed_workloads_fail_instead_of_hiding_regressions(self) -> None:
        (self.root / self.paths[0]).write_text("// cheaper workload\n")
        result = self.run_gate(self.base)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Incompatible workload", result.stderr)

    def test_invalid_baseline_is_not_silently_seeded(self) -> None:
        self.assertNotEqual(self.run_gate("missing-commit").returncode, 0)

    def test_incompatible_toolchains_are_not_compared(self) -> None:
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel="1.96.0"\n')
        result = self.run_gate(self.base)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Incompatible benchmark toolchains", result.stderr)


if __name__ == "__main__":
    unittest.main()
