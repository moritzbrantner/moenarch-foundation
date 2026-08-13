#!/usr/bin/env python3
"""Publish the one checked release manifest bound to this Agent Loop invocation.

All validation completes before the first publication, tag, or GitHub Release
side effect. The public interface is the no-argument CLI configured in
``.agent-loop.toml``; ``run_release`` accepts an effects adapter so tests can
replace only network and process boundaries.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Mapping, Protocol


EXPECTED_REPOSITORY = "moritzbrantner/moenarch-foundation"
AUTHORIZATION_LABEL = "release:approved"
REGISTRY = "crates.io"
OWNERSHIP_PATH = Path("docs/repository-split/package-ownership.json")
ROOT_FIELDS = {
    "schema_version",
    "repository",
    "issue",
    "head_sha",
    "registry",
    "dependency_order",
    "expected_tags",
    "packages",
    "github_releases",
}
PACKAGE_FIELDS = {
    "name",
    "version",
    "owner",
    "manifest_path",
    "dependencies",
    "tag",
}
RELEASE_FIELDS = {"tag", "title", "notes"}


class ReleaseError(RuntimeError):
    """A fail-closed release validation or external-operation failure."""


class Effects(Protocol):
    def repository(self) -> str: ...
    def head(self) -> str: ...
    def clean(self) -> bool: ...
    def tracked_manifests(self) -> list[str]: ...
    def issue(self, repository: str, number: int) -> dict[str, Any]: ...
    def cargo_metadata(self) -> dict[str, Any]: ...
    def registry_version(self, name: str, version: str) -> dict[str, Any] | None: ...
    def package(self, name: str) -> None: ...
    def publish(self, name: str) -> None: ...
    def wait_for_registry(self) -> None: ...
    def local_tag_target(self, tag: str) -> str | None: ...
    def remote_tag_target(self, tag: str) -> str | None: ...
    def create_tag(self, tag: str, message: str) -> None: ...
    def push_tag(self, tag: str) -> None: ...
    def release(self, repository: str, tag: str) -> dict[str, Any] | None: ...
    def create_release(
        self, repository: str, tag: str, title: str, notes: str
    ) -> None: ...


class CommandEffects:
    """Production adapter for GitHub, Cargo, git, and crates.io."""

    def __init__(self, root: Path) -> None:
        self.root = root.resolve()

    def _run(
        self, args: list[str], *, capture: bool = True, allow_failure: bool = False
    ) -> subprocess.CompletedProcess[str]:
        completed = subprocess.run(
            args,
            cwd=self.root,
            check=False,
            text=True,
            stdout=subprocess.PIPE if capture else None,
            stderr=subprocess.PIPE if capture else None,
        )
        if completed.returncode and not allow_failure:
            detail = (completed.stderr or "").strip()
            suffix = f": {detail}" if detail else ""
            raise ReleaseError(f"command failed ({' '.join(args)}){suffix}")
        return completed

    def repository(self) -> str:
        remote = self._run(["git", "config", "--get", "remote.origin.url"]).stdout.strip()
        if remote.startswith("git@github.com:"):
            remote = remote.removeprefix("git@github.com:")
        elif "github.com/" in remote:
            remote = remote.split("github.com/", 1)[1]
        return remote.removesuffix(".git").strip("/")

    def head(self) -> str:
        return self._run(["git", "rev-parse", "HEAD"]).stdout.strip()

    def clean(self) -> bool:
        return not self._run(["git", "status", "--porcelain"]).stdout.strip()

    def tracked_manifests(self) -> list[str]:
        output = self._run(
            ["git", "ls-tree", "-r", "--name-only", "HEAD", "--", "releases"]
        ).stdout
        return sorted(
            line.strip()
            for line in output.splitlines()
            if line.strip().startswith("releases/") and line.strip().endswith(".toml")
        )

    def issue(self, repository: str, number: int) -> dict[str, Any]:
        output = self._run(
            [
                "gh",
                "issue",
                "view",
                str(number),
                "--repo",
                repository,
                "--json",
                "number,state,url,labels",
            ]
        ).stdout
        try:
            return json.loads(output)
        except json.JSONDecodeError as error:
            raise ReleaseError("GitHub issue response was not valid JSON") from error

    def cargo_metadata(self) -> dict[str, Any]:
        output = self._run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"]
        ).stdout
        try:
            return json.loads(output)
        except json.JSONDecodeError as error:
            raise ReleaseError("Cargo metadata response was not valid JSON") from error

    def registry_version(self, name: str, version: str) -> dict[str, Any] | None:
        encoded_name = urllib.parse.quote(name, safe="")
        encoded_version = urllib.parse.quote(version, safe="")
        request = urllib.request.Request(
            f"https://crates.io/api/v1/crates/{encoded_name}/{encoded_version}",
            headers={"User-Agent": "moenarch-foundation-release-control/1"},
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                payload = json.load(response)
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return None
            raise ReleaseError(f"crates.io query failed with HTTP {error.code}") from error
        except (OSError, ValueError) as error:
            raise ReleaseError(f"crates.io query failed: {error}") from error
        record = payload.get("version")
        if not isinstance(record, dict):
            raise ReleaseError("crates.io returned an invalid version record")
        return record

    def package(self, name: str) -> None:
        self._run(["cargo", "package", "-p", name, "--locked"], capture=False)

    def publish(self, name: str) -> None:
        self._run(["cargo", "publish", "-p", name, "--locked"], capture=False)

    def wait_for_registry(self) -> None:
        time.sleep(5)

    def local_tag_target(self, tag: str) -> str | None:
        completed = self._run(
            ["git", "rev-parse", "--verify", f"refs/tags/{tag}^{{}}"],
            allow_failure=True,
        )
        return completed.stdout.strip() if completed.returncode == 0 else None

    def remote_tag_target(self, tag: str) -> str | None:
        output = self._run(
            [
                "git",
                "ls-remote",
                "--tags",
                "origin",
                f"refs/tags/{tag}",
                f"refs/tags/{tag}^{{}}",
            ]
        ).stdout
        records = {
            ref: sha for sha, ref in (line.split("\t", 1) for line in output.splitlines())
        }
        return records.get(f"refs/tags/{tag}^{{}}") or records.get(f"refs/tags/{tag}")

    def create_tag(self, tag: str, message: str) -> None:
        self._run(["git", "tag", "--annotate", tag, "HEAD", "--message", message])

    def push_tag(self, tag: str) -> None:
        self._run(["git", "push", "origin", f"refs/tags/{tag}"])

    def release(self, repository: str, tag: str) -> dict[str, Any] | None:
        completed = self._run(
            [
                "gh",
                "release",
                "view",
                tag,
                "--repo",
                repository,
                "--json",
                "tagName,name,body,isDraft,isPrerelease",
            ],
            allow_failure=True,
        )
        if completed.returncode:
            detail = (completed.stderr or "").lower()
            if "not found" in detail or "404" in detail:
                return None
            raise ReleaseError(f"could not inspect GitHub Release for {tag}")
        try:
            return json.loads(completed.stdout)
        except json.JSONDecodeError as error:
            raise ReleaseError("GitHub Release response was not valid JSON") from error

    def create_release(
        self, repository: str, tag: str, title: str, notes: str
    ) -> None:
        self._run(
            [
                "gh",
                "release",
                "create",
                tag,
                "--repo",
                repository,
                "--verify-tag",
                "--title",
                title,
                "--notes",
                notes,
            ]
        )


def _string(value: Any, description: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ReleaseError(f"{description} must be a non-empty string")
    return value.strip()


def _inside(root: Path, relative: str, description: str) -> Path:
    candidate = (root / relative).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError as error:
        raise ReleaseError(f"{description} escapes the repository") from error
    return candidate


def _unknown_fields(document: Mapping[str, Any], allowed: set[str], where: str) -> None:
    unknown = sorted(set(document) - allowed)
    if unknown:
        raise ReleaseError(f"{where} has unknown field(s): {', '.join(unknown)}")


def _load_candidates(root: Path, paths: list[str]) -> list[tuple[str, dict[str, Any]]]:
    candidates: list[tuple[str, dict[str, Any]]] = []
    if not paths:
        raise ReleaseError("no checked releases/*.toml manifests exist")
    for relative in paths:
        path = _inside(root, relative, "release manifest path")
        if not path.is_file():
            raise ReleaseError(f"checked release manifest is missing: {relative}")
        try:
            document = tomllib.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
            raise ReleaseError(f"malformed release manifest {relative}: {error}") from error
        if not isinstance(document, dict):
            raise ReleaseError(f"release manifest {relative} is not a TOML table")
        candidates.append((relative, document))
    return candidates


def _select_manifest(
    candidates: list[tuple[str, dict[str, Any]]],
    repository: str,
    issue: int,
    head: str,
) -> tuple[str, dict[str, Any]]:
    matches = [
        item
        for item in candidates
        if item[1].get("repository") == repository
        and item[1].get("issue") == issue
        and item[1].get("head_sha") == head
    ]
    if not matches:
        raise ReleaseError("no checked release manifest matches repository, issue, and head")
    if len(matches) != 1:
        raise ReleaseError("multiple checked release manifests match repository, issue, and head")
    return matches[0]


def _validate_issue(issue: dict[str, Any], repository: str, number: int) -> None:
    expected_url = f"https://github.com/{repository}/issues/{number}"
    labels = {
        label.get("name") for label in issue.get("labels", []) if isinstance(label, dict)
    }
    if issue.get("number") != number or issue.get("url") != expected_url:
        raise ReleaseError("GitHub issue does not match the destination-local authorization")
    if issue.get("state") != "OPEN":
        raise ReleaseError("destination-local release issue must be open")
    if AUTHORIZATION_LABEL not in labels:
        raise ReleaseError(f"destination-local release issue lacks {AUTHORIZATION_LABEL}")


def _validate_registry_record(
    record: dict[str, Any], name: str, version: str
) -> None:
    record_name = record.get("crate") or record.get("name")
    if (
        record_name != name
        or record.get("num") != version
        or record.get("yanked") is not False
    ):
        raise ReleaseError(f"registry conflict for {name} {version}")


def _validate_manifest(
    root: Path,
    manifest: dict[str, Any],
    metadata: dict[str, Any],
) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
    _unknown_fields(manifest, ROOT_FIELDS, "release manifest")
    if manifest.get("schema_version") != 1:
        raise ReleaseError("release manifest schema_version must be 1")
    if manifest.get("registry") != REGISTRY:
        raise ReleaseError(f"release manifest registry must be {REGISTRY}")

    raw_packages = manifest.get("packages")
    if not isinstance(raw_packages, list) or not raw_packages:
        raise ReleaseError("release manifest packages must be a non-empty array")
    packages: list[dict[str, Any]] = []
    for index, raw in enumerate(raw_packages):
        if not isinstance(raw, dict):
            raise ReleaseError(f"packages[{index}] must be a table")
        _unknown_fields(raw, PACKAGE_FIELDS, f"packages[{index}]")
        package = dict(raw)
        for field in ("name", "version", "owner", "manifest_path", "tag"):
            package[field] = _string(package.get(field), f"packages[{index}].{field}")
        if package["owner"] != EXPECTED_REPOSITORY:
            raise ReleaseError(f"{package['name']}: owner must be {EXPECTED_REPOSITORY}")
        if package["tag"] != f"{package['name']}-v{package['version']}":
            raise ReleaseError(f"{package['name']}: tag must be package-name-vversion")
        dependencies = package.get("dependencies")
        if not isinstance(dependencies, list) or any(
            not isinstance(item, str) or not item for item in dependencies
        ):
            raise ReleaseError(f"{package['name']}: dependencies must be a string array")
        package["dependencies"] = dependencies
        packages.append(package)

    names = [package["name"] for package in packages]
    tags = [package["tag"] for package in packages]
    if len(names) != len(set(names)) or len(tags) != len(set(tags)):
        raise ReleaseError("release manifest contains duplicate package names or tags")
    if manifest.get("dependency_order") != names:
        raise ReleaseError("dependency_order must exactly match packages array order")
    if manifest.get("expected_tags") != tags:
        raise ReleaseError("expected_tags must exactly match package tags")

    ownership_file = root / OWNERSHIP_PATH
    try:
        ownership_document = json.loads(ownership_file.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ReleaseError(f"cannot load package ownership: {error}") from error
    owned = {
        record.get("current_package_name"): record
        for record in ownership_document.get("packages", [])
        if isinstance(record, dict)
    }
    cargo_packages = {
        package.get("name"): package
        for package in metadata.get("packages", [])
        if isinstance(package, dict)
    }
    positions = {name: index for index, name in enumerate(names)}
    selected = set(names)
    for package in packages:
        name = package["name"]
        ownership = owned.get(name)
        cargo = cargo_packages.get(name)
        if ownership is None or cargo is None:
            raise ReleaseError(f"{name}: package is not owned by this Cargo workspace")
        if (
            ownership.get("intended_next_release_owner") != EXPECTED_REPOSITORY
            or ownership.get("manifest_path") != package["manifest_path"]
            or ownership.get("publication_class") != REGISTRY
            or ownership.get("automatic_publish_eligible") is not True
        ):
            raise ReleaseError(f"{name}: package ownership does not authorize this release")
        actual_manifest = Path(_string(cargo.get("manifest_path"), f"{name} Cargo manifest"))
        expected_manifest = _inside(root, package["manifest_path"], f"{name} manifest_path")
        if actual_manifest.resolve() != expected_manifest or not expected_manifest.is_file():
            raise ReleaseError(f"{name}: manifest_path does not match Cargo metadata")
        if cargo.get("version") != package["version"]:
            raise ReleaseError(f"{name}: version does not match Cargo metadata")
        publish = cargo.get("publish")
        if publish == [] or (isinstance(publish, list) and "crates-io" not in publish):
            raise ReleaseError(f"{name}: Cargo manifest does not permit crates.io publication")
        actual_dependencies = {
            dependency.get("name")
            for dependency in cargo.get("dependencies", [])
            if isinstance(dependency, dict)
            and dependency.get("kind") != "dev"
            and dependency.get("name") in selected
        }
        if set(package["dependencies"]) != actual_dependencies:
            raise ReleaseError(f"{name}: dependencies do not match Cargo metadata")
        for dependency in package["dependencies"]:
            if positions[dependency] >= positions[name]:
                raise ReleaseError(f"wrong dependency order: {dependency} must precede {name}")

    raw_releases = manifest.get("github_releases", [])
    if not isinstance(raw_releases, list):
        raise ReleaseError("github_releases must be an array")
    releases: list[dict[str, str]] = []
    release_tags: set[str] = set()
    for index, raw in enumerate(raw_releases):
        if not isinstance(raw, dict):
            raise ReleaseError(f"github_releases[{index}] must be a table")
        _unknown_fields(raw, RELEASE_FIELDS, f"github_releases[{index}]")
        release = {
            field: _string(raw.get(field), f"github_releases[{index}].{field}")
            for field in ("tag", "title", "notes")
        }
        if release["tag"] not in tags or release["tag"] in release_tags:
            raise ReleaseError("GitHub Releases must reference unique manifest-declared tags")
        release_tags.add(release["tag"])
        releases.append(release)
    return packages, releases


def _validate_existing_release(
    existing: dict[str, Any], release: dict[str, str]
) -> None:
    if (
        existing.get("tagName") != release["tag"]
        or existing.get("name") != release["title"]
        or existing.get("body") != release["notes"]
        or existing.get("isDraft") is not False
        or existing.get("isPrerelease") is not False
    ):
        raise ReleaseError(f"GitHub Release conflict for {release['tag']}")


def run_release(
    root: Path, environment: Mapping[str, str], effects: Effects
) -> dict[str, Any]:
    root = root.resolve()
    required = ("AGENT_LOOP_REPOSITORY", "AGENT_LOOP_ISSUE", "AGENT_LOOP_HEAD_SHA")
    if any(not environment.get(name, "").strip() for name in required):
        raise ReleaseError(
            "AGENT_LOOP_REPOSITORY, AGENT_LOOP_ISSUE, and "
            "AGENT_LOOP_HEAD_SHA are required"
        )
    repository = environment["AGENT_LOOP_REPOSITORY"].strip()
    head = environment["AGENT_LOOP_HEAD_SHA"].strip()
    try:
        issue_number = int(environment["AGENT_LOOP_ISSUE"])
    except ValueError as error:
        raise ReleaseError("AGENT_LOOP_ISSUE must be a positive integer") from error
    if issue_number < 1:
        raise ReleaseError("AGENT_LOOP_ISSUE must be a positive integer")
    if re.fullmatch(r"[0-9a-f]{40}", head) is None:
        raise ReleaseError("AGENT_LOOP_HEAD_SHA must be a full lowercase commit SHA")
    if repository != EXPECTED_REPOSITORY or effects.repository() != repository:
        raise ReleaseError("publication repository is not the owned destination repository")
    if effects.head() != head:
        raise ReleaseError("publication checkout does not match AGENT_LOOP_HEAD_SHA")
    if not effects.clean():
        raise ReleaseError("publication checkout must be clean")

    candidates = _load_candidates(root, effects.tracked_manifests())
    manifest_path, manifest = _select_manifest(candidates, repository, issue_number, head)
    _unknown_fields(manifest, ROOT_FIELDS, "release manifest")
    issue = effects.issue(repository, issue_number)
    _validate_issue(issue, repository, issue_number)
    packages, releases = _validate_manifest(root, manifest, effects.cargo_metadata())

    registry_present: list[bool] = []
    local_tags: dict[str, str | None] = {}
    remote_tags: dict[str, str | None] = {}
    for package in packages:
        record = effects.registry_version(package["name"], package["version"])
        if record is not None:
            _validate_registry_record(record, package["name"], package["version"])
        registry_present.append(record is not None)
        local_tags[package["tag"]] = effects.local_tag_target(package["tag"])
        remote_tags[package["tag"]] = effects.remote_tag_target(package["tag"])
        for target in (local_tags[package["tag"]], remote_tags[package["tag"]]):
            if target is not None and target != head:
                raise ReleaseError(f"immutable tag conflict for {package['tag']}")
        if record is None and (
            local_tags[package["tag"]] is not None
            or remote_tags[package["tag"]] is not None
        ):
            raise ReleaseError(f"tag exists before registry version for {package['name']}")

    first_absent = next(
        (index for index, present in enumerate(registry_present) if not present),
        len(packages),
    )
    if any(registry_present[first_absent:]):
        raise ReleaseError("registry state is not a published prefix in dependency order")

    existing_releases: dict[str, dict[str, Any] | None] = {}
    packages_by_tag = {package["tag"]: package for package in packages}
    for release in releases:
        existing = effects.release(repository, release["tag"])
        existing_releases[release["tag"]] = existing
        package = packages_by_tag[release["tag"]]
        if existing is not None:
            if not registry_present[packages.index(package)]:
                raise ReleaseError(f"GitHub Release exists before registry version for {release['tag']}")
            if remote_tags[release["tag"]] is None:
                raise ReleaseError(f"GitHub Release exists without its manifest tag: {release['tag']}")
            _validate_existing_release(existing, release)

    # Package every candidate before the first publishing side effect.
    for package in packages:
        effects.package(package["name"])

    package_results: list[dict[str, str]] = []
    for index, package in enumerate(packages):
        status = "registry-verified"
        if not registry_present[index]:
            effects.publish(package["name"])
            record = None
            for _ in range(12):
                record = effects.registry_version(package["name"], package["version"])
                if record is not None:
                    break
                effects.wait_for_registry()
            if record is None:
                raise ReleaseError(
                    f"published {package['name']} {package['version']} is not visible on crates.io"
                )
            _validate_registry_record(record, package["name"], package["version"])
            status = "published-and-verified"
        package_results.append({"name": package["name"], "status": status})

    tag_results: list[dict[str, str]] = []
    for package in packages:
        tag = package["tag"]
        status = "existing"
        if local_tags[tag] is None:
            effects.create_tag(tag, f"Release {package['name']} {package['version']}")
            status = "created"
        if remote_tags[tag] is None:
            effects.push_tag(tag)
            status = "created-and-pushed" if status == "created" else "pushed"
        if effects.remote_tag_target(tag) != head:
            raise ReleaseError(f"remote tag verification failed for {tag}")
        tag_results.append({"tag": tag, "status": status})

    release_results: list[dict[str, str]] = []
    for release in releases:
        status = "existing"
        if existing_releases[release["tag"]] is None:
            effects.create_release(
                repository, release["tag"], release["title"], release["notes"]
            )
            created = effects.release(repository, release["tag"])
            if created is None:
                raise ReleaseError(f"GitHub Release verification failed for {release['tag']}")
            _validate_existing_release(created, release)
            status = "created-and-verified"
        release_results.append({"tag": release["tag"], "status": status})

    return {
        "schemaVersion": 1,
        "repository": repository,
        "issue": issue_number,
        "head": head,
        "manifest": manifest_path,
        "packages": package_results,
        "tags": tag_results,
        "githubReleases": release_results,
    }


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    try:
        payload = run_release(root, os.environ, CommandEffects(root))
    except ReleaseError as error:
        print(f"release refused: {error}", file=sys.stderr)
        return 1
    print(json.dumps(payload, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
