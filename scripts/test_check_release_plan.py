#!/usr/bin/env python3
"""Focused tests for the non-publishing bootstrap release plan."""

from __future__ import annotations

import copy
import tomllib
import unittest
from pathlib import Path

from check_release_plan import (
    FOUNDATION_WAVE_1_CONSUMER_CHECKS,
    FOUNDATION_WAVE_1_VERSIONS,
    validate,
    validate_control_binding,
    validate_release_manifest,
)
from repository_split import OWNERSHIP_PATH, RELEASE_PLAN_PATH, cargo_metadata, load_json


FIXTURES = Path(__file__).with_name("fixtures") / "release_plans"


class ReleasePlanTests(unittest.TestCase):
    def setUp(self) -> None:
        self.plan = load_json(RELEASE_PLAN_PATH)
        self.ownership = load_json(OWNERSHIP_PATH)
        self.metadata = cargo_metadata()

    def errors(self, plan: dict) -> list[str]:
        return validate(plan, self.ownership, self.metadata)

    def test_live_nonpublishing_plan_is_valid(self) -> None:
        self.assertEqual(self.errors(self.plan), [])

    def test_nonpublishing_plan_names_current_and_next_release_owners(self) -> None:
        self.assertEqual(
            self.plan["active_release_owner"],
            "moritzbrantner/rust-packages",
        )
        self.assertTrue(
            all(
                package["intended_next_release_owner"]
                == "moritzbrantner/moenarch-foundation"
                for package in self.plan["packages"]
            )
        )

    def test_active_release_owner_cannot_move_during_bootstrap(self) -> None:
        plan = copy.deepcopy(self.plan)
        plan["active_release_owner"] = "moritzbrantner/moenarch-foundation"
        errors = self.errors(plan)
        self.assertTrue(any("wrong active release owner" in error for error in errors))

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
        self.assertTrue(any("60 owned packages" in e for e in errors))
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

    def setUp(self) -> None:
        self.ownership = load_json(OWNERSHIP_PATH)
        self.metadata = cargo_metadata()
        self.manifest = self.wave_manifest()

    def wave_manifest(self) -> dict:
        order = [name for name, _ in self.WAVE]
        versions = dict(self.WAVE)
        ownership = {
            package["current_package_name"]: package
            for package in self.ownership["packages"]
        }
        metadata = {package["name"]: package for package in self.metadata["packages"]}
        packages = []
        for name in order:
            dependencies = {
                dependency["name"]
                for dependency in metadata[name]["dependencies"]
                if dependency["kind"] != "dev" and dependency["name"] in metadata
            }
            packages.append(
                {
                    "name": name,
                    "version": versions[name],
                    "owner": "moritzbrantner/moenarch-foundation",
                    "manifest_path": ownership[name]["manifest_path"],
                    "dependencies": sorted(dependencies),
                    "tag": f"{name}-v{versions[name]}",
                }
            )
        checks = tomllib.loads(
            (OWNERSHIP_PATH.parents[2] / ".agent-loop.toml").read_text(
                encoding="utf-8"
            )
        )["verification"]["commands"]
        return {
            "schema_version": 1,
            "repository": "moritzbrantner/moenarch-foundation",
            "issue": 8,
            "source_sha": "a" * 40,
            "registry": "crates.io",
            "dependency_order": order,
            "expected_tags": [package["tag"] for package in packages],
            "required_checks": checks,
            "required_consumer_checks": list(FOUNDATION_WAVE_1_CONSUMER_CHECKS),
            "packages": packages,
            "github_releases": [],
        }

    def errors(self, manifest: dict) -> list[str]:
        return validate_release_manifest(
            manifest,
            self.ownership,
            self.metadata,
            Path("releases/foundation-wave-1.toml"),
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


if __name__ == "__main__":
    unittest.main(verbosity=2)
