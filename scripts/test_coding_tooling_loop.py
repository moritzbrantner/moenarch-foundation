import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import coding_tooling_loop as loop


class CodingToolingLoopTests(unittest.TestCase):
    def test_parse_json_result_requires_passing_status(self):
        payload = loop.parse_json_result(
            json.dumps({"status": "passed", "data": {"candidates": []}}),
            operation="plan",
        )
        self.assertEqual(payload["status"], "passed")

        for status in ("error", "warning", "unavailable"):
            with self.assertRaises(loop.LoopError):
                loop.parse_json_result(
                    json.dumps({"status": status, "diagnostics": [{"message": "bad"}]}),
                    operation="plan",
                )

    def test_remediation_candidates_fail_closed_on_malformed_shape(self):
        self.assertEqual(
            loop.remediation_candidates({"data": {"candidates": [{"id": "CT-RM-1"}]}}),
            [{"id": "CT-RM-1"}],
        )
        with self.assertRaises(loop.LoopError):
            loop.remediation_candidates({"data": {"candidates": "not-a-list"}})

    def test_tooling_commands_use_resolved_executable(self):
        self.assertEqual(
            loop.substitute_tooling(
                ["coding-tooling", "finding", "CT-1", "--json"],
                ["bun", "/tmp/coding-tooling/src/cli.ts"],
            ),
            ["bun", "/tmp/coding-tooling/src/cli.ts", "finding", "CT-1", "--json"],
        )
        self.assertEqual(
            loop.substitute_tooling(["cargo", "test"], ["coding-tooling"]),
            ["cargo", "test"],
        )

    def test_agent_command_receives_prompt_without_shell_interpolation(self):
        self.assertEqual(
            loop.render_agent_command(["codex", "exec", "{prompt}"], "fix\nthis"),
            ["codex", "exec", "fix\nthis"],
        )
        self.assertEqual(
            loop.render_agent_command(["claude", "-p"], "fix this"),
            ["claude", "-p", "fix this"],
        )

    def test_protected_control_changes_require_candidate_evidence(self):
        before = {path: "old" for path in loop.PROTECTED_CONTROL_PATHS}
        after = dict(before)
        after[".coding-tooling.json"] = "new"

        self.assertEqual(
            loop.changed_protected_paths(before, after, {"relatedFiles": []}),
            [".coding-tooling.json"],
        )
        self.assertEqual(
            loop.changed_protected_paths(
                before,
                after,
                {"relatedFiles": [".coding-tooling.json"]},
            ),
            [],
        )

    def test_prompt_preserves_fail_closed_boundaries_and_evidence(self):
        candidate = {
            "id": "CT-RM-ABC123",
            "findingIds": ["CT-1"],
            "relatedFiles": ["src/lib.rs"],
        }
        prompt = loop.build_prompt(
            candidate,
            failure_logs=[Path(".artifacts/coding-tooling/loop/failure.log")],
        )
        self.assertIn("CT-RM-ABC123", prompt)
        self.assertIn("CT-1", prompt)
        self.assertIn("Do not baseline or suppress findings", prompt)
        self.assertIn("Do not commit, push, switch branches", prompt)
        self.assertIn("failure.log", prompt)

    def test_candidate_verification_stops_on_first_failure(self):
        candidate = {"verification": [["first"], ["second"]]}
        with tempfile.TemporaryDirectory() as directory:
            artifact_dir = Path(directory)
            failure = subprocess.CompletedProcess(["first"], 1, "", "failed")
            with patch.object(loop, "run", return_value=failure) as mocked_run:
                failures = loop.run_candidate_verification(
                    Path(directory),
                    ["coding-tooling"],
                    candidate,
                    artifact_dir=artifact_dir,
                )

        self.assertEqual(len(failures), 1)
        self.assertEqual(mocked_run.call_count, 1)

    def test_review_candidate_requires_explicit_review(self):
        with self.assertRaisesRegex(loop.LoopError, "explicit review"):
            loop.repair_candidate(
                Path("."),
                ["coding-tooling"],
                ["codex", "exec", "{prompt}"],
                {"id": "CT-RM-REVIEW", "kind": "review"},
                artifact_dir=Path(".artifacts/coding-tooling/loop"),
                max_repairs=3,
            )

    def test_final_acceptance_does_not_run_full_tier_after_fast_failure(self):
        failure = Path(".artifacts/coding-tooling/loop/final-repository-fast.log")
        with patch.object(loop, "run_repository_gate", return_value=[failure]), patch.object(
            loop, "run_coding_tooling_tier"
        ) as full_tier:
            with self.assertRaises(loop.LoopError):
                loop.final_acceptance(
                    Path("."),
                    ["coding-tooling"],
                    tier="full",
                    artifact_dir=Path(".artifacts/coding-tooling/loop"),
                )

        full_tier.assert_not_called()


if __name__ == "__main__":
    unittest.main()
