#!/usr/bin/env python3
"""Regression tests for deterministic external Git packaging patches."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from check_release_plan import exact_external_git_patches


class ExactExternalGitPatchesTests(unittest.TestCase):
    def write_manifest(self, root: Path, dependency: str) -> dict:
        manifest = root / "crates" / "facade" / "Cargo.toml"
        manifest.parent.mkdir(parents=True)
        manifest.write_text(
            "[package]\n"
            'name = "facade"\n'
            'version = "0.1.0"\n'
            "\n[dependencies]\n"
            + dependency
            + "\n",
            encoding="utf-8",
        )
        return {
            "packages": [
                {
                    "current_package_name": "facade",
                    "manifest_path": "crates/facade/Cargo.toml",
                }
            ]
        }

    def test_exact_external_git_dependency_is_preserved_for_package_verification(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ownership = self.write_manifest(
                root,
                'collection-kernels = { version = "0.1.0", git = "https://github.com/moritzbrantner/rust-kernels", rev = "d7b6e69bf14bc7fc2a0e0776d5823139473fb7c5" }',
            )

            self.assertEqual(
                exact_external_git_patches(ownership, root),
                {
                    "collection-kernels": (
                        "https://github.com/moritzbrantner/rust-kernels",
                        "d7b6e69bf14bc7fc2a0e0776d5823139473fb7c5",
                    )
                },
            )

    def test_mutable_git_dependency_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ownership = self.write_manifest(
                root,
                'collection-kernels = { version = "0.1.0", git = "https://github.com/moritzbrantner/rust-kernels", branch = "main" }',
            )

            with self.assertRaisesRegex(ValueError, "exact 40-character rev"):
                exact_external_git_patches(ownership, root)


if __name__ == "__main__":
    unittest.main(verbosity=2)
