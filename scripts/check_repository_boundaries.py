#!/usr/bin/env python3
"""Validate the destination's exact ownership and dependency boundary."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
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
REPOSITORY_METADATA_PATH = ROOT / ".repository.toml"
LOCKFILE_PATH = ROOT / "Cargo.lock"
WORKSPACE_MANIFEST_PATH = ROOT / "Cargo.toml"
REPOSITORY_OWNER = "moritzbrantner"

# Capability repositories Foundation must never depend on, with the Cargo
# package-name prefixes they publish. They must stay declared as consumers in
# `.repository.toml`; application repositories are covered generically because
# every Git edge into a repository outside `depends_on` is rejected.
FORBIDDEN_UPSTREAM_REPOSITORIES = {
    "moritzbrantner/audio-analysis": ("moenarch-audio-analysis", "moenarch-audio-generation", "moenarch-audio-karaoke", "moenarch-audio-timed-text"),
    "moritzbrantner/visual-analysis": ("moenarch-image-analysis", "moenarch-video-analysis", "moenarch-vision"),
    "moritzbrantner/nlp-stack": ("moenarch-text-", "moenarch-nlp-"),
    "moritzbrantner/scenedetect-rs": ("scenedetect",),
}

# Capabilities Foundation must explicitly exclude (issue #108).
REQUIRED_EXCLUSIONS = frozenset(
    {
        "media/asr",
        "media/ocr",
        "media/scene-detection",
        "nlp/interpretation",
        "product/workflow",
        "corpus/policy",
        "interpretation/belief-semantics",
    }
)

GITHUB_REPOSITORY_RE = re.compile(
    r"^(?:git\+)?(?:https|ssh)://(?:[^@/]+@)?github\.com[/:]"
    r"(?P<owner>[A-Za-z0-9_.-]+)/(?P<repo>[A-Za-z0-9_.-]+?)(?:\.git)?/?(?:[?#].*)?$",
    re.IGNORECASE,
)


def immutable_git_source(source: str) -> bool:
    """Accept only Git dependencies requested at an exact commit revision.

    Cargo metadata reports a manifest `rev = "<sha>"` dependency as
    `...?rev=<sha>` while lockfile-derived source identifiers may additionally
    contain `#<resolved-sha>`. A full requested SHA is already immutable; when a
    resolved fragment is present, require it to name that same commit.
    """

    if not source.startswith("git+"):
        return True
    parsed = urlsplit(source[4:])
    query = parse_qs(parsed.query, keep_blank_values=True)
    revisions = query.get("rev", [])
    if set(query) != {"rev"} or len(revisions) != 1:
        return False

    revision = revisions[0]
    if FULL_SHA_RE.fullmatch(revision) is None:
        return False

    if not parsed.fragment:
        return True

    return parsed.fragment == revision


def normal_dependency_names(package: dict) -> set[str]:
    """Return packages linked through Cargo's normal/runtime dependency kind."""

    names: set[str] = set()
    for dependency in package.get("dependencies", []):
        if dependency.get("kind") not in {None, "normal"}:
            continue
        dependency_name = dependency.get("name")
        if isinstance(dependency_name, str):
            names.add(dependency_name)
    return names


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
    if len(records) != 65:
        errors.append(f"ownership must contain exactly 65 packages, found {len(records)}")
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
            else:
                package = cargo_packages.get(name)
                if package is not None and wrapped not in normal_dependency_names(package):
                    errors.append(
                        f"{name}: wrapped_library {wrapped!r} is not a normal dependency"
                    )
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


def load_toml(path: Path) -> dict:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def github_repository(source: str) -> str | None:
    """Return `owner/repo` for a GitHub Git source, or `None` otherwise."""

    match = GITHUB_REPOSITORY_RE.match(source)
    if match is None:
        return None
    return f"{match['owner']}/{match['repo']}".lower()


def capability_within(boundary: str, capability: str) -> bool:
    return capability == boundary or capability.startswith(f"{boundary}/")


def string_list(document: dict, key: str) -> list[str] | None:
    value = document.get(key)
    if isinstance(value, list) and all(isinstance(item, str) for item in value):
        return value
    return None


def validate_repository_declaration(declaration: dict) -> list[str]:
    """Validate the machine-readable purpose, exclusions, consumers, and Pages N/A."""

    errors: list[str] = []
    if declaration.get("schema_version") != 1:
        errors.append(".repository.toml: schema_version must be 1")
    if declaration.get("id") != DESTINATION_REPOSITORY:
        errors.append(f".repository.toml: id must be {DESTINATION_REPOSITORY!r}")
    summary = declaration.get("summary")
    if (
        not isinstance(summary, str)
        or not summary.strip()
        or "\n" in summary
        or not summary.endswith(".")
        or summary[:-1].count(". ") > 0
    ):
        errors.append(".repository.toml: summary must be a one-sentence purpose")

    depends_on = string_list(declaration, "depends_on")
    consumed_by = string_list(declaration, "consumed_by")
    if depends_on is None or consumed_by is None:
        errors.append(".repository.toml: depends_on and consumed_by must be string lists")
        depends_on, consumed_by = depends_on or [], consumed_by or []
    upstream = {value.lower() for value in depends_on}
    consumers = {value.lower() for value in consumed_by}
    for repository in sorted(upstream & consumers):
        errors.append(f".repository.toml: {repository} cannot be both a dependency and a consumer")
    for repository in sorted(FORBIDDEN_UPSTREAM_REPOSITORIES):
        if repository not in consumers:
            errors.append(f".repository.toml: {repository} must be declared in consumed_by")
        if repository in upstream:
            errors.append(f".repository.toml: {repository} must not be declared in depends_on")

    architecture = declaration.get("architecture")
    if not isinstance(architecture, dict):
        errors.append(".repository.toml: [architecture] is required")
    else:
        owns = string_list(architecture, "owns")
        must_not_own = string_list(architecture, "must_not_own")
        if not owns or must_not_own is None or string_list(architecture, "consumes") is None:
            errors.append(
                ".repository.toml: architecture owns, consumes, and must_not_own must be string lists"
            )
        else:
            for exclusion in sorted(REQUIRED_EXCLUSIONS - set(must_not_own)):
                errors.append(f".repository.toml: must_not_own must exclude {exclusion}")
            for capability in owns:
                for exclusion in must_not_own:
                    if capability_within(exclusion, capability):
                        errors.append(
                            f".repository.toml: owned capability {capability} is excluded by {exclusion}"
                        )

    bootstrap = declaration.get("bootstrap")
    if not isinstance(bootstrap, dict) or not bootstrap.get("command") or not bootstrap.get("check"):
        errors.append(".repository.toml: [bootstrap] command and check are required")

    pages = declaration.get("pages")
    if (
        not isinstance(pages, dict)
        or pages.get("status") != "not-applicable"
        or not isinstance(pages.get("reason"), str)
        or not pages["reason"].strip()
        or "command" in pages
        or "check" in pages
    ):
        errors.append(
            ".repository.toml: [pages] must be status = \"not-applicable\" with a reason "
            "and no build commands until a real Foundation site is introduced"
        )
    return errors


