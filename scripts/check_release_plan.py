#!/usr/bin/env python3
"""Validate the exact, intentionally non-publishing bootstrap release plan."""

from __future__ import annotations

import argparse
import sys
from collections import Counter
from pathlib import Path

from repository_split import (
    DESTINATION_REPOSITORY,
    EXTRACTION_SHA,
    OWNERSHIP_PATH,
    RELEASE_PLAN_PATH,
    ROOT,
    cargo_metadata,
    load_json,
)


def validate(plan: dict, ownership: dict, metadata: dict, root: Path = ROOT) -> list[str]:
    errors: list[str] = []
    if plan.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    if plan.get("repository") != DESTINATION_REPOSITORY:
        errors.append("wrong repository")
    if plan.get("source_sha") != EXTRACTION_SHA:
        errors.append("source_sha must match extraction SHA")
    if plan.get("publication_authorized") is not False:
        errors.append("bootstrap plan must explicitly deny publication")
    records = ownership.get("packages", [])
    owned = {record.get("current_package_name"): record for record in records}
    packages = plan.get("packages")
    if not isinstance(packages, list):
        return errors + ["packages must be a list"]
    names = [package.get("name") for package in packages]
    duplicates = sorted(name for name, count in Counter(names).items() if count > 1)
    if duplicates:
        errors.append("duplicate package names: " + ", ".join(duplicates))
    if len(packages) != 60 or set(names) != set(owned):
        errors.append("release plan must name all and only the 60 owned packages")
    metadata_names = {package["name"] for package in metadata.get("packages", [])}
    if set(names) != metadata_names:
        errors.append("release plan does not match Cargo metadata")
    for package in packages:
        name = package.get("name")
        record = owned.get(name, {})
        if package.get("owner") != DESTINATION_REPOSITORY:
            errors.append(f"{name}: wrong owner")
        if package.get("publish") is not False:
            errors.append(f"{name}: publication is not authorized")
        if package.get("new_version") != package.get("old_version"):
            errors.append(f"{name}: nonpublishing plan must retain version")
        if package.get("expected_tag") is not None:
            errors.append(f"{name}: nonpublishing plan must not declare a tag")
        if package.get("manifest_path") != record.get("manifest_path"):
            errors.append(f"{name}: manifest_path differs from ownership")
        manifest = (root / str(package.get("manifest_path"))).resolve()
        try:
            manifest.relative_to(root.resolve())
        except ValueError:
            errors.append(f"{name}: manifest_path escapes repository")
        if not manifest.is_file():
            errors.append(f"{name}: manifest_path does not exist")
    order = plan.get("dependency_order")
    if not isinstance(order, list) or len(order) != len(set(order)) or set(order) != set(names):
        errors.append("dependency_order must contain each package exactly once")
    positions = {name: index for index, name in enumerate(order or [])}
    for package in packages:
        for dependency in package.get("release_dependencies", []):
            if dependency not in positions or positions[dependency] >= positions.get(package.get("name"), -1):
                errors.append(f"wrong dependency order: {dependency} must precede {package.get('name')}")
    if plan.get("expected_tags") != []:
        errors.append("nonpublishing plan must have no expected tags")
    if plan.get("release_issue") is not None:
        errors.append("nonpublishing plan must not claim a release issue")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("plan", nargs="?", type=Path, default=RELEASE_PLAN_PATH)
    args = parser.parse_args()
    errors = validate(load_json(args.plan), load_json(OWNERSHIP_PATH), cargo_metadata())
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print("release plan passes: 60 packages retained at source versions; publication is not authorized")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
