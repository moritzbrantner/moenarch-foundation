#!/usr/bin/env python3
"""Audit or synchronize canonical workflow and local-first loop labels."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from typing import Any


LABELS = {
    "bug": ("D73A4A", "Something isn't working"),
    "enhancement": ("A2EEEF", "New feature or request"),
    "needs-triage": ("D4C5F9", "Needs review before assignment"),
    "needs-info": ("D876E3", "Waiting on reporter for more information"),
    "ready-for-agent": ("0E8A16", "Fully specified, ready for an AFK agent"),
    "ready-for-human": ("FBCA04", "Requires human implementation"),
    "wontfix": ("ffffff", "This will not be worked on"),
    "prd": ("6F42C1", "Product requirements document ready for workflow routing"),
    "agent-loop:active": ("1D76DB", "Work is active in the agent loop"),
    "agent-loop:blocked": ("D93F0B", "Blocked on human input or external access"),
    "agent-loop:ready-to-merge": ("5319E7", "Worker reports the PR is ready to merge"),
    "release:approved": ("B60205", "Destination-local issue authorizes its exact release manifest"),
}


def run_gh(args: list[str]) -> str:
    proc = subprocess.run(
        ["gh", *args],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if proc.returncode:
        sys.stderr.write(proc.stderr)
        raise SystemExit(proc.returncode)
    return proc.stdout


def load_labels(repo: str) -> dict[str, dict[str, str]]:
    raw = run_gh(
        [
            "label",
            "list",
            "--repo",
            repo,
            "--limit",
            "500",
            "--json",
            "name,color,description",
        ]
    )
    return {
        item["name"]: {
            "color": item.get("color") or "",
            "description": item.get("description") or "",
        }
        for item in json.loads(raw)
    }


def audit_payload(repo: str) -> dict[str, Any]:
    labels = load_labels(repo)
    missing = sorted(set(LABELS) - set(labels))
    mismatched = [
        {
            "name": name,
            "expectedColor": color,
            "actualColor": labels[name]["color"],
            "expectedDescription": description,
            "actualDescription": labels[name]["description"],
        }
        for name, (color, description) in LABELS.items()
        if name in labels
        and (
            labels[name]["color"].lower() != color.lower()
            or labels[name]["description"] != description
        )
    ]
    return {
        "repo": repo,
        "ok": not missing and not mismatched,
        "missing": missing,
        "mismatched": mismatched,
        "required": sorted(LABELS),
    }


def cmd_audit(args: argparse.Namespace) -> None:
    payload = audit_payload(args.repo)
    print(json.dumps(payload, indent=2, sort_keys=True))
    if args.strict and not payload["ok"]:
        raise SystemExit(1)


def cmd_sync(args: argparse.Namespace) -> None:
    for name, (color, description) in LABELS.items():
        run_gh(
            [
                "label",
                "create",
                name,
                "--repo",
                args.repo,
                "--color",
                color,
                "--description",
                description,
                "--force",
            ]
        )
    cmd_audit(argparse.Namespace(repo=args.repo, strict=True))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(required=True)
    audit = sub.add_parser("audit", help="Report required label drift")
    audit.add_argument("--repo", required=True)
    audit.add_argument("--strict", action="store_true")
    audit.set_defaults(func=cmd_audit)
    sync = sub.add_parser("sync", help="Create or update required labels")
    sync.add_argument("--repo", required=True)
    sync.set_defaults(func=cmd_sync)
    return parser


def main() -> None:
    args = build_parser().parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
