#!/usr/bin/env python3
"""Validate the exact, intentionally non-publishing bootstrap release plan."""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
import subprocess
import tempfile
import tomllib
from collections import Counter
from pathlib import Path

from publish_release import CONTROL_REPAIR_SCRIPT_PATHS, ReleaseError, validate_manifest

from repository_split import (
    DESTINATION_REPOSITORY,
    EXTRACTION_SHA,
    OWNERSHIP_PATH,
    RELEASE_PLAN_PATH,
    ROOT,
    SOURCE_REPOSITORY,
    cargo_metadata,
    inside_root,
    load_json,
)

REQUIRED_CHECKS = {
    "cargo metadata --format-version 1 --no-deps",
    "cargo test --workspace --all-features",
    "cargo test --workspace --no-default-features",
    "cargo doc --workspace --no-deps",
    "cargo package -p <each-public-package> --locked",
    "python3 scripts/check_repository_boundaries.py --check",
    "python3 scripts/check_release_plan.py --check docs/repository-split/release-plan.json",
    "python3 -m unittest discover -s scripts -p 'test_*.py'",
    "python3 scripts/repository_split.py --harness-audit --base-ref <reviewed-base-sha>",
}

FOUNDATION_WAVE_1 = [
    ("moenarch-runtime-core", "0.2.1"),
    ("moenarch-runtime-onnx", "0.1.1"),
    ("moenarch-jobs-core", "0.1.2"),
    ("moenarch-math-geometry-2d", "0.1.1"),
    ("moenarch-numbers-core", "0.1.1"),
    ("moenarch-tensor-data", "0.1.1"),
    ("moenarch-vector-analysis-core", "0.1.1"),
    ("moenarch-data-inversion-core", "0.1.1"),
    ("moenarch-model-runtime", "0.1.1"),
    ("moenarch-math-linear", "0.1.1"),
    ("moenarch-math-signal-core", "0.1.1"),
    ("moenarch-vector-analysis-index", "0.1.1"),
    ("moenarch-math-sparse-data", "0.1.1"),
]
FOUNDATION_WAVE_1_PATH = "releases/foundation-wave-1.toml"
FOUNDATION_WAVE_1_VERSIONS = dict(FOUNDATION_WAVE_1)
FOUNDATION_WAVE_1_CONSUMER_CHECKS = [
    "bash scripts/check_foundation_wave_1_candidate_consumer.sh",
    *[
        f"cargo package -p {name} --locked --list"
        for name, _version in FOUNDATION_WAVE_1
    ],
]
FOUNDATION_WAVE_2 = [
    ("moenarch-graph-analysis-core", "0.1.0"),
    ("moenarch-math-statistics", "0.1.0"),
    ("moenarch-dense-data", "0.1.0"),
]
FOUNDATION_WAVE_2_PATH = "releases/foundation-wave-2.toml"
FOUNDATION_WAVE_2_VERSIONS = dict(FOUNDATION_WAVE_2)
FOUNDATION_AUDIO_CONTRACTS = [("moenarch-audio-contracts", "0.1.0")]
FOUNDATION_AUDIO_CONTRACTS_PATH = "releases/foundation-audio-contracts.toml"
FOUNDATION_AUDIO_CONTRACTS_VERSIONS = dict(FOUNDATION_AUDIO_CONTRACTS)


def manifest_hashes(root: Path, ownership: dict) -> dict[str, str]:
    paths = [root / "Cargo.toml"] + [
        root / record["manifest_path"] for record in ownership.get("packages", [])
    ]
    return {
        path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in paths
    }


def package_all(plan: dict, ownership: dict, root: Path = ROOT) -> list[str]:
    """Package every crate without publishing or mutating tracked manifests."""

    before = manifest_hashes(root, ownership)
    patch_lines = ["[patch.crates-io]"]
    records = {
        record["current_package_name"]: record
        for record in ownership.get("packages", [])
    }
    for name in sorted(records):
        crate = (root / records[name]["manifest_path"]).parent.resolve()
        patch_lines.append(f'"{name}" = {{ path = "{crate}" }}')
    failures: list[str] = []
    with tempfile.NamedTemporaryFile(mode="w", suffix=".toml") as config:
        config.write("\n".join(patch_lines) + "\n")
        config.flush()
        for name in plan.get("dependency_order", []):
            completed = subprocess.run(
                [
                    "cargo",
                    "package",
                    "-p",
                    name,
                    "--locked",
                    "--config",
                    config.name,
                ],
                cwd=root,
                check=False,
            )
            if completed.returncode:
                failures.append(name)
            else:
                print(f"PACKAGED {name}")
    after = manifest_hashes(root, ownership)
    if before != after:
        failures.append("tracked Cargo manifests changed during packaging")
    return failures


