"""Run numerical tests from fresh crate archives against exact local sources."""

from __future__ import annotations

import json
import os
import subprocess
import tarfile
import tempfile
from pathlib import Path

from repository_split import ROOT, cargo_metadata

PACKAGES = (
    "moenarch-numbers-core",
    "moenarch-math-linear",
    "moenarch-math-geometry-3d",
)


def extract_archive(archive_path: Path, destination: Path, revision: str) -> Path:
    """Reject stale archives and missing helpers before invoking Cargo."""
    directory = archive_path.name.removesuffix(".crate")
    with tarfile.open(archive_path) as archive:
        vcs_file = archive.extractfile(f"{directory}/.cargo_vcs_info.json")
        if vcs_file is None:
            raise ValueError("archive has no source revision")
        vcs = json.load(vcs_file)["git"]
        if vcs["sha1"] != revision or vcs.get("dirty", False):
            raise ValueError("archive must come from the current clean source revision")
        helper = archive.getmember(f"{directory}/tests/support/numerical.rs")
        if not helper.isfile():
            raise ValueError("archive numerical helper must be an ordinary file")
        archive.extractall(destination, filter="data")
    return destination / directory


def main() -> None:
    if subprocess.check_output(
        ["git", "status", "--porcelain", "--untracked-files=normal"], cwd=ROOT, text=True
    ).strip():
        raise ValueError("archive verification requires a clean source checkout")
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    metadata = cargo_metadata()
    packages = {package["name"]: package for package in metadata["packages"]}
    target = Path(metadata["target_directory"])
    with tempfile.TemporaryDirectory(prefix="foundation-archive-tests-") as temporary:
        root = Path(temporary)
        config = root / "patches.toml"
        config.write_text(
            "[patch.crates-io]\n" + "\n".join(
                f'{json.dumps(name)} = {{ path = {json.dumps(str(Path(p["manifest_path"]).parent))} }}'
                for name, p in sorted(packages.items())
            ) + "\n",
            encoding="utf-8",
        )
        environment = dict(os.environ, CARGO_TARGET_DIR=str(target))
        for name in PACKAGES:
            filename = f'{name}-{packages[name]["version"]}.crate'
            extracted = extract_archive(target / "package" / filename, root, revision)
            # Only the disposable lockfile changes to select local sources.
            subprocess.run(
                ["cargo", "update", "--workspace", "--offline", "--config", str(config)],
                cwd=extracted, env=environment, check=True, timeout=300,
            )
            subprocess.run(
                ["cargo", "test", "--all-features", "--locked", "--offline", "--config", str(config)],
                cwd=extracted, env=environment, check=True, timeout=600,
            )
            print(f"ARCHIVE TESTS PASSED {name}", flush=True)


if __name__ == "__main__":
    main()
