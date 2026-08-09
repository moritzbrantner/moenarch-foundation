#!/usr/bin/env python3
"""Validate the destination's exact ownership and dependency boundary."""

from __future__ import annotations

import argparse
import re
import sys
from collections import Counter
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

from repository_split import (
    DESTINATION_REPOSITORY,
    EXTRACTION_SHA,
    OWNERSHIP_PATH,
    PHASE_A_BASELINE,
    ROOT,
    SOURCE_REPOSITORY,
    cargo_metadata,
    load_json,
)

FULL_SHA_RE = re.compile(r"^[0-9a-f]{40}$")


def immutable_git_source(source: str) -> bool:
    """Accept only an exact requested revision plus Cargo's resolved commit."""

    if not source.startswith("git+"):
        return True
    parsed = urlsplit(source[4:])
    query = parse_qs(parsed.query, keep_blank_values=True)
    revisions = query.get("rev", [])
    return (
        set(query) == {"rev"}
        and len(revisions) == 1
        and FULL_SHA_RE.fullmatch(revisions[0]) is not None
        and FULL_SHA_RE.fullmatch(parsed.fragment) is not None
    )


def validate(metadata: dict, ownership: dict, root: Path = ROOT) -> list[str]:
    errors: list[str] = []
    expected_header = {
        "schema_version": 1,
        "repository": DESTINATION_REPOSITORY,
        "source_repository": SOURCE_REPOSITORY,
        "phase_a_baseline": PHASE_A_BASELINE,
        "extraction_sha": EXTRACTION_SHA,
    }
    for key, expected in expected_header.items():
        if ownership.get(key) != expected:
            errors.append(f"{key} must be {expected!r}")
    records = ownership.get("packages")
    if not isinstance(records, list):
        return errors + ["packages must be a list"]
    names = [record.get("current_package_name") for record in records]
    owners = {
        record.get("current_package_name"): record.get("target_repository")
        for record in records
    }
    duplicates = sorted(name for name, count in Counter(names).items() if count > 1)
    if duplicates:
        errors.append("packages classified more than once: " + ", ".join(duplicates))
    cargo_packages = {package["name"]: package for package in metadata.get("packages", [])}
    missing = sorted(set(cargo_packages) - set(names))
    extra = sorted(set(names) - set(cargo_packages))
    if missing:
        errors.append("unclassified Cargo packages: " + ", ".join(missing))
    if extra:
        errors.append("ownership entries absent from cargo metadata: " + ", ".join(extra))
    if len(records) != 60:
        errors.append(f"ownership must contain exactly 60 packages, found {len(records)}")
    for record in records:
        name = record.get("current_package_name")
        if record.get("target_repository") != "moenarch-foundation":
            errors.append(f"{name}: wrong target repository")
        if record.get("intended_next_release_owner") != DESTINATION_REPOSITORY:
            errors.append(f"{name}: wrong release owner")
        manifest = record.get("manifest_path")
        if not isinstance(manifest, str):
            errors.append(f"{name}: missing manifest_path")
            continue
        path = (root / manifest).resolve()
        try:
            path.relative_to(root.resolve())
        except ValueError:
            errors.append(f"{name}: manifest_path escapes repository")
        if not path.is_file():
            errors.append(f"{name}: manifest_path does not exist")
        if record.get("package_kind") in {"CLI", "server", "WASM"}:
            wrapped = record.get("wrapped_library")
            if wrapped not in cargo_packages:
                errors.append(f"{name}: invalid wrapped_library {wrapped!r}")
    for package in cargo_packages.values():
        for dependency in package.get("dependencies", []):
            dependency_name = dependency.get("name")
            if (
                isinstance(dependency_name, str)
                and dependency_name.startswith("moenarch-")
                and dependency_name not in cargo_packages
            ):
                errors.append(
                    f"{package['name']}: out-of-inventory Moenarch dependency "
                    f"{dependency_name}"
                )
            dependency_owner = owners.get(dependency_name)
            if dependency_owner and dependency_owner != "moenarch-foundation":
                errors.append(
                    f"{package['name']}: forbidden foundation dependency on "
                    f"{dependency_name} owned by {dependency_owner}"
                )
            dep_path = dependency.get("path")
            if dep_path:
                try:
                    Path(dep_path).resolve().relative_to(root.resolve())
                except ValueError:
                    errors.append(f"{package['name']}: dependency path escapes repository")
            source = dependency.get("source") or ""
            if not immutable_git_source(source):
                errors.append(
                    f"{package['name']}: non-immutable Git dependency {source}"
                )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--metadata", type=Path)
    parser.add_argument("--ownership", type=Path, default=OWNERSHIP_PATH)
    args = parser.parse_args()
    metadata = load_json(args.metadata) if args.metadata else cargo_metadata()
    ownership = load_json(args.ownership)
    errors = validate(metadata, ownership)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print("repository boundaries pass: 60 uniquely owned foundation packages; no path escapes or moving Git dependencies")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