def package_release(manifest: dict, metadata: dict, root: Path = ROOT) -> list[str]:
    """Package only one manifest's selected crates with its candidate closure patched."""

    packages = manifest.get("packages", [])
    metadata_packages = {
        package["name"]: package for package in metadata.get("packages", [])
    }
    patch_names = {package["name"] for package in packages}
    patch_names.update(
        dependency
        for package in packages
        for dependency in package.get("dependencies", [])
    )
    patch_lines = ["[patch.crates-io]"]
    for name in sorted(patch_names):
        crate = Path(metadata_packages[name]["manifest_path"]).parent.resolve()
        patch_lines.append(f'"{name}" = {{ path = "{crate}" }}')
    failures: list[str] = []
    with tempfile.NamedTemporaryFile(mode="w", suffix=".toml") as config:
        config.write("\n".join(patch_lines) + "\n")
        config.flush()
        for package in packages:
            name = package["name"]
            completed = subprocess.run(
                [
                    "cargo",
                    "package",
                    "-p",
                    name,
                    "--locked",
                    "--registry",
                    "crates-io",
                    "--config",
                    config.name,
                ],
                cwd=root,
                check=False,
            )
            if completed.returncode:
                failures.append(name)
            else:
                print(f"PACKAGED {name}")
    return failures


def validate(plan: dict, ownership: dict, metadata: dict, root: Path = ROOT) -> list[str]:
    errors: list[str] = []
    if plan.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    if plan.get("repository") != DESTINATION_REPOSITORY:
        errors.append("wrong repository")
    if plan.get("active_release_owner") != SOURCE_REPOSITORY:
        errors.append("wrong active release owner")
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
    metadata_packages = {
        package["name"]: package for package in metadata.get("packages", [])
    }
    metadata_names = set(metadata_packages)
    if set(names) != metadata_names:
        errors.append("release plan does not match Cargo metadata")
    for package in packages:
        name = package.get("name")
        record = owned.get(name, {})
        if package.get("intended_next_release_owner") != DESTINATION_REPOSITORY:
            errors.append(f"{name}: wrong intended next release owner")
        if package.get("publish") is not False:
            errors.append(f"{name}: publication is not authorized")
        if package.get("new_version") != package.get("old_version"):
            errors.append(f"{name}: nonpublishing plan must retain version")
        actual_version = metadata_packages.get(name, {}).get("version")
        source_version = record.get("source_version")
        if (
            package.get("old_version") != source_version
            or package.get("new_version") != source_version
        ):
            errors.append(
                f"{name}: ownership source_version does not match the bootstrap plan"
            )
        authorized_workspace_versions = {source_version}
        wave_version = (
            FOUNDATION_WAVE_1_VERSIONS | FOUNDATION_WAVE_2_VERSIONS
        ).get(name)
        if wave_version is not None:
            authorized_workspace_versions.add(wave_version)
        if actual_version not in authorized_workspace_versions:
            errors.append(
                f"{name}: workspace version is not an authorized source or wave version "
                f"{actual_version!r}"
            )
        if package.get("expected_tag") is not None:
            errors.append(f"{name}: nonpublishing plan must not declare a tag")
        if package.get("manifest_path") != record.get("manifest_path"):
            errors.append(f"{name}: manifest_path differs from ownership")
        manifest = inside_root(root, str(package.get("manifest_path")))
        if manifest is None:
            errors.append(f"{name}: manifest_path escapes repository")
        elif not manifest.is_file():
            errors.append(f"{name}: manifest_path does not exist")
        actual_dependencies = {
            dependency["name"]
            for dependency in metadata_packages.get(name, {}).get("dependencies", [])
            if dependency.get("name") in metadata_names
            and dependency.get("kind") != "dev"
        }
        planned_dependencies = package.get("release_dependencies")
        if not isinstance(planned_dependencies, list):
            errors.append(f"{name}: release_dependencies must be a list")
        elif set(planned_dependencies) != actual_dependencies:
            errors.append(
                f"{name}: release_dependencies do not match workspace metadata"
            )
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
    required_checks = plan.get("required_checks")
    if not isinstance(required_checks, list) or set(required_checks) != REQUIRED_CHECKS:
        errors.append("required_checks must match the complete bootstrap gate set")
    return errors


