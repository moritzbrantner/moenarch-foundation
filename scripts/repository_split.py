#!/usr/bin/env python3
"""Shared helpers for the moenarch-foundation ownership and release validators."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OWNERSHIP_PATH = ROOT / "docs/repository-split/package-ownership.json"
RELEASE_PLAN_PATH = ROOT / "docs/repository-split/release-plan.json"
SOURCE_REPOSITORY = "moritzbrantner/rust-packages"
DESTINATION_REPOSITORY = "moritzbrantner/moenarch-foundation"
PHASE_A_BASELINE = "d032ad2890c1df3c6a5b9eff024562f00d017fce"
EXTRACTION_SHA = "364627c233b314807ba4f21298ada4cf63333bed"


def load_json(path: Path) -> dict:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def cargo_metadata(root: Path = ROOT) -> dict:
    completed = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout)


def inside_root(root: Path, value: str, base: Path | None = None) -> Path | None:
    root = root.resolve()
    candidate = ((base or root) / value).resolve()
    try:
        candidate.relative_to(root)
    except ValueError:
        return None
    return candidate


def ownership_records(document: dict) -> list[dict]:
    records = document.get("packages", [])
    return records if isinstance(records, list) else []


def main() -> int:
    if sys.argv[1:] != ["--harness-audit"]:
        print("usage: repository_split.py --harness-audit", file=sys.stderr)
        return 2
    codex_home = Path(os.environ.get("CODEX_HOME", Path.home() / ".codex"))
    harness = (
        codex_home
        / "skills/moenarch-verification-harness/scripts/verification_harness.py"
    )
    requirements = ROOT / ".agent-loop/verification/requirements.json"
    return subprocess.run(
        [
            sys.executable,
            str(harness),
            "audit",
            "--repo-root",
            str(ROOT),
            "--requirements-bundle",
            str(requirements),
            "--json",
        ],
        cwd=ROOT,
        check=False,
    ).returncode


if __name__ == "__main__":
    raise SystemExit(main())
