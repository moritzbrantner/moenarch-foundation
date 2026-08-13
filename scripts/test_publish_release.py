#!/usr/bin/env python3
"""Behavior tests for the repository-owned release publisher."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from typing import Any


SCRIPT = Path(__file__).with_name("publish_release.py")
SPEC = importlib.util.spec_from_file_location("publish_release", SCRIPT)
assert SPEC and SPEC.loader
publish_release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publish_release)

REPOSITORY = "moritzbrantner/moenarch-foundation"
HEAD = "a" * 40
ENVIRONMENT = {
    "AGENT_LOOP_REPOSITORY": REPOSITORY,
    "AGENT_LOOP_ISSUE": "7",
    "AGENT_LOOP_HEAD_SHA": HEAD,
}


def registry_record(name: str, version: str) -> dict[str, Any]:
    return {"crate": name, "num": version, "yanked": False}


class ForbiddenEffects:
    def __init__(self) -> None:
        self.calls: list[str] = []

    def __getattr__(self, name: str):
        self.calls.append(name)
        raise AssertionError(f"unexpected external access: {name}")


class FakeEffects:
    def __init__(self, packages: list[dict[str, Any]]) -> None:
        self.calls: list[str] = []
        self.repo = REPOSITORY
        self.sha = HEAD
        self.is_clean = True
        self.manifests = ["releases/release.toml"]
        self.issue_payload = {
            "number": 7,
            "state": "OPEN",
            "url": f"https://github.com/{REPOSITORY}/issues/7",
            "labels": [{"name": "release:approved"}],
        }
        self.metadata = {"packages": packages}
        self.published: set[str] = set()
        self.registry: dict[str, dict[str, Any]] = {}
        self.local_tags: dict[str, str] = {}
        self.remote_tags: dict[str, str] = {}
        self.releases: dict[str, dict[str, Any]] = {}
        self.fail_publish: str | None = None

    def repository(self) -> str:
        self.calls.append("repository")
        return self.repo

    def head(self) -> str:
        self.calls.append("head")
        return self.sha

    def clean(self) -> bool:
        self.calls.append("clean")
        return self.is_clean

    def tracked_manifests(self) -> list[str]:
        self.calls.append("tracked-manifests")
        return self.manifests

    def issue(self, repository: str, number: int) -> dict[str, Any]:
        self.calls.append(f"issue:{repository}#{number}")
        return self.issue_payload

    def cargo_metadata(self) -> dict[str, Any]:
        self.calls.append("cargo-metadata")
        return self.metadata

    def registry_version(self, name: str, version: str) -> dict[str, Any] | None:
        self.calls.append(f"registry:{name}@{version}")
        if name in self.published:
            return registry_record(name, version)
        return self.registry.get(name)

    def package(self, name: str) -> None:
        self.calls.append(f"package:{name}")

    def publish(self, name: str) -> None:
        self.calls.append(f"publish:{name}")
        if name == self.fail_publish:
            raise publish_release.ReleaseError(f"simulated publish failure: {name}")
        self.published.add(name)

    def wait_for_registry(self) -> None:
        self.calls.append("wait")

    def local_tag_target(self, tag: str) -> str | None:
        self.calls.append(f"local-tag:{tag}")
        return self.local_tags.get(tag)

    def remote_tag_target(self, tag: str) -> str | None:
        self.calls.append(f"remote-tag:{tag}")
        return self.remote_tags.get(tag)

    def create_tag(self, tag: str, message: str) -> None:
        self.calls.append(f"create-tag:{tag}")
        self.local_tags[tag] = HEAD

    def push_tag(self, tag: str) -> None:
        self.calls.append(f"push-tag:{tag}")
        self.remote_tags[tag] = self.local_tags[tag]

    def release(self, repository: str, tag: str) -> dict[str, Any] | None:
        self.calls.append(f"release:{tag}")
        return self.releases.get(tag)

    def create_release(
        self, repository: str, tag: str, title: str, notes: str
    ) -> None:
        self.calls.append(f"create-release:{tag}")
        self.releases[tag] = {
            "tagName": tag,
            "name": title,
            "body": notes,
            "isDraft": False,
            "isPrerelease": False,
        }


def package_record(
    root: Path,
    name: str,
    version: str = "1.0.0",
    dependencies: tuple[str, ...] = (),
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    manifest_path = f"crates/{name}/Cargo.toml"
    path = root / manifest_path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n")
    manifest_package = {
        "name": name,
        "version": version,
        "owner": REPOSITORY,
        "manifest_path": manifest_path,
        "dependencies": list(dependencies),
        "tag": f"{name}-v{version}",
    }
    metadata_package = {
        "name": name,
        "version": version,
        "manifest_path": str(path.resolve()),
        "publish": None,
        "dependencies": [
            {"name": dependency, "kind": None} for dependency in dependencies
        ],
    }
    ownership = {
        "current_package_name": name,
        "manifest_path": manifest_path,
        "intended_next_release_owner": REPOSITORY,
        "publication_class": "crates.io",
        "automatic_publish_eligible": True,
    }
    return manifest_package, metadata_package, ownership


def write_fixture(
    root: Path,
    package_specs: list[tuple[str, str, tuple[str, ...]]],
    *,
    releases: bool = True,
) -> tuple[list[dict[str, Any]], FakeEffects]:
    manifest_packages: list[dict[str, Any]] = []
    metadata_packages: list[dict[str, Any]] = []
    ownership: list[dict[str, Any]] = []
    for name, version, dependencies in package_specs:
        manifest, metadata, owned = package_record(root, name, version, dependencies)
        manifest_packages.append(manifest)
        metadata_packages.append(metadata)
        ownership.append(owned)
    ownership_path = root / "docs/repository-split/package-ownership.json"
    ownership_path.parent.mkdir(parents=True, exist_ok=True)
    ownership_path.write_text(json.dumps({"packages": ownership}), encoding="utf-8")
    write_manifest(root / "releases/release.toml", manifest_packages, releases=releases)
    return manifest_packages, FakeEffects(metadata_packages)


def write_manifest(
    path: Path,
    packages: list[dict[str, Any]],
    *,
    releases: bool = True,
    repository: str = REPOSITORY,
    issue: int = 7,
    head: str = HEAD,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "schema_version = 1",
        f"repository = {json.dumps(repository)}",
        f"issue = {issue}",
        f"head_sha = {json.dumps(head)}",
        'registry = "crates.io"',
        "dependency_order = " + json.dumps([package["name"] for package in packages]),
        "expected_tags = " + json.dumps([package["tag"] for package in packages]),
    ]
    for package in packages:
        lines.extend(
            [
                "",
                "[[packages]]",
                f"name = {json.dumps(package['name'])}",
                f"version = {json.dumps(package['version'])}",
                f"owner = {json.dumps(package['owner'])}",
                f"manifest_path = {json.dumps(package['manifest_path'])}",
                "dependencies = " + json.dumps(package["dependencies"]),
                f"tag = {json.dumps(package['tag'])}",
            ]
        )
    if releases:
        for package in packages:
            lines.extend(
                [
                    "",
                    "[[github_releases]]",
                    f"tag = {json.dumps(package['tag'])}",
                    f"title = {json.dumps('Release ' + package['name'] + ' ' + package['version'])}",
                    f"notes = {json.dumps('Verified immutable release.')}",
                ]
            )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


class PublishReleaseTests(unittest.TestCase):
    def test_missing_agent_loop_binding_refuses_before_external_access(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            effects = ForbiddenEffects()
            with self.assertRaisesRegex(
                publish_release.ReleaseError,
                "AGENT_LOOP_REPOSITORY, AGENT_LOOP_ISSUE, and AGENT_LOOP_HEAD_SHA",
            ):
                publish_release.run_release(Path(temp), {}, effects)
            self.assertEqual(effects.calls, [])

    def test_malformed_and_ambiguous_checked_manifests_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            packages, effects = write_fixture(root, [("foundation-a", "1.0.0", ())])
            malformed = root / "releases/broken.toml"
            malformed.write_text("not = [valid", encoding="utf-8")
            effects.manifests.append("releases/broken.toml")
            with self.assertRaisesRegex(publish_release.ReleaseError, "malformed"):
                publish_release.run_release(root, ENVIRONMENT, effects)

            write_manifest(malformed, packages)
            with self.assertRaisesRegex(publish_release.ReleaseError, "multiple checked"):
                publish_release.run_release(root, ENVIRONMENT, effects)

    def test_repository_head_manifest_and_issue_bindings_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(root, [("foundation-a", "1.0.0", ())])
            effects.repo = "moritzbrantner/rust-packages"
            with self.assertRaisesRegex(publish_release.ReleaseError, "owned destination"):
                publish_release.run_release(root, ENVIRONMENT, effects)

            effects.repo = REPOSITORY
            effects.sha = "b" * 40
            with self.assertRaisesRegex(publish_release.ReleaseError, "HEAD_SHA"):
                publish_release.run_release(root, ENVIRONMENT, effects)

            effects.sha = HEAD
            write_manifest(
                root / "releases/release.toml",
                [package_record(root, "foundation-a")[0]],
                head="c" * 40,
            )
            with self.assertRaisesRegex(publish_release.ReleaseError, "no checked release"):
                publish_release.run_release(root, ENVIRONMENT, effects)

    def test_open_destination_issue_requires_authorization_label(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(root, [("foundation-a", "1.0.0", ())])
            effects.issue_payload["labels"] = []
            with self.assertRaisesRegex(publish_release.ReleaseError, "lacks release:approved"):
                publish_release.run_release(root, ENVIRONMENT, effects)
            self.assertFalse(any(call.startswith("package:") for call in effects.calls))

    def test_package_version_ownership_order_and_tag_are_validated(self) -> None:
        cases = (
            ("version", lambda packages: packages[0].update(version="9.9.9")),
            ("owner", lambda packages: packages[0].update(owner="someone/else")),
            ("dependency order", lambda packages: packages.reverse()),
            ("tag", lambda packages: packages[0].update(tag="latest")),
        )
        for expected, mutate in cases:
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                packages, effects = write_fixture(
                    root,
                    [
                        ("foundation-a", "1.0.0", ()),
                        ("foundation-b", "1.0.0", ("foundation-a",)),
                    ],
                )
                mutate(packages)
                write_manifest(root / "releases/release.toml", packages)
                with self.assertRaisesRegex(publish_release.ReleaseError, expected):
                    publish_release.run_release(root, ENVIRONMENT, effects)
                self.assertFalse(any(call.startswith("publish:") for call in effects.calls))

    def test_registry_conflict_and_non_prefix_partial_state_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(
                root,
                [
                    ("foundation-a", "1.0.0", ()),
                    ("foundation-b", "1.0.0", ("foundation-a",)),
                ],
            )
            effects.registry["foundation-a"] = {
                "crate": "foundation-a",
                "num": "1.0.0",
                "yanked": True,
            }
            with self.assertRaisesRegex(publish_release.ReleaseError, "registry conflict"):
                publish_release.run_release(root, ENVIRONMENT, effects)

            effects.registry = {
                "foundation-b": registry_record("foundation-b", "1.0.0")
            }
            with self.assertRaisesRegex(publish_release.ReleaseError, "published prefix"):
                publish_release.run_release(root, ENVIRONMENT, effects)

    def test_partial_resume_skips_only_registry_verified_prefix(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(
                root,
                [
                    ("foundation-a", "1.0.0", ()),
                    ("foundation-b", "1.0.0", ("foundation-a",)),
                ],
            )
            effects.registry["foundation-a"] = registry_record("foundation-a", "1.0.0")
            result = publish_release.run_release(root, ENVIRONMENT, effects)

            self.assertNotIn("publish:foundation-a", effects.calls)
            self.assertIn("publish:foundation-b", effects.calls)
            self.assertEqual(
                result["packages"],
                [
                    {"name": "foundation-a", "status": "registry-verified"},
                    {"name": "foundation-b", "status": "published-and-verified"},
                ],
            )

    def test_publish_failure_stops_before_tags_and_releases(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(root, [("foundation-a", "1.0.0", ())])
            effects.fail_publish = "foundation-a"
            with self.assertRaisesRegex(publish_release.ReleaseError, "simulated"):
                publish_release.run_release(root, ENVIRONMENT, effects)
            self.assertFalse(any(call.startswith("create-tag:") for call in effects.calls))
            self.assertFalse(any(call.startswith("push-tag:") for call in effects.calls))
            self.assertFalse(any(call.startswith("create-release:") for call in effects.calls))

    def test_registry_verification_precedes_tag_and_release_creation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(root, [("foundation-a", "1.0.0", ())])
            result = publish_release.run_release(root, ENVIRONMENT, effects)
            tag = "foundation-a-v1.0.0"
            publish_index = effects.calls.index("publish:foundation-a")
            verified_index = len(effects.calls) - 1 - effects.calls[::-1].index(
                "registry:foundation-a@1.0.0"
            )
            create_tag_index = effects.calls.index(f"create-tag:{tag}")
            push_tag_index = effects.calls.index(f"push-tag:{tag}")
            create_release_index = effects.calls.index(f"create-release:{tag}")
            self.assertLess(publish_index, verified_index)
            self.assertLess(verified_index, create_tag_index)
            self.assertLess(create_tag_index, push_tag_index)
            self.assertLess(push_tag_index, create_release_index)
            self.assertEqual(result["githubReleases"][0]["status"], "created-and-verified")


if __name__ == "__main__":
    unittest.main()
