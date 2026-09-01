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
    SOURCE_OWNERSHIP_RECORDS_SHA256,
    POST_EXTRACTION_PACKAGE_NAMES,
    POST_EXTRACTION_RECORDS_SHA256,
    cargo_metadata,
    inside_root,
    load_json,
    ownership_records_sha256,
    records_except_named,
    records_named,
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
        "source_ownership_records_sha256": SOURCE_OWNERSHIP_RECORDS_SHA256,
    }
    for key, expected in expected_header.items():
        if ownership.get(key) != expected:
            errors.append(f"{key} must be {expected!r}")
    expected_authority = {
        "repository": DESTINATION_REPOSITORY,
        "responsibilities": ["source", "tests", "issues", "versions", "releases"],
    }
    if ownership.get("canonical_authority") != expected_authority:
        errors.append(
            "canonical authority must assign source/tests/issues/versions/releases "
            "to the destination"
        )
    if ownership.get("historical_source_role") != "compatibility-provenance-only":
        errors.append("historical source role must be compatibility-provenance-only")
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
    if len(records) != 62:
        errors.append(f"ownership must contain exactly 62 packages, found {len(records)}")
    source_document = {"packages": records_except_named(ownership, POST_EXTRACTION_PACKAGE_NAMES)}
    if ownership_records_sha256(source_document) != SOURCE_OWNERSHIP_RECORDS_SHA256:
        errors.append("source ownership records differ from the extraction inventory")
    additions_document = {"packages": records_named(ownership, POST_EXTRACTION_PACKAGE_NAMES)}
    if ownership_records_sha256(additions_document) != POST_EXTRACTION_RECORDS_SHA256:
        errors.append("post-extraction ownership additions differ from the approved inventory")
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
        path = inside_root(root, manifest)
        if path is None:
            errors.append(f"{name}: manifest_path escapes repository")
        elif not path.is_file():
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
    print("repository boundaries pass: 62 uniquely owned foundation packages; no path escapes or moving Git dependencies")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
