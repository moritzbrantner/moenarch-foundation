#!/usr/bin/env python3
"""Focused tests for the non-publishing bootstrap release plan."""

from __future__ import annotations

import copy
import unittest

from check_release_plan import validate
from repository_split import OWNERSHIP_PATH, RELEASE_PLAN_PATH, cargo_metadata, load_json


class ReleasePlanTests(unittest.TestCase):
    def setUp(self) -> None:
        self.plan = load_json(RELEASE_PLAN_PATH)
        self.ownership = load_json(OWNERSHIP_PATH)
        self.metadata = cargo_metadata()

    def errors(self, plan: dict) -> list[str]:
        return validate(plan, self.ownership, self.metadata)

    def test_live_nonpublishing_plan_is_valid(self) -> None:
        self.assertEqual(self.errors(self.plan), [])

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
        self.assertTrue(any("workspace version" in error for error in errors), errors)

    def test_required_checks_cannot_be_deleted(self) -> None:
        plan = copy.deepcopy(self.plan)
        plan["required_checks"] = []
        errors = self.errors(plan)
        self.assertTrue(any("complete bootstrap gate set" in error for error in errors), errors)

    def test_real_internal_dependency_cannot_be_deleted(self) -> None:
        plan = copy.deepcopy(self.plan)
        package = next(item for item in plan["packages"] if item["release_dependencies"])
        package["release_dependencies"] = []
        errors = self.errors(plan)
        self.assertTrue(any("do not match workspace metadata" in error for error in errors), errors)

    def test_wrong_owner_and_missing_package_are_rejected(self) -> None:
        plan = copy.deepcopy(self.plan)
        plan["packages"][0]["owner"] = "moritzbrantner/rust-packages"
        plan["packages"].pop()
        errors = self.errors(plan)
        self.assertTrue(any("60 owned packages" in e for e in errors))

    def test_wrong_dependency_order_is_rejected(self) -> None:
        plan = copy.deepcopy(self.plan)
        dependent = next(p for p in plan["packages"] if p["release_dependencies"])
        dependency = dependent["release_dependencies"][0]
        order = plan["dependency_order"]
        order.remove(dependency)
        order.append(dependency)
        self.assertTrue(any("wrong dependency order" in e for e in self.errors(plan)))


if __name__ == "__main__":
    unittest.main(verbosity=2)
