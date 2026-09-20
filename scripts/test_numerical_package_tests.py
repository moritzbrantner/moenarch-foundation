"""Archive evidence must be fresh and independent of repository symlinks."""

from __future__ import annotations

import io
import json
import tarfile
import tempfile
import unittest
from pathlib import Path

from check_numerical_package_tests import extract_archive


class NumericalArchiveTests(unittest.TestCase):
    def check_archive(self, *, revision: str = "a" * 40, helper: bool = True, symlink: bool = False, dirty: bool = False) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "fixture-1.0.0.crate"
            with tarfile.open(path, "w:gz") as archive:
                data = json.dumps({"git": {"sha1": revision, "dirty": dirty}}).encode()
                member = tarfile.TarInfo("fixture-1.0.0/.cargo_vcs_info.json")
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
                if helper:
                    member = tarfile.TarInfo("fixture-1.0.0/tests/support/numerical.rs")
                    if symlink:
                        member.type = tarfile.SYMTYPE
                        member.linkname = "../../../outside.rs"
                    archive.addfile(member, io.BytesIO(b""))
            extracted = extract_archive(path, root / "extracted", "a" * 40)
            self.assertTrue((extracted / "tests/support/numerical.rs").is_file())

    def test_current_self_contained_archive_is_accepted(self) -> None:
        self.check_archive()

    def test_missing_helper_fails(self) -> None:
        with self.assertRaises(KeyError):
            self.check_archive(helper=False)

    def test_external_helper_link_fails(self) -> None:
        with self.assertRaisesRegex(ValueError, "ordinary file"):
            self.check_archive(symlink=True)

    def test_stale_or_dirty_evidence_fails(self) -> None:
        with self.assertRaisesRegex(ValueError, "clean source"):
            self.check_archive(revision="b" * 40)
        with self.assertRaisesRegex(ValueError, "clean source"):
            self.check_archive(dirty=True)


if __name__ == "__main__":
    unittest.main()
