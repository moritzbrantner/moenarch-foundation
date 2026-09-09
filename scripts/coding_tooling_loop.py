#!/usr/bin/env python3
"""Bounded local remediation loop driven by coding-tooling evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
from typing import Any, Sequence

PROTECTED_CONTROL_PATHS = (
    ".agent-loop.toml",
    ".coding-tooling.json",
    ".github/workflows/workspace-ci.yml",
    "scripts/check-fast.sh",
)


class LoopError(RuntimeError):
    pass


def run(command: Sequence[str], *, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        list(command),
        cwd=cwd,
        check=False,
        text=True,
        capture_output=True,
    )


def repository_root() -> Path:
    result = run(["git", "rev-parse", "--show-toplevel"], cwd=Path.cwd())
    if result.returncode != 0:
        raise LoopError(result.stderr.strip() or "not inside a Git repository")
    return Path(result.stdout.strip()).resolve()


def resolve_coding_tooling(root: Path) -> list[str]:
    override = os.environ.get("CODING_TOOLING_COMMAND")
    if override:
        command = shlex.split(override)
        if not command:
            raise LoopError("CODING_TOOLING_COMMAND is empty")
        return command

    executable = shutil.which("coding-tooling")
    if executable:
        return [executable]

    tooling_dir = Path(
        os.environ.get("CODING_TOOLING_DIR", str(root.parent / "coding-tooling"))
    )
    cli = tooling_dir / "src" / "cli.ts"
    bun = shutil.which("bun")
    if cli.is_file() and bun:
        return [bun, str(cli)]

    raise LoopError(
        "coding-tooling is required. Install it, set CODING_TOOLING_COMMAND, "
        "or set CODING_TOOLING_DIR to a checkout."
    )


def resolve_agent_command(explicit: str | None) -> list[str] | None:
    raw = explicit or os.environ.get("CODING_TOOLING_LOOP_AGENT_COMMAND")
    if raw:
        command = shlex.split(raw)
        if not command:
            raise LoopError("agent command is empty")
        return command

    codex = shutil.which("codex")
    return [codex, "exec", "{prompt}"] if codex else None


def parse_json_result(stdout: str, *, operation: str) -> dict[str, Any]:
    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as exc:
        raise LoopError(f"{operation} returned invalid JSON: {exc}") from exc
    if not isinstance(payload, dict):
        raise LoopError(f"{operation} returned a non-object JSON result")
    if payload.get("status") not in {"passed", "warning"}:
        raise LoopError(f"{operation} did not pass: {payload.get('diagnostics', [])}")
    return payload


def remediation_candidates(payload: dict[str, Any]) -> list[dict[str, Any]]:
    data = payload.get("data")
    if not isinstance(data, dict) or not isinstance(data.get("candidates"), list):
        raise LoopError("remediation plan is missing candidates")
    candidates = data["candidates"]
    if not all(isinstance(candidate, dict) for candidate in candidates):
        raise LoopError("remediation plan contains a malformed candidate")
    return candidates


def substitute_tooling(
    command: Sequence[str], tooling_command: Sequence[str]
) -> list[str]:
    if command and command[0] == "coding-tooling":
        return [*tooling_command, *command[1:]]
    return list(command)


def render_agent_command(command: Sequence[str], prompt: str) -> list[str]:
    rendered: list[str] = []
    replaced = False
    for token in command:
        if "{prompt}" in token:
            rendered.append(token.replace("{prompt}", prompt))
            replaced = True
        else:
            rendered.append(token)
    if not replaced:
        rendered.append(prompt)
    return rendered


def allowed_control_paths(candidate: dict[str, Any]) -> set[str]:
    paths: set[str] = set()
    related = candidate.get("relatedFiles", [])
    if isinstance(related, list):
        paths.update(path for path in related if isinstance(path, str))
    scaffolds = candidate.get("scaffolds", [])
    if isinstance(scaffolds, list):
        for scaffold in scaffolds:
            if isinstance(scaffold, dict) and isinstance(scaffold.get("path"), str):
                paths.add(scaffold["path"])
    return paths


def control_hashes(root: Path) -> dict[str, str | None]:
    hashes: dict[str, str | None] = {}
    for relative in PROTECTED_CONTROL_PATHS:
        path = root / relative
        hashes[relative] = (
            hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
        )
    return hashes


def changed_protected_paths(
    before: dict[str, str | None],
    after: dict[str, str | None],
    candidate: dict[str, Any],
) -> list[str]:
    allowed = allowed_control_paths(candidate)
    return sorted(
        path
        for path in PROTECTED_CONTROL_PATHS
        if before.get(path) != after.get(path) and path not in allowed
    )


def git_identity(root: Path) -> tuple[str, str]:
    branch = run(["git", "branch", "--show-current"], cwd=root)
    head = run(["git", "rev-parse", "HEAD"], cwd=root)
    if branch.returncode != 0 or head.returncode != 0:
        raise LoopError("unable to read Git branch/head identity")
    return branch.stdout.strip(), head.stdout.strip()


def worktree_fingerprint(root: Path) -> str:
    diff = run(["git", "diff", "--binary", "HEAD"], cwd=root)
    status = run(["git", "status", "--porcelain=v1", "--untracked-files=all"], cwd=root)
    untracked = run(["git", "ls-files", "--others", "--exclude-standard", "-z"], cwd=root)
    if diff.returncode or status.returncode or untracked.returncode:
        raise LoopError("unable to fingerprint working tree")

    digest = hashlib.sha256()
    digest.update(diff.stdout.encode())
    digest.update(b"\0")
    digest.update(status.stdout.encode())
    for relative in filter(None, untracked.stdout.split("\0")):
        path = root / relative
        if path.is_file():
            digest.update(relative.encode())
            digest.update(b"\0")
            digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()


def write_log(path: Path, result: subprocess.CompletedProcess[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        f"returncode={result.returncode}\n\nSTDOUT\n{result.stdout}\n\nSTDERR\n{result.stderr}\n",
        encoding="utf-8",
    )


def remediation_plan(
    root: Path,
    tooling: Sequence[str],
    *,
    include_baseline: bool,
    artifact_dir: Path,
) -> list[dict[str, Any]]:
    args = ["remediation", "plan"]
    if include_baseline:
        args.append("--include-baseline")
    args.append("--json")
    result = run([*tooling, *args], cwd=root)
    write_log(artifact_dir / "remediation-plan.log", result)
    if result.returncode != 0:
        raise LoopError(
            f"coding-tooling remediation plan failed; see "
            f"{artifact_dir / 'remediation-plan.log'}"
        )
    return remediation_candidates(
        parse_json_result(result.stdout, operation="coding-tooling remediation plan")
    )


def build_prompt(
    candidate: dict[str, Any],
    *,
    failure_logs: Sequence[Path],
) -> str:
    failure_note = ""
    if failure_logs:
        failure_note = (
            "\nPrevious verification failed. Read these local logs before editing:\n"
            + "\n".join(f"- {path}" for path in failure_logs)
            + "\n"
        )
    return f"""Resolve exactly this coding-tooling remediation candidate in moenarch-foundation.