def forbidden_package_owner(name: str) -> str | None:
    for repository, prefixes in sorted(FORBIDDEN_UPSTREAM_REPOSITORIES.items()):
        if any(name.startswith(prefix) for prefix in prefixes):
            return repository
    return None


def upstream_source_error(origin: str, source: str, upstream: set[str], consumers: set[str]) -> str | None:
    repository = github_repository(source)
    if repository is None:
        return None
    if repository in consumers or repository in FORBIDDEN_UPSTREAM_REPOSITORIES:
        return f"{origin}: forbidden upstream Git dependency on consumer repository {repository}"
    owner = repository.split("/", 1)[0]
    if owner == REPOSITORY_OWNER and repository not in upstream:
        return (
            f"{origin}: Git dependency on {repository}, which is not a declared "
            "Foundation dependency (depends_on)"
        )
    return None


def validate_upstream_dependencies(
    metadata: dict,
    lockfile: dict,
    declaration: dict,
    workspace_manifest: dict | None = None,
    root: Path = ROOT,
) -> list[str]:
    """Reject Cargo edges from Foundation into capability or product repositories.

    Every dependency kind (normal, dev, and build) of every workspace package is
    checked, plus every package in `Cargo.lock` and every workspace `[patch]`
    entry, so transitive, test-only, and patched edges cannot hide an upward
    dependency.
    """

    errors: list[str] = []
    upstream = {value.lower() for value in string_list(declaration, "depends_on") or []}
    consumers = {value.lower() for value in string_list(declaration, "consumed_by") or []}
    workspace = {package["name"] for package in metadata.get("packages", [])}

    for package in metadata.get("packages", []):
        for dependency in package.get("dependencies", []):
            name = dependency.get("name")
            origin = f"{package['name']} -> {name}"
            if isinstance(name, str) and name not in workspace:
                owner = forbidden_package_owner(name)
                if owner is not None:
                    errors.append(f"{origin}: forbidden dependency on {owner} package")
            error = upstream_source_error(origin, dependency.get("source") or "", upstream, consumers)
            if error:
                errors.append(error)

    for package in lockfile.get("package", []):
        name = package.get("name")
        source = package.get("source")
        if not isinstance(name, str) or source is None:
            continue
        origin = f"Cargo.lock {name}"
        owner = forbidden_package_owner(name)
        if owner is not None:
            errors.append(f"{origin}: forbidden locked package from {owner}")
        elif name.startswith("moenarch-"):
            errors.append(f"{origin}: locked Moenarch package outside the Foundation inventory")
        error = upstream_source_error(origin, source, upstream, consumers)
        if error:
            errors.append(error)

    for registry, patches in (workspace_manifest or {}).get("patch", {}).items():
        for name, patch in patches.items():
            origin = f"[patch.{registry}] {name}"
            if not isinstance(patch, dict):
                continue
            path = patch.get("path")
            if isinstance(path, str) and inside_root(root, path) is None:
                errors.append(f"{origin}: patch path escapes repository")
            git = patch.get("git")
            if isinstance(git, str):
                error = upstream_source_error(origin, git, upstream, consumers)
                if error:
                    errors.append(error)
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--metadata", type=Path)
    parser.add_argument("--ownership", type=Path, default=OWNERSHIP_PATH)
    parser.add_argument("--repository-metadata", type=Path, default=REPOSITORY_METADATA_PATH)
    parser.add_argument("--lockfile", type=Path, default=LOCKFILE_PATH)
    args = parser.parse_args()
    metadata = load_json(args.metadata) if args.metadata else cargo_metadata()
    ownership = load_json(args.ownership)
    declaration = load_toml(args.repository_metadata)
    errors = validate(metadata, ownership)
    errors += validate_repository_declaration(declaration)
    errors += validate_upstream_dependencies(
        metadata,
        load_toml(args.lockfile),
        declaration,
        load_toml(WORKSPACE_MANIFEST_PATH),
    )
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        "repository boundaries pass: 65 uniquely owned foundation packages; "
        "wrappers depend on their declared libraries; no path escapes or moving Git dependencies; "
        "purpose, exclusions, and Pages N/A declared; no upstream edges into capability or "
        "product repositories"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