def _relative_plan_path(path: Path, root: Path) -> str:
    candidate = path if path.is_absolute() else root / path
    try:
        return candidate.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def validate_release_manifest(
    manifest: dict,
    ownership: dict,
    metadata: dict,
    path: Path,
    root: Path = ROOT,
) -> list[str]:
    """Validate checked publisher-schema TOML without performing release effects."""

    errors: list[str] = []
    relative_path = _relative_plan_path(path, root)
    if manifest.get("repository") != DESTINATION_REPOSITORY:
        errors.append("wrong repository")
    if relative_path == FOUNDATION_WAVE_1_PATH:
        expected_order = [name for name, _ in FOUNDATION_WAVE_1]
        expected_versions = FOUNDATION_WAVE_1_VERSIONS
        if manifest.get("issue") != 8:
            errors.append("foundation wave 1 must bind destination issue 8")
        if manifest.get("dependency_order") != expected_order:
            errors.append("foundation wave 1 package order does not match its release contract")
        actual_versions = {
            package.get("name"): package.get("version")
            for package in manifest.get("packages", [])
            if isinstance(package, dict)
        }
        if actual_versions != expected_versions:
            errors.append("foundation wave 1 package versions do not match its release contract")
        if manifest.get("required_consumer_checks") != FOUNDATION_WAVE_1_CONSUMER_CHECKS:
            errors.append(
                "foundation wave 1 consumer checks do not match its release contract"
            )
    elif relative_path == FOUNDATION_WAVE_2_PATH:
        expected_order = [name for name, _ in FOUNDATION_WAVE_2]
        if manifest.get("issue") != 13:
            errors.append("foundation wave 2 must bind destination issue 13")
        if manifest.get("dependency_order") != expected_order:
            errors.append("foundation wave 2 package order does not match its release contract")
        actual_versions = {
            package.get("name"): package.get("version")
            for package in manifest.get("packages", [])
            if isinstance(package, dict)
        }
        if actual_versions != FOUNDATION_WAVE_2_VERSIONS:
            errors.append("foundation wave 2 package versions do not match its release contract")
        if manifest.get("required_consumer_checks") != []:
            errors.append("foundation wave 2 must not require consumer checks")
    elif relative_path == FOUNDATION_AUDIO_CONTRACTS_PATH:
        expected_order = [name for name, _ in FOUNDATION_AUDIO_CONTRACTS]
        if manifest.get("issue") != 17:
            errors.append("foundation audio-contracts must bind destination issue 17")
        if manifest.get("dependency_order") != expected_order:
            errors.append(
                "foundation audio-contracts package order does not match its release contract"
            )
        actual_versions = {
            package.get("name"): package.get("version")
            for package in manifest.get("packages", [])
            if isinstance(package, dict)
        }
        if actual_versions != FOUNDATION_AUDIO_CONTRACTS_VERSIONS:
            errors.append(
                "foundation audio-contracts package version does not match its release contract"
            )
        if manifest.get("required_consumer_checks") != []:
            errors.append("foundation audio-contracts must not require consumer checks")
    try:
        validate_manifest(root, manifest, metadata, ownership)
    except ReleaseError as error:
        errors.append(str(error))
    return errors


def validate_control_binding(
    manifest: dict,
    path: Path,
    head: str,
    source_is_ancestor: bool,
    changed_paths: list[str],
    root: Path = ROOT,
    *,
    repair_is_ancestor: bool | None = None,
    repair_changed_paths: list[str] | None = None,
) -> list[str]:
    """Validate the destination's source/control two-commit binding."""

    errors: list[str] = []
    source_sha = manifest.get("source_sha")
    if not isinstance(source_sha, str) or re.fullmatch(r"[0-9a-f]{40}", source_sha) is None:
        return ["release manifest source_sha must be a full lowercase commit SHA"]
    if not re.fullmatch(r"[0-9a-f]{40}", head):
        errors.append("release control head must be a full lowercase commit SHA")
    manifest_path = _relative_plan_path(path, root)
    repair_source_sha = manifest.get("repair_source_sha")
    if repair_source_sha is None:
        if source_sha == head or not source_is_ancestor:
            errors.append("source_sha must be an ancestor of the release control head")
        if changed_paths != [manifest_path]:
            errors.append(
                "release control head must differ from source_sha only by its manifest"
            )
        return errors

    if (
        not isinstance(repair_source_sha, str)
        or re.fullmatch(r"[0-9a-f]{40}", repair_source_sha) is None
        or repair_source_sha in {source_sha, head}
        or not source_is_ancestor
        or repair_is_ancestor is not True
    ):
        errors.append(
            "release manifest control repair must be bound between source_sha and control head"
        )
    expected_repair_paths = CONTROL_REPAIR_SCRIPT_PATHS | {manifest_path}
    if set(changed_paths) != expected_repair_paths:
        errors.append("release manifest control repair changed outside its fixed surface")
    if repair_changed_paths != [manifest_path]:
        errors.append(
            "release control head must differ from repair_source_sha only by its manifest"
        )
    return errors


