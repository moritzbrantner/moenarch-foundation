#!/usr/bin/env python3
"""Behavior tests for the repository-owned release publisher."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path
from typing import Any
from unittest import mock


SCRIPT = Path(__file__).with_name("publish_release.py")
SPEC = importlib.util.spec_from_file_location("publish_release", SCRIPT)
assert SPEC and SPEC.loader
publish_release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publish_release)

REPOSITORY = "moritzbrantner/moenarch-foundation"
HEAD = "a" * 40
SOURCE = "b" * 40
CHECKSUM = "c" * 64
CONFIG_COMMANDS = tomllib.loads(
    (SCRIPT.parents[1] / ".agent-loop.toml").read_text(encoding="utf-8")
)["verification"]["commands"]
ENVIRONMENT = {
    "AGENT_LOOP_REPOSITORY": REPOSITORY,
    "AGENT_LOOP_ISSUE": "7",
    "AGENT_LOOP_HEAD_SHA": HEAD,
}


def registry_record(name: str, version: str) -> dict[str, Any]:
    return {
        "crate": name,
        "num": version,
        "yanked": False,
        "checksum": CHECKSUM,
    }


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
            "body": "",
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

    def source_is_ancestor(self, source: str, head: str) -> bool:
        self.calls.append(f"ancestor:{source}:{head}")
        return source == SOURCE and head == HEAD

    def changed_paths(self, source: str, head: str) -> list[str]:
        self.calls.append(f"changed:{source}:{head}")
        return ["releases/release.toml"]

    def issue(self, repository: str, number: int) -> dict[str, Any]:
        self.calls.append(f"issue:{repository}#{number}")
        return self.issue_payload

    def cargo_metadata(self) -> dict[str, Any]:
        self.calls.append("cargo-metadata")
        return self.metadata

    def verify(self, command: str) -> None:
        self.calls.append(f"verify:{command}")

    def registry_version(self, name: str, version: str) -> dict[str, Any] | None:
        self.calls.append(f"registry:{name}@{version}")
        if name in self.published:
            return registry_record(name, version)
        return self.registry.get(name)

    def package(self, name: str, version: str, patches: dict[str, str]) -> str:
        self.calls.append(f"package:{name}@{version}")
        return CHECKSUM

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
    (root / ".agent-loop.toml").write_text(
        (SCRIPT.parents[1] / ".agent-loop.toml").read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    write_manifest(root / "releases/release.toml", manifest_packages, releases=releases)
    effects = FakeEffects(metadata_packages)
    authorize_manifest(root, effects)
    return manifest_packages, effects


def authorize_manifest(root: Path, effects: FakeEffects) -> None:
    digest = hashlib.sha256((root / "releases/release.toml").read_bytes()).hexdigest()
    effects.issue_payload["body"] = f"Release manifest SHA-256: {digest}"


def write_manifest(
    path: Path,
    packages: list[dict[str, Any]],
    *,
    releases: bool = True,
    repository: str = REPOSITORY,
    issue: int = 7,
    source: str = SOURCE,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "schema_version = 1",
        f"repository = {json.dumps(repository)}",
        f"issue = {issue}",
        f"source_sha = {json.dumps(source)}",
        'registry = "crates.io"',
        "dependency_order = " + json.dumps([package["name"] for package in packages]),
        "expected_tags = " + json.dumps([package["tag"] for package in packages]),
        "required_checks = " + json.dumps(CONFIG_COMMANDS),
        'required_consumer_checks = ["true"]',
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
                source="c" * 40,
            )
            authorize_manifest(root, effects)
            with self.assertRaisesRegex(publish_release.ReleaseError, "source_sha"):
                publish_release.run_release(root, ENVIRONMENT, effects)

    def test_open_destination_issue_requires_authorization_label(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(root, [("foundation-a", "1.0.0", ())])
            effects.issue_payload["labels"] = []
            with self.assertRaisesRegex(publish_release.ReleaseError, "lacks release:approved"):
                publish_release.run_release(root, ENVIRONMENT, effects)
            self.assertFalse(any(call.startswith("package:") for call in effects.calls))

    def test_issue_authorizes_the_exact_manifest_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(root, [("foundation-a", "1.0.0", ())])
            effects.issue_payload["body"] = "Release manifest SHA-256: " + "0" * 64
            with self.assertRaisesRegex(publish_release.ReleaseError, "exact manifest digest"):
                publish_release.run_release(root, ENVIRONMENT, effects)

    def test_source_commit_may_differ_only_by_the_checked_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(root, [("foundation-a", "1.0.0", ())])
            effects.changed_paths = lambda source, head: [
                "crates/foundation-a/src/lib.rs",
                "releases/release.toml",
            ]
            with self.assertRaisesRegex(publish_release.ReleaseError, "more than"):
                publish_release.run_release(root, ENVIRONMENT, effects)

    def test_candidate_consumer_check_is_required(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(root, [("foundation-a", "1.0.0", ())])
            manifest = root / "releases/release.toml"
            manifest.write_text(
                manifest.read_text(encoding="utf-8").replace(
                    'required_consumer_checks = ["true"]',
                    "required_consumer_checks = []",
                ),
                encoding="utf-8",
            )
            authorize_manifest(root, effects)
            with self.assertRaisesRegex(publish_release.ReleaseError, "consumer_checks"):
                publish_release.run_release(root, ENVIRONMENT, effects)

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
                authorize_manifest(root, effects)
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

            effects.registry = {
                "foundation-a": {
                    **registry_record("foundation-a", "1.0.0"),
                    "checksum": "d" * 64,
                }
            }
            with self.assertRaisesRegex(publish_release.ReleaseError, "registry conflict"):
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

    def test_checkout_mutation_during_packaging_stops_before_publication(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(root, [("foundation-a", "1.0.0", ())])

            def mutate_checkout(
                name: str, version: str, patches: dict[str, str]
            ) -> str:
                effects.calls.append(f"package:{name}@{version}")
                effects.is_clean = False
                return CHECKSUM

            effects.package = mutate_checkout
            with self.assertRaisesRegex(publish_release.ReleaseError, "changed"):
                publish_release.run_release(root, ENVIRONMENT, effects)
            self.assertNotIn("publish:foundation-a", effects.calls)

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

    def test_undeclared_existing_github_release_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _, effects = write_fixture(
                root, [("foundation-a", "1.0.0", ())], releases=False
            )
            tag = "foundation-a-v1.0.0"
            effects.registry["foundation-a"] = registry_record("foundation-a", "1.0.0")
            effects.local_tags[tag] = HEAD
            effects.remote_tags[tag] = HEAD
            effects.releases[tag] = {
                "tagName": tag,
                "name": "Unreviewed",
                "body": "Unreviewed",
                "isDraft": False,
                "isPrerelease": False,
            }
            with self.assertRaisesRegex(publish_release.ReleaseError, "undeclared"):
                publish_release.run_release(root, ENVIRONMENT, effects)

    def test_cargo_effects_pin_package_and_publish_to_crates_io(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive = root / "target/package/foundation-a-1.0.0.crate"
            archive.parent.mkdir(parents=True)
            archive.write_bytes(b"archive")
            completed = publish_release.subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(
                publish_release.subprocess, "run", return_value=completed
            ) as run:
                effects = publish_release.CommandEffects(root)
                effects.package(
                    "foundation-a",
                    "1.0.0",
                    {"foundation-a": str(root / "foundation-a")},
                )
                effects.publish("foundation-a")
            commands = [call.args[0] for call in run.call_args_list]
            package_command = next(command for command in commands if command[1] == "package")
            self.assertEqual(package_command[:7], [
                "cargo",
                "package",
                "-p",
                "foundation-a",
                "--locked",
                "--registry",
                "crates-io",
            ])
            self.assertEqual(package_command[7], "--config")
            self.assertIn(
                [
                    "cargo",
                    "publish",
                    "-p",
                    "foundation-a",
                    "--locked",
                    "--registry",
                    "crates-io",
                ],
                commands,
            )

    def test_cargo_packages_downstream_with_unpublished_local_dependency(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "Cargo.toml").write_text(
                '[workspace]\nmembers = ["foundation-a", "foundation-b"]\nresolver = "2"\n',
                encoding="utf-8",
            )
            for name in ("foundation-a", "foundation-b"):
                crate = root / name
                (crate / "src").mkdir(parents=True)
                dependencies = (
                    '\n[dependencies]\nfoundation-a = { version = "1.0.0", path = "../foundation-a" }\n'
                    if name == "foundation-b"
                    else ""
                )
                (crate / "Cargo.toml").write_text(
                    f'[package]\nname = "{name}"\nversion = "1.0.0"\nedition = "2021"\nlicense = "MIT"\n'
                    + dependencies,
                    encoding="utf-8",
                )
                (crate / "src/lib.rs").write_text("pub fn value() -> u8 { 1 }\n")
            subprocess.run(
                ["cargo", "generate-lockfile"], cwd=root, check=True, capture_output=True
            )
            with mock.patch.dict(
                publish_release.os.environ, {"CARGO_NET_OFFLINE": "true"}
            ):
                checksum = publish_release.CommandEffects(root).package(
                    "foundation-b",
                    "1.0.0",
                    {
                        "foundation-a": str(root / "foundation-a"),
                        "foundation-b": str(root / "foundation-b"),
                    },
                )
            self.assertRegex(checksum, r"^[0-9a-f]{64}$")


if __name__ == "__main__":
    unittest.main()
