"""Check that numerical integration-test modules ship inside their crates."""

from __future__ import annotations

import posixpath
import re
import subprocess
import unittest

from repository_split import ROOT


class NumericalPackageArchiveTests(unittest.TestCase):
    def test_explicit_test_modules_are_included_in_package_file_lists(self) -> None:
        for crate in (
            "crates/data/numbers-core",
            "crates/math/math-linear",
            "crates/math/math-geometry-3d",
        ):
            with self.subTest(crate=crate):
                # Listing is read-only and must inspect current edits as well
                # as committed files; --allow-dirty does not publish anything.
                completed = subprocess.run(
                    [
                        "cargo", "package", "--list", "--locked", "--offline",
                        "--allow-dirty",
                        "--manifest-path", str(ROOT / crate / "Cargo.toml"),
                    ],
                    cwd=ROOT,
                    check=False,
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                self.assertEqual(completed.returncode, 0, completed.stderr)
                packaged = set(completed.stdout.splitlines())
                for path in sorted(packaged):
                    if not path.startswith("tests/") or not path.endswith(".rs"):
                        continue
                    source = (ROOT / crate / path).read_text(encoding="utf-8")
                    for module in re.findall(r'#\[path\s*=\s*"([^"]+)"\]', source):
                        module_path = posixpath.normpath(
                            posixpath.join(posixpath.dirname(path), module)
                        )
                        self.assertIn(
                            module_path,
                            packaged,
                            f"{crate}/{path} references a module absent from its archive",
                        )


if __name__ == "__main__":
    unittest.main()
