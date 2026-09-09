#!/usr/bin/env python3
"""Focused tests for destination ownership and dependency boundaries."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path

from check_repository_boundaries import validate
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


if __name__ == "__main__":
    unittest.main(verbosity=2)
