#!/usr/bin/env python3
"""Focused tests for the non-publishing bootstrap release plan."""

from __future__ import annotations

import copy
import shutil
import tempfile
import tomllib
import unittest
from pathlib import Path

from check_release_plan import (
    FOUNDATION_WAVE_1_CONSUMER_CHECKS,
    FOUNDATION_WAVE_1_VERSIONS,
    MEDIA_CORE_0_2,
    MEDIA_CORE_0_2_CONSUMER_CHECKS,
    MEDIA_CORE_0_2_ISSUE,
    MEDIA_CORE_0_2_PATH,
    external_prerequisites,
    validate,
    validate_control_binding,
    validate_release_manifest,
)
from repository_split import (
    OWNERSHIP_PATH,
    RELEASE_PLAN_PATH,
    ROOT,
    cargo_metadata,
    load_json,
)


FIXTURES = Path(__file__).with_name("fixtures") / "release_plans"


def historical_workspace(metadata: dict, versions: dict[str, str], root: Path) -> dict:
    """Mirror workspace package manifests under ``root`` at historical versions.

    Only the fields the release-manifest validator reads are reproduced: each
    package's ``[package].version``, its manifest location, and the exact
    ``=version`` requirements of internal path dependencies.
    """

    metadata = copy.deepcopy(metadata)
    live_root = ROOT.resolve()
    moved: dict[Path, Path] = {}
    for package in metadata["packages"]:
        live_manifest = Path(package["manifest_path"]).resolve()
        mirrored = root / live_manifest.relative_to(live_root)
        version = versions.get(package["name"], package["version"])
        mirrored.parent.mkdir(parents=True, exist_ok=True)
        mirrored.write_text(
            f'[package]\nname = "{package["name"]}"\nversion = "{version}"\n',
            encoding="utf-8",
        )
        moved[live_manifest.parent] = mirrored.parent
        package["manifest_path"] = str(mirrored)
        package["version"] = version
    fixture_versions = {package["name"]: package["version"] for package in metadata["packages"]}
    for package in metadata["packages"]:
        for dependency in package.get("dependencies", []):
            path = dependency.get("path")
            if path is None or Path(path).resolve() not in moved:
                continue
            dependency["path"] = str(moved[Path(path).resolve()])
            if str(dependency.get("req", "")).startswith("="):
                dependency["req"] = f"={fixture_versions[dependency['name']]}"
    shutil.copyfile(live_root / "release-gate.toml", root / "release-gate.toml")
    # Historical manifests validate against a minimal workspace root.
    (root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    return metadata


def candidate_manifest(
    ownership_document: dict,
    metadata_document: dict,
    wave: list[tuple[str, str]],
    *,
    issue: int,
    required_checks: list[str],
    required_consumer_checks: list[str],
) -> dict:
    ownership = {
        package["current_package_name"]: package
        for package in ownership_document["packages"]
    }
    metadata = {package["name"]: package for package in metadata_document["packages"]}
    packages = []
    for name, version in wave:
        dependencies = {
            dependency["name"]
            for dependency in metadata[name]["dependencies"]
            if dependency["kind"] != "dev" and dependency["name"] in metadata
        }
        packages.append(
            {
                "name": name,
                "version": version,
                "owner": "moritzbrantner/moenarch-foundation",
                "manifest_path": ownership[name]["manifest_path"],
                "dependencies": sorted(dependencies),
                "tag": f"{name}-v{version}",
            }
        )
    return {
        "schema_version": 1,
        "repository": "moritzbrantner/moenarch-foundation",
        "issue": issue,
        "source_sha": "a" * 40,
        "registry": "crates.io",
        "dependency_order": [name for name, _ in wave],
        "expected_tags": [package["tag"] for package in packages],
        "required_checks": required_checks,
        "required_consumer_checks": required_consumer_checks,
        "packages": packages,
        "github_releases": [],
    }


class ReleasePlanTests(unittest.TestCase):
    def setUp(self) -> None:
        self.plan = load_json(RELEASE_PLAN_PATH)
        self.ownership = load_json(OWNERSHIP_PATH)
        self.metadata = cargo_metadata()

    def errors(self, plan: dict) -> list[str]:
        return validate(plan, self.ownership, self.metadata)

    def test_live_nonpublishing_plan_is_valid(self) -> None:
        self.assertEqual(self.errors(self.plan), [])

    def test_nonpublishing_plan_names_canonical_release_owner(self) -> None:
        self.assertEqual(
            self.plan["active_release_owner"],
            "moritzbrantner/moenarch-foundation",
        )
        self.assertTrue(
            all(
                package["intended_next_release_owner"]
                == "moritzbrantner/moenarch-foundation"
                for package in self.plan["packages"]
            )
        )

    def test_active_release_owner_cannot_return_to_source_repository(self) -> None:
        plan = copy.deepcopy(self.plan)
        plan["active_release_owner"] = "moritzbrantner/rust-packages"
        errors = self.errors(plan)
        self.assertTrue(any("wrong active release owner" in error for error in errors))

    def test_historical_source_cannot_regain_release_authority(self) -> None:
        plan = copy.deepcopy(self.plan)
        plan["historical_source_role"] = "active-release-owner"
        errors = self.errors(plan)
        self.assertTrue(any("historical source role" in error for error in errors))

    def test_publication_cannot_be_smuggled_into_bootstrap(self) -> None:
        plan = copy.deepcopy(self.plan)
        plan["packages"][0]["publish"] = True
        self.assertTrue(any("publication is not authorized" in e for e in self.errors(plan)))

    def test_version_change_is_rejected(self) -> None:
        plan = copy.deepcopy(self.plan)
        plan["packages"][0]["new_version"] = "9.9.9"
        self.assertTrue(any("retain version" in e for e in self.errors(plan)))

    def test_forged_equal_versions_are_rejected(self) -> None:
        plan = copy.deepcopy(self.plan)
        plan["packages"][0]["old_version"] = "9.9.9"
        plan["packages"][0]["new_version"] = "9.9.9"
        errors = self.errors(plan)
        self.assertTrue(any("ownership source_version" in error for error in errors), errors)

    def test_required_checks_cannot_be_deleted(self) -> None:
        plan = copy.deepcopy(self.plan)
        plan["required_checks"] = []
        errors = self.errors(plan)
        self.assertTrue(any("complete bootstrap gate set" in error for error in errors), errors)

    def test_required_checks_include_validator_tests_and_exact_base_harness_audit(self) -> None:
        self.assertIn(
            "python3 -m unittest discover -s scripts -p 'test_*.py'",
            self.plan["required_checks"],
        )
        self.assertIn(
            "python3 scripts/repository_split.py --harness-audit --base-ref <reviewed-base-sha>",
            self.plan["required_checks"],
        )

    def test_real_internal_dependency_cannot_be_deleted(self) -> None:
        plan = copy.deepcopy(self.plan)
        package = next(item for item in plan["packages"] if item["release_dependencies"])
        package["release_dependencies"] = []
        errors = self.errors(plan)
        self.assertTrue(any("do not match workspace metadata" in error for error in errors), errors)

    def test_ownership_source_version_is_bound(self) -> None:
        ownership = copy.deepcopy(self.ownership)
        ownership["packages"][0]["source_version"] = "9.9.9"
        errors = validate(self.plan, ownership, self.metadata)
        self.assertTrue(any("ownership source_version" in error for error in errors), errors)

    def test_metadata_and_plan_cannot_jointly_forge_source_version(self) -> None:
        plan = copy.deepcopy(self.plan)
        metadata = copy.deepcopy(self.metadata)
        name = plan["packages"][0]["name"]
        plan["packages"][0]["old_version"] = "9.9.9"
        plan["packages"][0]["new_version"] = "9.9.9"
        next(package for package in metadata["packages"] if package["name"] == name)[
            "version"
        ] = "9.9.9"
        errors = validate(plan, self.ownership, metadata)
        self.assertTrue(any("ownership source_version" in error for error in errors), errors)

    def test_workspace_versions_allow_only_the_exact_foundation_wave(self) -> None:
        metadata = copy.deepcopy(self.metadata)
        source_versions = {
            package["current_package_name"]: package["source_version"]
            for package in self.ownership["packages"]
        }
        for package in metadata["packages"]:
            package["version"] = FOUNDATION_WAVE_1_VERSIONS.get(
                package["name"], source_versions[package["name"]]
            )

        self.assertEqual(validate(self.plan, self.ownership, metadata), [])

        out_of_wave = next(
            package
            for package in metadata["packages"]
            if package["name"] not in FOUNDATION_WAVE_1_VERSIONS
        )
        out_of_wave["version"] = "9.9.9"
        errors = validate(self.plan, self.ownership, metadata)
        self.assertTrue(
            any("authorized source or wave version" in error for error in errors),
            errors,
        )

    def test_wrong_owner_and_missing_package_are_rejected(self) -> None:
        plan = copy.deepcopy(self.plan)
        plan["packages"][0]["intended_next_release_owner"] = (
            "moritzbrantner/rust-packages"
        )
        plan["packages"].pop()
        errors = self.errors(plan)
        self.assertTrue(any("65 owned packages" in e for e in errors))
        self.assertTrue(any("wrong intended next release owner" in e for e in errors))

    def test_wrong_dependency_order_is_rejected(self) -> None:
        plan = copy.deepcopy(self.plan)
        dependent = next(p for p in plan["packages"] if p["release_dependencies"])
        dependency = dependent["release_dependencies"][0]
        order = plan["dependency_order"]
        order.remove(dependency)
        order.append(dependency)
        self.assertTrue(any("wrong dependency order" in e for e in self.errors(plan)))


class CheckedReleaseManifestTests(unittest.TestCase):
    WAVE = [
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

    # Wave 1 published against media-core 0.1.0; later waves moved the live
    # workspace past these versions, so the historical contract is validated
    # against a manifest-only mirror of the workspace at its own versions.
    HISTORICAL_VERSIONS = {"moenarch-media-core": "0.1.0"}

    def setUp(self) -> None:
        self.ownership = load_json(OWNERSHIP_PATH)
        self.metadata = cargo_metadata()
        self.manifest = self.wave_manifest()
        self._mirror = tempfile.TemporaryDirectory()
        self.addCleanup(self._mirror.cleanup)
        self.root = Path(self._mirror.name)
        self.metadata = historical_workspace(
            self.metadata,
            self.HISTORICAL_VERSIONS | dict(self.WAVE),
            self.root,
        )

    def wave_manifest(self) -> dict:
        checks = tomllib.loads(
            (ROOT / "releases/foundation-wave-1.toml").read_text(encoding="utf-8")
        )["required_checks"]
        return candidate_manifest(
            self.ownership,
            self.metadata,
            self.WAVE,
            issue=8,
            required_checks=checks,
            required_consumer_checks=list(FOUNDATION_WAVE_1_CONSUMER_CHECKS),
        )

    def errors(self, manifest: dict) -> list[str]:
        return validate_release_manifest(
            manifest,
            self.ownership,
            self.metadata,
            Path("releases/foundation-wave-1.toml"),
            self.root,
        )

    def test_exact_foundation_wave_is_valid(self) -> None:
        self.assertEqual(self.errors(self.manifest), [])

    def test_wrong_owner_is_rejected(self) -> None:
        self.manifest["packages"][0]["owner"] = "moritzbrantner/rust-packages"
        self.assertTrue(any("owner" in error for error in self.errors(self.manifest)))

    def test_wrong_order_is_rejected(self) -> None:
        self.manifest["packages"][0], self.manifest["packages"][1] = (
            self.manifest["packages"][1],
            self.manifest["packages"][0],
        )
        self.manifest["dependency_order"] = [
            package["name"] for package in self.manifest["packages"]
        ]
        errors = self.errors(self.manifest)
        self.assertTrue(any("package order" in error for error in errors), errors)

    def test_wrong_version_is_rejected(self) -> None:
        self.manifest["packages"][0]["version"] = "0.2.2"
        errors = self.errors(self.manifest)
        self.assertTrue(any("versions" in error for error in errors), errors)

    def test_wrong_destination_issue_is_rejected(self) -> None:
        self.manifest["issue"] = 9
        errors = self.errors(self.manifest)
        self.assertTrue(any("destination issue 8" in error for error in errors), errors)

    def test_wrong_source_and_control_binding_are_rejected(self) -> None:
        errors = validate_control_binding(
            self.manifest,
            Path("releases/foundation-wave-1.toml"),
            "b" * 40,
            False,
            ["Cargo.toml", "releases/foundation-wave-1.toml"],
        )
        self.assertTrue(any("ancestor" in error for error in errors), errors)
        self.assertTrue(any("only by its manifest" in error for error in errors), errors)

    def test_partial_wave_control_repair_binding_is_validated(self) -> None:
        self.manifest["repair_source_sha"] = "c" * 40
        repair_paths = [
            "releases/foundation-wave-1.toml",
            "scripts/check_release_plan.py",
            "scripts/publish_release.py",
            "scripts/test_check_release_plan.py",
            "scripts/test_publish_release.py",
        ]
        errors = validate_control_binding(
            self.manifest,
            Path("releases/foundation-wave-1.toml"),
            "b" * 40,
            True,
            repair_paths,
            repair_is_ancestor=True,
            repair_changed_paths=["releases/foundation-wave-1.toml"],
        )
        self.assertEqual(errors, [])

        repair_paths.append("crates/runtime/runtime-core/src/lib.rs")
        errors = validate_control_binding(
            self.manifest,
            Path("releases/foundation-wave-1.toml"),
            "b" * 40,
            True,
            repair_paths,
            repair_is_ancestor=True,
            repair_changed_paths=["releases/foundation-wave-1.toml"],
        )
        self.assertTrue(any("control repair" in error for error in errors), errors)

    def test_wrong_dependency_metadata_is_rejected(self) -> None:
        jobs = next(
            package
            for package in self.manifest["packages"]
            if package["name"] == "moenarch-jobs-core"
        )
        jobs["dependencies"] = []
        errors = self.errors(self.manifest)
        self.assertTrue(any("dependencies" in error for error in errors), errors)

    def test_missing_consumer_or_package_list_gate_is_rejected(self) -> None:
        self.manifest["required_consumer_checks"].pop()
        errors = self.errors(self.manifest)
        self.assertTrue(any("consumer checks" in error for error in errors), errors)

    def test_generic_toml_fixture_uses_the_destination_publisher_schema(self) -> None:
        fixture = tomllib.loads((FIXTURES / "valid.toml").read_text(encoding="utf-8"))
        ownership = load_json(FIXTURES / "ownership.json")
        workspace = FIXTURES / "workspace"
        errors = validate_release_manifest(
            fixture,
            ownership,
            cargo_metadata(workspace),
            Path("releases/fixture.toml"),
            workspace,
        )
        self.assertEqual(errors, [])


class MediaCoreModelRequiredWaveTests(unittest.TestCase):
    """The issue #143 contract validates against the live workspace."""

    def setUp(self) -> None:
        self.ownership = load_json(OWNERSHIP_PATH)
        self.metadata = cargo_metadata()
        gate = tomllib.loads((ROOT / "release-gate.toml").read_text(encoding="utf-8"))
        self.manifest = candidate_manifest(
            self.ownership,
            self.metadata,
            MEDIA_CORE_0_2,
            issue=MEDIA_CORE_0_2_ISSUE,
            required_checks=list(gate["verification"]["commands"]),
            required_consumer_checks=list(MEDIA_CORE_0_2_CONSUMER_CHECKS),
        )

    def errors(self, manifest: dict) -> list[str]:
        return validate_release_manifest(
            manifest, self.ownership, self.metadata, Path(MEDIA_CORE_0_2_PATH)
        )

    def test_exact_wave_matches_live_workspace(self) -> None:
        self.assertEqual(self.errors(self.manifest), [])

    def test_rust_kernels_use_registry_requirements_without_patches(self) -> None:
        prerequisites = external_prerequisites(
            self.manifest, self.ownership, self.metadata
        )
        self.assertEqual(prerequisites, [])
        kernel_dependencies = {
            (package["name"], dependency["name"]): dependency
            for package in self.metadata["packages"]
            for dependency in package["dependencies"]
            if dependency["name"] in {"graph-kernels", "search-kernels"}
        }
        self.assertEqual(
            set(kernel_dependencies),
            {
                ("moenarch-graph-analysis-core", "graph-kernels"),
                ("moenarch-vector-analysis-index", "search-kernels"),
                ("moenarch-math-sparse-data", "search-kernels"),
            },
        )
        self.assertNotIn("patch", tomllib.loads((ROOT / "Cargo.toml").read_text()))
        for dependency in kernel_dependencies.values():
            self.assertEqual(dependency["req"], "=0.1.0")
            self.assertIsNone(dependency["path"])
            self.assertTrue(dependency["source"].startswith("registry+"))

    def test_kernel_lockfiles_resolve_only_published_crates(self) -> None:
        kernel_names = {"collection-kernels", "graph-kernels", "search-kernels"}
        sources = []
        for lockfile in (
            ROOT / "Cargo.lock",
            ROOT / "scripts/fixtures/media_core_0_2_consumer/Cargo.lock",
        ):
            lock = tomllib.loads(lockfile.read_text(encoding="utf-8"))
            kernel_packages = {
                package["name"]: package
                for package in lock["package"]
                if package["name"] in kernel_names
            }
            self.assertEqual(set(kernel_packages), kernel_names)
            for name, package in kernel_packages.items():
                self.assertEqual(package["version"], "0.1.0", name)
                self.assertEqual(
                    package["source"], "registry+https://github.com/rust-lang/crates.io-index"
                )
                self.assertRegex(package["checksum"], r"^[0-9a-f]{64}$")
            sources.append(
                {name: package["checksum"] for name, package in kernel_packages.items()}
            )
        self.assertEqual(*sources)

    def test_registry_lookup_is_not_needed_without_kernel_patches(self) -> None:
        errors = validate_release_manifest(
            self.manifest,
            self.ownership,
            self.metadata,
            Path(MEDIA_CORE_0_2_PATH),
            registry_version=lambda _name, _version: None,
        )
        self.assertEqual(errors, [])

    def test_every_published_media_core_dependent_is_in_the_wave(self) -> None:
        wave = {name for name, _ in MEDIA_CORE_0_2}
        for package in self.metadata["packages"]:
            if package["name"] in wave or package.get("publish") == []:
                continue
            uses_media_core = any(
                dependency["name"] == "moenarch-media-core"
                and dependency["kind"] != "dev"
                for dependency in package["dependencies"]
            )
            if uses_media_core:
                # Unpublished packages may follow the pin without a release.
                self.assertIn(
                    package["name"],
                    {"moenarch-math-probability"},
                    f"{package['name']} depends on media-core but is not in the wave",
                )

    def test_wrong_issue_is_rejected(self) -> None:
        self.manifest["issue"] = 17
        errors = self.errors(self.manifest)
        self.assertTrue(any("issue 143" in error for error in errors), errors)

    def test_wrong_version_is_rejected(self) -> None:
        self.manifest["packages"][0]["version"] = "0.1.1"
        self.manifest["packages"][0]["tag"] = "moenarch-media-core-v0.1.1"
        self.manifest["expected_tags"][0] = "moenarch-media-core-v0.1.1"
        errors = self.errors(self.manifest)
        self.assertTrue(any("versions" in error for error in errors), errors)

    def test_missing_consumer_gate_is_rejected(self) -> None:
        self.manifest["required_consumer_checks"] = []
        errors = self.errors(self.manifest)
        self.assertTrue(any("consumer checks" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main(verbosity=2)