Read AGENTS.md and CONTEXT.md before editing. Preserve repository ownership and release boundaries.
Do not baseline or suppress findings. Do not weaken, remove, or bypass validation.
Do not commit, push, switch branches, create tags/releases, publish packages, or bump package versions merely to make source work pass.
The loop protects these control paths unless the candidate itself names one: {", ".join(PROTECTED_CONTROL_PATHS)}.
Run the narrowest meaningful verification after the fix. The outer loop will run the recorded candidate verification and the repository handoff gate.
{failure_note}
Candidate:
{json.dumps(candidate, indent=2, sort_keys=True)}
"""


def apply_scaffolds(
    root: Path,
    tooling: Sequence[str],
    candidate: dict[str, Any],
    *,
    artifact_dir: Path,
) -> bool:
    scaffolds = candidate.get("scaffolds", [])
    if not isinstance(scaffolds, list):
        raise LoopError("candidate scaffolds are malformed")
    changed = False
    for index, scaffold in enumerate(scaffolds, start=1):
        if not isinstance(scaffold, dict):
            raise LoopError("candidate scaffold is malformed")
        command = scaffold.get("command")
        if not isinstance(command, list) or not all(isinstance(part, str) for part in command):
            raise LoopError("candidate scaffold command is malformed")
        result = run(substitute_tooling(command, tooling), cwd=root)
        log_path = artifact_dir / f"scaffold-{index}.log"
        write_log(log_path, result)
        if result.returncode != 0:
            raise LoopError(f"deterministic scaffold failed; see {log_path}")
        changed = True
    return changed


def invoke_agent(
    root: Path,
    agent_command: Sequence[str],
    candidate: dict[str, Any],
    *,
    failure_logs: Sequence[Path],
    artifact_dir: Path,
    attempt: int,
) -> None:
    result = run(
        render_agent_command(
            agent_command,
            build_prompt(candidate, failure_logs=failure_logs),
        ),
        cwd=root,
    )
    log_path = artifact_dir / f"agent-attempt-{attempt}.log"
    write_log(log_path, result)
    if result.returncode != 0:
        raise LoopError(f"agent command failed; see {log_path}")


def verification_commands(candidate: dict[str, Any]) -> list[list[str]]:
    raw = candidate.get("verification", [])
    if not isinstance(raw, list):
        raise LoopError("candidate verification is malformed")
    commands: list[list[str]] = []
    for command in raw:
        if not isinstance(command, list) or not all(isinstance(part, str) for part in command):
            raise LoopError("candidate verification command is malformed")
        commands.append(command)
    return commands


def run_candidate_verification(
    root: Path,
    tooling: Sequence[str],
    candidate: dict[str, Any],
    *,
    artifact_dir: Path,
) -> list[Path]:
    failures: list[Path] = []
    for index, command in enumerate(verification_commands(candidate), start=1):
        result = run(substitute_tooling(command, tooling), cwd=root)
        log_path = artifact_dir / f"verification-{index}.log"
        write_log(log_path, result)
        if result.returncode != 0:
            failures.append(log_path)
    return failures


def run_repository_gate(root: Path, *, artifact_dir: Path, label: str) -> list[Path]:
    result = run(["bash", "scripts/check-fast.sh"], cwd=root)
    log_path = artifact_dir / f"{label}-repository-fast.log"
    write_log(log_path, result)
    return [] if result.returncode == 0 else [log_path]


def run_coding_tooling_tier(
    root: Path,
    tooling: Sequence[str],
    tier: str,
    *,
    artifact_dir: Path,
    label: str,
) -> list[Path]:
    report = artifact_dir / f"{label}-{tier}.json"
    result = run(
        [
            *tooling,
            "run",
            "--tier",
            tier,
            "--strict",
            "--report",
            str(report),
            "--json",
        ],
        cwd=root,
    )
    log_path = artifact_dir / f"{label}-{tier}.log"
    write_log(log_path, result)
    return [] if result.returncode == 0 else [log_path]


def repair_candidate(
    root: Path,
    tooling: Sequence[str],
    agent_command: Sequence[str] | None,
    candidate: dict[str, Any],
    *,
    artifact_dir: Path,
    max_repairs: int,
) -> None:
    candidate_id = str(candidate.get("id", "candidate"))
    candidate_dir = artifact_dir / candidate_id
    candidate_dir.mkdir(parents=True, exist_ok=True)
    failure_logs: list[Path] = []

    for attempt in range(1, max_repairs + 1):
        branch_before, head_before = git_identity(root)
        fingerprint_before = worktree_fingerprint(root)
        controls_before = control_hashes(root)

        did_mutate = False
        if attempt == 1 and candidate.get("kind") == "deterministic-scaffold":
            did_mutate = apply_scaffolds(
                root, tooling, candidate, artifact_dir=candidate_dir
            )

        if not did_mutate:
            if agent_command is None:
                raise LoopError(
                    f"{candidate_id} requires an agent, but no agent command is available"
                )
            invoke_agent(
                root,
                agent_command,
                candidate,
                failure_logs=failure_logs,
                artifact_dir=candidate_dir,
                attempt=attempt,
            )

        if git_identity(root) != (branch_before, head_before):
            raise LoopError(
                f"{candidate_id} moved Git branch/HEAD; only working-tree edits are allowed"
            )

        unauthorized = changed_protected_paths(
            controls_before, control_hashes(root), candidate
        )
        if unauthorized:
            raise LoopError(
                f"{candidate_id} changed protected validation controls without "
                f"candidate evidence: {', '.join(unauthorized)}"
            )

        if worktree_fingerprint(root) == fingerprint_before:
            raise LoopError(f"{candidate_id} made no repository progress")

        failure_logs = run_candidate_verification(
            root, tooling, candidate, artifact_dir=candidate_dir
        )
        failure_logs.extend(
            run_repository_gate(
                root,
                artifact_dir=candidate_dir,
                label=f"attempt-{attempt}",
            )
        )
        if not failure_logs:
            return

    raise LoopError(
        f"{candidate_id} did not converge after {max_repairs} repair attempts; "
        f"see {candidate_dir}"
    )


def run_readiness(root: Path, *, artifact_dir: Path) -> None:
    result = run(["bash", "scripts/check-agent-readiness.sh"], cwd=root)
    log_path = artifact_dir / "readiness.log"
    write_log(log_path, result)
    if result.returncode != 0:
        raise LoopError(f"agent readiness failed; see {log_path}")


def final_acceptance(
    root: Path,
    tooling: Sequence[str],
    *,
    tier: str,
    artifact_dir: Path,
) -> None:
    failures = run_repository_gate(root, artifact_dir=artifact_dir, label="final")
    failures.extend(
        run_coding_tooling_tier(
            root,
            tooling,
            tier,
            artifact_dir=artifact_dir,
            label="final",
        )
    )
    if failures:
        raise LoopError(f"final acceptance failed; see {failures[0]}")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Bounded coding-tooling remediation loop for moenarch-foundation"
    )
    parser.add_argument(
        "--agent-command",
        help="Agent command. Use {prompt} as a placeholder; otherwise the prompt is appended.",
    )
    parser.add_argument(
        "--include-baseline",
        action="store_true",
        help="Opt into baselined findings in addition to new findings.",
    )
    parser.add_argument("--max-candidates", type=int, default=5)
    parser.add_argument("--max-repairs", type=int, default=3)
    parser.add_argument("--final-tier", default="full")
    parser.add_argument("--skip-readiness", action="store_true")
    args = parser.parse_args(argv)

    if args.max_candidates < 1 or args.max_repairs < 1:
        parser.error("--max-candidates and --max-repairs must be positive")

    root = repository_root()
    artifact_dir = root / ".artifacts" / "coding-tooling" / "loop"
    artifact_dir.mkdir(parents=True, exist_ok=True)
    tooling = resolve_coding_tooling(root)
    agent_command = resolve_agent_command(args.agent_command)

    if not args.skip_readiness:
        run_readiness(root, artifact_dir=artifact_dir)

    for index in range(1, args.max_candidates + 1):
        candidates = remediation_plan(
            root,
            tooling,
            include_baseline=args.include_baseline,
            artifact_dir=artifact_dir,
        )
        if not candidates:
            final_acceptance(
                root,
                tooling,
                tier=args.final_tier,
                artifact_dir=artifact_dir,
            )
            scope = "new/baseline" if args.include_baseline else "new"
            print(
                f"coding-tooling loop: converged with no active {scope} "
                f"remediation candidates"
            )
            return 0

        candidate = candidates[0]
        print(
            f"coding-tooling loop: candidate {index}/{args.max_candidates} "
            f"{candidate.get('id', '<unknown>')}: {candidate.get('summary', '')}"
        )
        repair_candidate(
            root,
            tooling,
            agent_command,
            candidate,
            artifact_dir=artifact_dir,
            max_repairs=args.max_repairs,
        )

    remaining = remediation_plan(
        root,
        tooling,
        include_baseline=args.include_baseline,
        artifact_dir=artifact_dir,
    )
    if remaining:
        raise LoopError(
            f"coding-tooling loop reached --max-candidates with "
            f"{len(remaining)} candidate(s) still active"
        )
    final_acceptance(
        root,
        tooling,
        tier=args.final_tier,
        artifact_dir=artifact_dir,
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except LoopError as exc:
        print(f"coding-tooling loop: {exc}", file=sys.stderr)
        raise SystemExit(1)
