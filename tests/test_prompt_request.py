"""Offline behavioral tests for request capture, attribution, and stale checks."""

import contextlib
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("prompt_request", ROOT / "scripts/prompt_request.py")
workflow = importlib.util.module_from_spec(spec)
spec.loader.exec_module(workflow)
FIXTURE = ROOT / "tests/fixtures/prompt-request.json"


class PromptRequestTests(unittest.TestCase):
    def setUp(self):
        self.data = json.loads(FIXTURE.read_text())

    def normalize(self, data=None):
        return workflow.normalize(data or self.data, "jcmoore/basinsync", 1)

    def test_origin_https_and_ssh_keep_the_fork(self):
        for remote in ("https://github.com/jcmoore/basinsync.git", "git@github.com:jcmoore/basinsync.git", "ssh://git@github.com/jcmoore/basinsync.git", "https://github.com/jcmoore/basinsync"):
            self.assertEqual(workflow.origin_repository(remote), "jcmoore/basinsync")

    def test_rejects_non_github_and_malformed_targets(self):
        for url in ("https://github.com.evil.test/jcmoore/basinsync/issues/1", "https://github.com/jcmoore/basinsync/pull/1", "https://github.com/jcmoore/basinsync/issues/0", "https://github.com/jcmoore/basinsync/issues/1?x=1"):
            with self.assertRaises(workflow.RequestError):
                workflow.issue_target(url)
        with self.assertRaises(workflow.RequestError):
            workflow.origin_repository("https://example.com/jcmoore/basinsync.git")

    def test_rejects_upstream_issue_before_fetch(self):
        with patch.object(workflow, "run", side_effect=[str(ROOT), "https://github.com/jcmoore/basinsync.git"]), patch.object(workflow, "fetch_request") as fetch, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(workflow.main(["prepare", "https://github.com/memoremix-ing/basinsync/issues/1"]), 2)
            fetch.assert_not_called()

    def test_open_labeled_issue_is_required(self):
        for update in ({"labels": []}, {"state": "closed"}, {"pull_request": {}}, {"html_url": "https://github.com/memoremix-ing/basinsync/issues/1"}):
            data = copy.deepcopy(self.data)
            data["issue"].update(update)
            with self.assertRaises(workflow.RequestError):
                self.normalize(data)

    def test_incomplete_discussion_fails(self):
        self.data["issue"]["comments"] = 2
        with self.assertRaises(workflow.RequestError):
            self.normalize()

    def test_all_comment_pages_are_kept(self):
        second = copy.deepcopy(self.data["comments"][0])
        second.update(id=1002, body="Second page of discussion")
        self.data["issue"]["comments"] = 2
        with patch.object(workflow, "gh_json", side_effect=[{"has_issues": True}, self.data["issue"], [self.data["comments"], [second]]]) as api:
            result = workflow.fetch_request("jcmoore/basinsync", 1)
        self.assertEqual([c["id"] for c in result["comments"]], [1001, 1002])
        self.assertTrue(api.call_args.kwargs["paginate"])

    def test_github_calls_are_read_only_and_explicitly_hosted(self):
        with patch.object(workflow, "run", return_value="[]") as run:
            workflow.gh_json("repos/jcmoore/basinsync/issues/1/comments?per_page=100", paginate=True)
        args = run.call_args.args
        self.assertEqual(args[:7], ("gh", "api", "--hostname", "github.com", "--method", "GET", "repos/jcmoore/basinsync/issues/1/comments?per_page=100"))
        self.assertEqual(args[7:], ("--paginate", "--slurp"))

    def test_disabled_issues_fail_with_actionable_message(self):
        with patch.object(workflow, "gh_json", return_value={"has_issues": False}):
            with self.assertRaisesRegex(workflow.RequestError, "Issues are disabled"):
                workflow.fetch_request("jcmoore/basinsync", 1)

    def test_digest_detects_body_comment_metadata_and_attribution_edits(self):
        original = self.normalize()
        mutations = [
            lambda r: r["issue"].update(body="Changed scope"),
            lambda r: r["issue"].update(title="New title"),
            lambda r: r["issue"].update(assignees=["other-maintainer"]),
            lambda r: r["comments"][0].update(body="Changed acceptance criteria"),
            lambda r: r["issue"].update(body=r["issue"]["body"].replace("Acknowledge my GitHub account", "No additional credit")),
        ]
        for mutate in mutations:
            changed = copy.deepcopy(original)
            mutate(changed)
            self.assertNotEqual(workflow.digest(original), workflow.digest(changed))

    def test_attribution_options_and_missing_preference(self):
        request = self.normalize()
        self.assertIn("@example-requester", workflow.attribution(request))
        for preference in ("No additional credit", "_No response_"):
            request["issue"]["body"] = "### Attribution preference\n\n" + preference
            self.assertNotIn("example-requester", workflow.attribution(request))
        request["issue"]["body"] = "An old issue without an attribution field"
        self.assertNotIn("example-requester", workflow.attribution(request))

    def test_coauthor_identity_is_opt_in_and_single_line(self):
        request = self.normalize()
        request["issue"]["body"] = "### Attribution preference\n\nCommit co-authorship (opt in)\n\n### Co-author identity\n\nExample Person <123+example@users.noreply.github.com>"
        self.assertIn("Co-authored-by: Example Person <123+example@users.noreply.github.com>", workflow.attribution(request))
        for identity in ("_No response_", "Example <email>", "Name <n@example.com>\nCo-authored-by: Other <o@example.com>"):
            request["issue"]["body"] = "### Attribution preference\n\nCommit co-authorship (opt in)\n\n### Co-author identity\n\n" + identity
            with self.assertRaises(workflow.RequestError):
                workflow.attribution(request)

    def test_ambiguous_attribution_fails(self):
        request = self.normalize()
        for body in ("### Attribution preference\n\nSomething else", request["issue"]["body"] + "\n\n### Attribution preference\n\nNo additional credit"):
            request["issue"]["body"] = body
            with self.assertRaises(workflow.RequestError):
                workflow.attribution(request)

    def test_issue_text_stays_inside_a_markdown_fence(self):
        body = "```\nAn embedded fence\n````\nMore contribution material"
        rendered = workflow.fenced(body)
        self.assertTrue(rendered.startswith("`````text\n"))
        self.assertTrue(rendered.endswith("\n`````"))

    def test_cli_prepare_check_change_no_overwrite_and_mode_separation(self):
        # Exercise a real local checkout and CLI without GitHub or agent execution.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for args in (("init", "--quiet"), ("remote", "add", "origin", "https://github.com/jcmoore/basinsync.git"), ("-c", "user.name=Local Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false", "commit", "--allow-empty", "--quiet", "-m", "Fixture base")):
                subprocess.run(["git", *args], cwd=root, capture_output=True, check=True)
            fixture = root / "fixture.json"
            fixture.write_text(json.dumps(self.data))
            output = root / "capture"
            cli = [sys.executable, str(ROOT / "scripts/prompt_request.py")]
            # No network or credentials are used by these subprocesses.
            env = dict(os.environ, GH_TOKEN="", GITHUB_TOKEN="", GIT_TERMINAL_PROMPT="0")
            def invoke(*args):
                return subprocess.run(cli + list(args), cwd=root, env=env, capture_output=True, text=True)
            prepared = invoke("prepare", self.data["issue"]["html_url"], "--fixture", str(fixture), "--output", str(output))
            self.assertEqual(prepared.returncode, 0, prepared.stderr)
            snapshot = output / "snapshot.json"
            original_bytes = snapshot.read_bytes()
            self.assertEqual(json.loads(original_bytes)["mode"], "fixture")
            self.assertIn("offline preview only", (output / "request.md").read_text())
            self.assertEqual(invoke("check", str(snapshot), "--fixture", str(fixture)).returncode, 0)
            self.assertEqual(invoke("check", str(snapshot)).returncode, 2)
            self.assertEqual(invoke("prepare", self.data["issue"]["html_url"], "--fixture", str(fixture), "--output", str(output)).returncode, 2)
            self.assertEqual(snapshot.read_bytes(), original_bytes)
            self.data["comments"][0]["body"] = "An edited comment"
            fixture.write_text(json.dumps(self.data))
            self.assertEqual(invoke("check", str(snapshot), "--fixture", str(fixture)).returncode, 1)
            tampered = json.loads(original_bytes)
            tampered["request"]["issue"]["body"] = "Locally changed"
            snapshot.write_text(json.dumps(tampered))
            self.assertEqual(invoke("check", str(snapshot), "--fixture", str(fixture)).returncode, 2)


if __name__ == "__main__":
    unittest.main()