def _load_toml(path: Path) -> dict:
    try:
        document = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise ValueError(f"cannot load TOML release manifest: {error}") from error
    if not isinstance(document, dict):
        raise ValueError("TOML release manifest must be a table")
    return document


def _git_control_binding(manifest: dict, path: Path, root: Path) -> list[str]:
    source_sha = manifest.get("source_sha")
    if not isinstance(source_sha, str) or re.fullmatch(r"[0-9a-f]{40}", source_sha) is None:
        return ["release manifest source_sha must be a full lowercase commit SHA"]
    head = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()
    repair_source_sha = manifest.get("repair_source_sha")
    control_source_sha = (
        repair_source_sha if isinstance(repair_source_sha, str) else source_sha
    )
    source_target = control_source_sha if repair_source_sha is not None else head
    ancestor = subprocess.run(
        ["git", "merge-base", "--is-ancestor", source_sha, source_target],
        cwd=root,
        check=False,
    ).returncode == 0
    changed_result = subprocess.run(
        ["git", "diff", "--name-only", source_sha, source_target],
        cwd=root,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    changed = changed_result.stdout.splitlines() if changed_result.returncode == 0 else []
    if repair_source_sha is None:
        return validate_control_binding(manifest, path, head, ancestor, changed, root)
    repair_ancestor = subprocess.run(
        ["git", "merge-base", "--is-ancestor", control_source_sha, head],
        cwd=root,
        check=False,
    ).returncode == 0
    repair_changed_result = subprocess.run(
        ["git", "diff", "--name-only", control_source_sha, head],
        cwd=root,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    repair_changed = (
        repair_changed_result.stdout.splitlines()
        if repair_changed_result.returncode == 0
        else []
    )
    return validate_control_binding(
        manifest,
        path,
        head,
        ancestor,
        changed,
        root,
        repair_is_ancestor=repair_ancestor,
        repair_changed_paths=repair_changed,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--package-all", action="store_true")
    parser.add_argument("--package-release", action="store_true")
    parser.add_argument("plan", nargs="?", type=Path, default=RELEASE_PLAN_PATH)
    args = parser.parse_args()
    ownership = load_json(OWNERSHIP_PATH)
    metadata = cargo_metadata()
    if args.plan.suffix == ".toml":
        try:
            plan = _load_toml(args.plan)
        except ValueError as error:
            print(f"error: {error}", file=sys.stderr)
            return 1
        errors = validate_release_manifest(plan, ownership, metadata, args.plan)
        if not errors:
            errors.extend(_git_control_binding(plan, args.plan, ROOT))
    elif args.plan.suffix == ".json":
        plan = load_json(args.plan)
        errors = validate(plan, ownership, metadata)
    else:
        print("error: release plan must be JSON or TOML", file=sys.stderr)
        return 1
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    if args.package_all and args.package_release:
        print("error: choose only one packaging mode", file=sys.stderr)
        return 1
    if args.package_all:
        if args.plan.suffix != ".json":
            print("error: --package-all requires the bootstrap JSON inventory", file=sys.stderr)
            return 1
        failures = package_all(plan, ownership)
        if failures:
            print("error: packaging failed: " + ", ".join(failures), file=sys.stderr)
            return 1
        print("package verification passes: 60 packages; tracked manifest hashes unchanged")
    elif args.package_release:
        if args.plan.suffix != ".toml":
            print("error: --package-release requires a TOML manifest", file=sys.stderr)
            return 1
        failures = package_release(plan, metadata)
        if failures:
            print(
                "error: release packaging failed: " + ", ".join(failures),
                file=sys.stderr,
            )
            return 1
        print(f"release package verification passes: {len(plan['packages'])} packages")
    elif args.plan.suffix == ".toml":
        print(
            f"checked release manifest passes: {len(plan['packages'])} packages; "
            "source/control binding verified"
        )
    else:
        print("release plan passes: 60 packages retained at source versions; publication is not authorized")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
