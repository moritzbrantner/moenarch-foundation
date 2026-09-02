#!/usr/bin/env python3
"""Shared helpers for the moenarch-foundation ownership and release validators."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OWNERSHIP_PATH = ROOT / "docs/repository-split/package-ownership.json"
POST_EXTRACTION_OWNERSHIP_PATH = (
    ROOT / "docs/repository-split/post-extraction-package-ownership.json"
)
RELEASE_PLAN_PATH = ROOT / "docs/repository-split/release-plan.json"
POST_EXTRACTION_RELEASE_PLAN_PATH = (
    ROOT / "docs/repository-split/post-extraction-release-plan.json"
)
SOURCE_REPOSITORY = "moritzbrantner/rust-packages"
DESTINATION_REPOSITORY = "moritzbrantner/moenarch-foundation"
PHASE_A_BASELINE = "d032ad2890c1df3c6a5b9eff024562f00d017fce"
EXTRACTION_SHA = "364627c233b314807ba4f21298ada4cf63333bed"
SOURCE_OWNERSHIP_RECORDS_SHA256 = (
    "6d1ae73c470e4e6adaf83705c315e47faa9189db5ce6ab0541c8b711305b9540"
)
POST_EXTRACTION_PACKAGE_NAMES = frozenset(
    {"moenarch-corpus-core", "moenarch-math-geometry-3d", "moenarch-math-probability", "moenarch-priority-queue"}
)
POST_EXTRACTION_RECORDS_SHA256 = (
    "4e7225483ac8d17388df5973b48ff2670c5726ed5f7a166a585c3ab4bdf6410d"
)


def _load_json_file(path: Path) -> dict:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def _extend_list(document: dict, extension: dict, key: str) -> None:
    values = extension.get(key, [])
    if values:
        document.setdefault(key, []).extend(values)


def load_json(path: Path) -> dict:
    """Load JSON plus explicit post-extraction metadata for canonical plans."""

    document = _load_json_file(path)
    resolved = path.resolve()
    if resolved == OWNERSHIP_PATH.resolve() and POST_EXTRACTION_OWNERSHIP_PATH.is_file():
        extension = _load_json_file(POST_EXTRACTION_OWNERSHIP_PATH)
        _extend_list(document, extension, "packages")
    elif (
        resolved == RELEASE_PLAN_PATH.resolve()
        and POST_EXTRACTION_RELEASE_PLAN_PATH.is_file()
    ):
        extension = _load_json_file(POST_EXTRACTION_RELEASE_PLAN_PATH)
        _extend_list(document, extension, "packages")
        _extend_list(document, extension, "dependency_order")
    return document


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


def ownership_records_sha256(document: dict) -> str:
    records = sorted(
        ownership_records(document),
        key=lambda record: str(record.get("current_package_name")),
    )
    encoded = json.dumps(records, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def records_named(document: dict, names: frozenset[str]) -> list[dict]:
    return [
        record
        for record in ownership_records(document)
        if record.get("current_package_name") in names
    ]


def records_except_named(document: dict, names: frozenset[str]) -> list[dict]:
    return [
        record
        for record in ownership_records(document)
        if record.get("current_package_name") not in names
    ]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--harness-audit", action="store_true")
    parser.add_argument("--base-ref", required=True)
    args = parser.parse_args()
    if not args.harness_audit:
        parser.error("--harness-audit is required")
    resolved_base = subprocess.run(
        ["git", "rev-parse", "--verify", f"{args.base_ref}^{{commit}}"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if resolved_base.returncode != 0:
        parser.error(f"--base-ref does not resolve to a commit: {args.base_ref}")
    base_sha = resolved_base.stdout.strip()
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
            "--base-ref",
            base_sha,
            "--requirements-bundle",
            str(requirements),
            "--json",
        ],
        cwd=ROOT,
        check=False,
    ).returncode


if __name__ == "__main__":
    raise SystemExit(main())
