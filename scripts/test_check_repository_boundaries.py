#!/usr/bin/env python3
"""Focused tests for destination ownership and dependency boundaries."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path

from check_repository_boundaries import (
    LOCKFILE_PATH,
    REPOSITORY_METADATA_PATH,
    WORKSPACE_MANIFEST_PATH,
    load_toml,
    validate,
    validate_repository_declaration,
    validate_upstream_dependencies,
)
from repository_split import OWNERSHIP_PATH, cargo_metadata, load_json

ROOT = Path(__file__).resolve().parents[1]
NEGATIVE = ROOT / "scripts/fixtures/repository_boundaries/negative-new-edge"


class RepositoryBoundaryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.metadata = cargo_metadata()
        self.ownership = load_json(OWNERSHIP_PATH)

    def test_live_workspace_has_exact_approved_ownership(self) -> None:
        self.assertEqual(validate(self.metadata, self.ownership), [])

    def test_missing_and_duplicate_records_fail(self) -> None:
        missing = copy.deepcopy(self.ownership)
        missing["packages"].pop()
        self.assertTrue(any("unclassified" in e for e in validate(self.metadata, missing)))
        duplicate = copy.deepcopy(self.ownership)
        duplicate["packages"].append(copy.deepcopy(duplicate["packages"][0]))
        self.assertTrue(any("classified more than once" in e for e in validate(self.metadata, duplicate)))

    def test_wrong_owner_fails(self) -> None:
        ownership = copy.deepcopy(self.ownership)
        ownership["packages"][0]["target_repository"] = "rust-packages"
        self.assertTrue(any("wrong target repository" in e for e in validate(self.metadata, ownership)))

    def test_historical_source_cannot_retain_active_authority(self) -> None:
        ownership = copy.deepcopy(self.ownership)
        ownership["historical_source_role"] = "active-release-owner"
        errors = validate(self.metadata, ownership)
        self.assertTrue(any("historical source role" in error for error in errors), errors)

    def test_canonical_authority_cannot_point_to_historical_source(self) -> None:
        ownership = copy.deepcopy(self.ownership)
        ownership["canonical_authority"]["repository"] = "moritzbrantner/rust-packages"
        errors = validate(self.metadata, ownership)
        self.assertTrue(any("canonical authority" in error for error in errors), errors)

    def test_phase_a_record_field_drift_fails(self) -> None:
        ownership = copy.deepcopy(self.ownership)
        record = next(
            item for item in ownership["packages"] if "provenance" not in item
        )
        record["current_domain"] = "unreviewed-domain"
        errors = validate(self.metadata, ownership)
        self.assertTrue(any("source ownership records" in e for e in errors), errors)

    def test_post_extraction_record_field_drift_fails(self) -> None:
        ownership = copy.deepcopy(self.ownership)
        record = next(
            item
            for item in ownership["packages"]
            if item["current_package_name"] == "moenarch-math-geometry-3d"
        )
        record["current_domain"] = "unreviewed-domain"
        errors = validate(self.metadata, ownership)
        self.assertTrue(any("post-extraction ownership additions" in e for e in errors), errors)

    def test_adapter_must_wrap_a_workspace_library(self) -> None:
        ownership = copy.deepcopy(self.ownership)
        adapter = next(r for r in ownership["packages"] if r["package_kind"] == "CLI")
        adapter["wrapped_library"] = "missing-library"
        self.assertTrue(any("invalid wrapped_library" in e for e in validate(self.metadata, ownership)))

    def test_adapter_must_depend_on_declared_wrapped_library(self) -> None:
        metadata = copy.deepcopy(self.metadata)
        adapter_record = next(
            r for r in self.ownership["packages"] if r["package_kind"] == "CLI"
        )
        adapter = next(
            p
            for p in metadata["packages"]
            if p["name"] == adapter_record["current_package_name"]
        )
        wrapped = adapter_record["wrapped_library"]
        matching_dependencies = [
            dependency
            for dependency in adapter["dependencies"]
            if dependency.get("name") == wrapped
        ]
        self.assertTrue(matching_dependencies)
        adapter["dependencies"] = [
            dependency
            for dependency in adapter["dependencies"]
            if dependency.get("name") != wrapped
        ]

        errors = validate(metadata, self.ownership)
        self.assertTrue(any("is not a normal dependency" in e for e in errors), errors)

    def test_dev_only_dependency_does_not_satisfy_wrapper_contract(self) -> None:
        metadata = copy.deepcopy(self.metadata)
        adapter_record = next(
            r for r in self.ownership["packages"] if r["package_kind"] == "CLI"
        )
        adapter = next(
            p
            for p in metadata["packages"]
            if p["name"] == adapter_record["current_package_name"]
        )
        wrapped = adapter_record["wrapped_library"]
        matching_dependencies = [
            dependency
            for dependency in adapter["dependencies"]
            if dependency.get("name") == wrapped
        ]
        self.assertTrue(matching_dependencies)
        for dependency in matching_dependencies:
            dependency["kind"] = "dev"

        errors = validate(metadata, self.ownership)
        self.assertTrue(any("is not a normal dependency" in e for e in errors), errors)

    def test_synthetic_path_escape_fails(self) -> None:
        metadata = copy.deepcopy(self.metadata)
        metadata["packages"][0]["dependencies"].append({"name": "outside", "path": "/tmp/outside"})
        self.assertTrue(any("escapes repository" in e for e in validate(metadata, self.ownership)))

    def test_checked_in_cross_capability_edge_fixture_fails(self) -> None:
        errors = validate(
            load_json(NEGATIVE / "metadata.json"),
            load_json(NEGATIVE / "ownership.json"),
        )
        self.assertTrue(
            any("forbidden foundation dependency" in error for error in errors),
            errors,
        )

    def test_out_of_inventory_moenarch_dependency_fails(self) -> None:
        metadata = copy.deepcopy(self.metadata)
        metadata["packages"][0]["dependencies"].append(
            {"name": "moenarch-unreviewed-capability", "kind": None}
        )
        errors = validate(metadata, self.ownership)
        self.assertTrue(any("out-of-inventory" in error for error in errors), errors)

    def test_moving_git_branch_with_resolved_hash_fails(self) -> None:
        metadata = copy.deepcopy(self.metadata)
        metadata["packages"][0]["dependencies"].append(
            {
                "name": "remote",
                "source": "git+https://example.invalid/repository?branch=main#"
                + "a" * 40,
            }
        )
        errors = validate(metadata, self.ownership)
        self.assertTrue(any("non-immutable Git" in error for error in errors), errors)

    def test_exact_git_revision_metadata_form_is_allowed(self) -> None:
        metadata = copy.deepcopy(self.metadata)
        metadata["packages"][0]["dependencies"].append(
            {
                "name": "remote",
                "source": "git+https://example.invalid/repository?rev=" + "a" * 40,
            }
        )
        self.assertFalse(
            any("Git dependency" in error for error in validate(metadata, self.ownership))
        )

    def test_exact_git_revision_and_matching_resolved_hash_is_allowed(self) -> None:
        metadata = copy.deepcopy(self.metadata)
        metadata["packages"][0]["dependencies"].append(
            {
                "name": "remote",
                "source": "git+https://example.invalid/repository?rev="
                + "a" * 40
                + "#"
                + "a" * 40,
            }
        )
        self.assertFalse(
            any("Git dependency" in error for error in validate(metadata, self.ownership))
        )

    def test_exact_git_revision_with_mismatched_resolved_hash_fails(self) -> None:
        metadata = copy.deepcopy(self.metadata)
        metadata["packages"][0]["dependencies"].append(
            {
                "name": "remote",
                "source": "git+https://example.invalid/repository?rev="
                + "a" * 40
                + "#"
                + "b" * 40,
            }
        )
        errors = validate(metadata, self.ownership)
        self.assertTrue(any("non-immutable Git" in error for error in errors), errors)


class FoundationNeutralityTests(unittest.TestCase):
    """Issue #108: purpose/exclusion declaration and upward-dependency boundary."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.metadata = cargo_metadata()
        cls.lockfile = load_toml(LOCKFILE_PATH)
        cls.declaration = load_toml(REPOSITORY_METADATA_PATH)
        cls.manifest = load_toml(WORKSPACE_MANIFEST_PATH)

    def upstream_errors(self, metadata=None, lockfile=None, declaration=None, manifest=None):
        return validate_upstream_dependencies(
            metadata or self.metadata,
            lockfile or self.lockfile,
            declaration or self.declaration,
            manifest if manifest is not None else self.manifest,
        )

    def with_dependency(self, dependency: dict) -> dict:
        metadata = copy.deepcopy(self.metadata)
        metadata["packages"][0]["dependencies"].append(dependency)
        return metadata

    def test_live_declaration_and_dependency_graph_pass(self) -> None:
        self.assertEqual(validate_repository_declaration(self.declaration), [])
        self.assertEqual(self.upstream_errors(), [])

    def test_purpose_must_be_one_sentence(self) -> None:
        for summary in ["", "Two sentences. Not one.", "No terminal period"]:
            declaration = copy.deepcopy(self.declaration)
            declaration["summary"] = summary
            errors = validate_repository_declaration(declaration)
            self.assertTrue(any("one-sentence purpose" in e for e in errors), (summary, errors))

    def test_required_exclusions_cannot_be_dropped(self) -> None:
        declaration = copy.deepcopy(self.declaration)
        declaration["architecture"]["must_not_own"].remove("media/asr")
        errors = validate_repository_declaration(declaration)
        self.assertTrue(any("must exclude media/asr" in e for e in errors), errors)

    def test_owned_capability_inside_an_exclusion_fails(self) -> None:
        declaration = copy.deepcopy(self.declaration)
        declaration["architecture"]["owns"].append("media/ocr/layout")
        errors = validate_repository_declaration(declaration)
        self.assertTrue(any("excluded by media/ocr" in e for e in errors), errors)

    def test_capability_repositories_must_stay_declared_consumers(self) -> None:
        for repository in [
            "moritzbrantner/audio-analysis",
            "moritzbrantner/visual-analysis",
            "moritzbrantner/nlp-stack",
            "moritzbrantner/scenedetect-rs",
        ]:
            declaration = copy.deepcopy(self.declaration)
            declaration["consumed_by"].remove(repository)
            declaration["depends_on"].append(repository)
            errors = validate_repository_declaration(declaration)
            self.assertTrue(any(f"{repository} must be declared in consumed_by" in e for e in errors))
            self.assertTrue(any(f"{repository} must not be declared in depends_on" in e for e in errors))

    def test_pages_must_be_declared_not_applicable(self) -> None:
        missing = copy.deepcopy(self.declaration)
        del missing["pages"]
        enabled = copy.deepcopy(self.declaration)
        enabled["pages"] = {"status": "enabled", "command": ["true"], "check": ["true"]}
        no_reason = copy.deepcopy(self.declaration)
        no_reason["pages"] = {"status": "not-applicable", "reason": " "}
        for declaration in [missing, enabled, no_reason]:
            errors = validate_repository_declaration(declaration)
            self.assertTrue(any("[pages]" in e for e in errors), errors)

    def test_registry_packages_from_capability_repositories_fail(self) -> None:
        for name, repository in [
            ("moenarch-audio-analysis-core", "audio-analysis"),
            ("moenarch-video-analysis-core", "visual-analysis"),
            ("moenarch-text-core", "nlp-stack"),
            ("scenedetect-core", "scenedetect-rs"),
        ]:
            for kind in [None, "dev", "build"]:
                errors = self.upstream_errors(
                    metadata=self.with_dependency(
                        {
                            "name": name,
                            "kind": kind,
                            "source": "registry+https://github.com/rust-lang/crates.io-index",
                        }
                    )
                )
                self.assertTrue(
                    any(f"moritzbrantner/{repository} package" in e for e in errors),
                    (name, kind, errors),
                )

    def test_git_edges_into_consumer_and_application_repositories_fail(self) -> None:
        for repository in [
            "visual-analysis",
            "scenedetect-rs",
            "spatial-media-explorer",
            "some-new-application",
        ]:
            errors = self.upstream_errors(
                metadata=self.with_dependency(
                    {
                        "name": "upward",
                        "kind": None,
                        "source": f"git+https://github.com/moritzbrantner/{repository}.git?rev="
                        + "a" * 40,
                    }
                )
            )
            self.assertTrue(
                any(f"moritzbrantner/{repository}" in e for e in errors), (repository, errors)
            )

    def test_declared_upstream_git_source_is_allowed(self) -> None:
        errors = self.upstream_errors(
            metadata=self.with_dependency(
                {
                    "name": "collection-kernels",
                    "kind": None,
                    "source": "git+https://github.com/moritzbrantner/rust-kernels?rev=" + "a" * 40,
                }
            )
        )
        self.assertEqual(errors, [])

    def test_transitive_locked_capability_package_fails(self) -> None:
        lockfile = copy.deepcopy(self.lockfile)
        lockfile["package"].append(
            {
                "name": "moenarch-image-analysis-ocr",
                "version": "0.1.0",
                "source": "git+https://github.com/moritzbrantner/visual-analysis?rev="
                + "a" * 40
                + "#"
                + "a" * 40,
            }
        )
        errors = self.upstream_errors(lockfile=lockfile)
        self.assertTrue(any("forbidden locked package" in e for e in errors), errors)
        self.assertTrue(any("consumer repository moritzbrantner/visual-analysis" in e for e in errors))

    def test_workspace_patch_into_sibling_checkout_fails(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["patch"] = {
            "crates-io": {
                "moenarch-text-core": {"path": "../nlp-stack/crates/text/text-core"},
                "scenedetect-core": {"git": "https://github.com/moritzbrantner/scenedetect-rs"},
            }
        }
        errors = self.upstream_errors(manifest=manifest)
        self.assertTrue(any("patch path escapes repository" in e for e in errors), errors)
        self.assertTrue(any("scenedetect-rs" in e for e in errors), errors)


if __name__ == "__main__":
    unittest.main(verbosity=2)
