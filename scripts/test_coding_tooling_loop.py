import json
from pathlib import Path
import unittest

import coding_tooling_loop as loop


class CodingToolingLoopTests(unittest.TestCase):
    def test_parse_json_result_requires_passing_status(self):
        payload = loop.parse_json_result(
            json.dumps({"status": "passed", "data": {"candidates": []}}),
            operation="plan",
        )
        self.assertEqual(payload["status"], "passed")

        with self.assertRaises(loop.LoopError):
            loop.parse_json_result(
                json.dumps({"status": "error", "diagnostics": [{"message": "bad"}]}),
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


if __name__ == "__main__":
    unittest.main()
