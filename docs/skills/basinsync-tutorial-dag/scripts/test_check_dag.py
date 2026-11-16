#!/usr/bin/env python3
"""Dependency graph, layout, CLI, and standalone skill regression tests."""
import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path

from check_dag import decode_height, encode_height, lint_tutorials

SCRIPTS = Path(__file__).resolve().parent
CORE = SCRIPTS / "check_dag.py"
ROOT = "0z000/root"
LEFT = "0z001/left"
RIGHT = "0z001/right"
MERGE = "0z002/merge"


def frontmatter(dependencies):
    field = ("prerequisites:\n" + "".join(f"  - {name}\n" for name in dependencies)
             if dependencies else "prerequisites: []\n")
    return f"---\n{field}---\n\n# A tutorial\n"


def write(directory, name, text):
    path = directory / name
    if path.suffix.lower() != ".md":
        path /= "README.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


@contextmanager
def fixture(files):
    with tempfile.TemporaryDirectory(prefix="tutorial dag-") as directory:
        directory = Path(directory)
        for name, text in files.items():
            write(directory, name, text)
        yield directory


def cli(*args, script=CORE):
    return subprocess.run([sys.executable, str(script), *map(str, args)],
                          cwd=tempfile.gettempdir(), capture_output=True, text=True)


class TutorialDagTests(unittest.TestCase):
    def test_lexidecimal_boundaries_and_arbitrary_precision(self):
        examples = [(0, "0z000"), (1, "0z001"), (15, "0z00f"), (16, "0z0110"),
                    (255, "0z01ff"), (256, "0z02100"), (65536, "0z0410000"),
                    (1 << 64, "0z11010000000000000000"),
                    (1 << 128, "0z1201" + "0" * 32)]
        for height, encoded in examples:
            with self.subTest(height=height):
                self.assertEqual(encode_height(height), encoded)
                self.assertEqual(decode_height(encoded), height)
        huge = (1 << 4096) + 19
        self.assertEqual(decode_height(encode_height(huge)), huge)
        for invalid in ["0yfff", "0z", "0Z000", "0z00F", "0z1000", "0z0100", "0z0f0", "0z0000"]:
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                decode_height(invalid)
        for invalid in [-1, True, 1.5, "1"]:
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                encode_height(invalid)

    def test_branches_roots_bom_crlf_and_height_gaps(self):
        with fixture({ROOT: frontmatter([]),
                      LEFT: "\ufeff" + frontmatter([ROOT]).replace("\n", "\r\n"),
                      RIGHT: frontmatter([ROOT]), MERGE: frontmatter([LEFT, RIGHT]),
                      "0z000/independent": frontmatter([]),
                      "0z00f/deeper": frontmatter([MERGE])}) as directory:
            self.assertEqual(lint_tutorials(directory), {"errors": [], "tutorials": 6, "edges": 5})

    def test_graph_heights_keep_arbitrary_precision(self):
        height = 1 << 128
        parent, child = f"{encode_height(height)}/parent", f"{encode_height(height + 1)}/child"
        with fixture({parent: frontmatter([]), child: frontmatter([parent])}) as directory:
            self.assertEqual(lint_tutorials(directory)["errors"], [])
            write(directory, parent, frontmatter([child]))
            self.assertIn("must exceed", "\n".join(lint_tutorials(directory)["errors"]))

    def test_slug_uniqueness_across_folders_and_cli_failure(self):
        first, second = "0z000/shared-topic", "0z001/shared-topic"
        with fixture({first: frontmatter([]), second: frontmatter([first])}) as directory:
            errors = lint_tutorials(directory)["errors"]
            self.assertEqual(len(errors), 1)
            self.assertIn('duplicate tutorial slug "shared-topic"', errors[0])
            self.assertIn(first, errors[0])
            self.assertIn(second, errors[0])
            result = cli(directory)
            self.assertEqual(result.returncode, 1)
            self.assertIn("duplicate tutorial slug", result.stderr)

    def test_nested_tutorials_and_repeated_height_names(self):
        independent = "0z000/independent"
        left = ROOT + "/0z001/left"
        right = independent + "/0z001/right"
        merge = left + "/0z002/merge"
        with fixture({ROOT: frontmatter([]), independent: frontmatter([]),
                      left: frontmatter([ROOT]), right: frontmatter([independent]),
                      merge: frontmatter([left, right])}) as directory:
            self.assertEqual(lint_tutorials(directory), {"errors": [], "tutorials": 5, "edges": 4})
            write(directory, left, frontmatter([merge]))
            errors = "\n".join(lint_tutorials(directory)["errors"])
            self.assertIn("must exceed", errors)
            self.assertIn("prerequisite cycle", errors)

    def test_tutorial_slugs_cannot_repeat_at_any_nesting_level(self):
        for duplicate in [ROOT + "/0z001/root", ROOT + "/0z001/left"]:
            with self.subTest(duplicate=duplicate), fixture({ROOT: frontmatter([]),
                    LEFT: frontmatter([ROOT]), duplicate: frontmatter([ROOT])}) as directory:
                self.assertIn("duplicate tutorial slug", "\n".join(lint_tutorials(directory)["errors"]))

    def test_height_shaped_tutorial_names_are_not_exempt(self):
        first, second = "0z000/0z001", "0z001/0z001"
        with fixture({first: frontmatter([]), second: frontmatter([first])}) as directory:
            self.assertIn('duplicate tutorial slug "0z001"', "\n".join(lint_tutorials(directory)["errors"]))

    def test_supporting_folder_names_and_filenames_may_repeat(self):
        for first, second in [(ROOT + "/assets", LEFT + "/assets"),
                              (ROOT + "/left", LEFT),
                              (ROOT + "/assets/archive", LEFT + "/media/archive"),
                              (ROOT + "/benchmark/data", LEFT + "/benchmark/data")]:
            with self.subTest(first=first, second=second), fixture({ROOT: frontmatter([]),
                    LEFT: frontmatter([ROOT])}) as directory:
                (directory / first).mkdir(parents=True, exist_ok=True)
                (directory / second).mkdir(parents=True, exist_ok=True)
                for folder in [first, second]:
                    (directory / folder / "summary.json").write_text("{}", encoding="utf-8")
                self.assertEqual(lint_tutorials(directory)["errors"], [])

    def test_namespace_excludes_other_documentation_folders(self):
        with fixture({}) as directory:
            docs = directory / "docs"
            write(docs / "diy", "0z000/glossary", frontmatter([]))
            (docs / "glossary").mkdir()
            (docs / "reference/glossary").mkdir(parents=True)
            self.assertEqual(lint_tutorials(docs / "diy"),
                             {"errors": [], "tutorials": 1, "edges": 0})

    def test_distinct_slugs_may_share_trailing_words(self):
        with fixture({"0z000/shared-topic": frontmatter([]),
                      "0z001/other-shared-topic": frontmatter(["0z000/shared-topic"])}) as directory:
            self.assertEqual(lint_tutorials(directory)["errors"], [])

    def test_invalid_graphs_layouts_and_frontmatter(self):
        cases = [
            ("missing target", {LEFT: frontmatter([ROOT])}, "missing prerequisite"),
            ("duplicate edge", {ROOT: frontmatter([]), LEFT: frontmatter([ROOT, ROOT])}, "duplicate prerequisite"),
            ("self edge", {ROOT: frontmatter([ROOT])}, "self prerequisite"),
            ("equal height", {LEFT: frontmatter([RIGHT]), RIGHT: frontmatter([])}, "must exceed"),
            ("decreasing height", {ROOT: frontmatter([LEFT]), LEFT: frontmatter([])}, "must exceed"),
            ("cycle", {ROOT: frontmatter([MERGE]), LEFT: frontmatter([ROOT]), MERGE: frontmatter([LEFT])}, "prerequisite cycle"),
            ("signed height", {"0yfff/root": frontmatter([])}, "unsigned Lexidecimal"),
            ("noncanonical height", {"0z0100/root": frontmatter([])}, "noncanonical"),
            ("uppercase height", {"0Z000/root": frontmatter([])}, "unsigned Lexidecimal"),
            ("uppercase slug", {"0z000/Root": frontmatter([])}, "lowercase-slug"),
            ("wrong README casing", {"0z000/root/readme.md": frontmatter([])}, "missing README.md"),
            ("wrong README extension", {"0z000/root/README.MD": frontmatter([])}, "missing README.md"),
            ("previous file layout", {"0z000/root.md": frontmatter([])}, "tutorial folder containing README.md"),
            ("case mismatch", {"0z000/Root": frontmatter([]), LEFT: frontmatter([ROOT])}, "missing prerequisite"),
            ("old flat layout", {"0z000-root.md": frontmatter([])}, "unsigned-Lexidecimal-height"),
            ("nested folder", {"0z000/nested/root/README.md": frontmatter([])}, "missing README.md"),
            ("missing nested height", {ROOT: frontmatter([]), ROOT + "/child": frontmatter([ROOT])}, "alternating"),
            ("noncanonical nested height", {ROOT: frontmatter([]), ROOT + "/0z0100/child": frontmatter([ROOT])}, "noncanonical"),
            ("missing frontmatter", {ROOT: "# No metadata\n"}, "missing frontmatter"),
            ("unclosed frontmatter", {ROOT: "---\nprerequisites: []\n"}, "unterminated"),
            ("missing prerequisites", {ROOT: "---\ntitle: root\n---\n"}, "expected prerequisites"),
            ("duplicate key", {ROOT: "---\nprerequisites: []\nprerequisites: []\n---\n"}, "expected prerequisites"),
            ("null list", {ROOT: "---\nprerequisites:\n---\n"}, "expected prerequisites"),
            ("path escape", {LEFT: frontmatter(["../outside.md"])}, "paths relative to diy/"),
            ("absolute path", {LEFT: frontmatter(["/outside.md"])}, "paths relative to diy/"),
            ("URL", {LEFT: frontmatter(["https://example.com/root.md"])}, "paths relative to diy/"),
            ("backslash", {LEFT: frontmatter(["0z000\\root.md"])}, "paths relative to diy/"),
            ("non-normal path", {LEFT: frontmatter(["0z000/../root.md"])}, "paths relative to diy/"),
            ("old prerequisite", {LEFT: frontmatter(["0z000-root.md"])}, "paths relative to diy/"),
            ("file prerequisite", {LEFT: frontmatter(["0z000/root.md"])}, "paths relative to diy/"),
            ("README prerequisite", {LEFT: frontmatter(["0z000/root/README.md"])}, "paths relative to diy/"),
            ("docs prefix prerequisite", {LEFT: frontmatter(["diy/0z000/root"])}, "paths relative to diy/"),
            ("trailing slash prerequisite", {LEFT: frontmatter(["0z000/root/"])}, "paths relative to diy/"),
            ("wrong indentation", {LEFT: "---\nprerequisites:\n - 0z000/root.md\n---\n"}, "paths relative to diy/"),
        ]
        for description, files, expected in cases:
            with self.subTest(description=description), fixture(files) as directory:
                self.assertIn(expected, "\n".join(lint_tutorials(directory)["errors"]))

    def test_symlinks_do_not_escape_the_bundle(self):
        with fixture({ROOT: frontmatter([])}) as directory:
            link = directory / "0z001"
            try:
                link.symlink_to(directory / "0z000", target_is_directory=True)
            except (OSError, NotImplementedError):
                self.skipTest("symlink creation is unavailable")
            self.assertIn("must not be symlinks", "\n".join(lint_tutorials(directory)["errors"]))
            link.unlink()
            link = directory / "0z000/alias"
            link.symlink_to(directory / ROOT, target_is_directory=True)
            self.assertIn("symlink", "\n".join(lint_tutorials(directory)["errors"]))
            link.unlink()
            write(directory, "0z000/independent", frontmatter([]))
            readme = directory / ROOT / "README.md"
            readme.unlink()
            readme.symlink_to(directory / "0z000/independent/README.md")
            self.assertIn("regular file", "\n".join(lint_tutorials(directory)["errors"]))

    def test_tutorial_folders_allow_supporting_assets(self):
        with fixture({ROOT: frontmatter([])}) as directory:
            assets = directory / ROOT / "assets"
            assets.mkdir()
            (assets / "diagram.svg").write_text("<svg/>", encoding="utf-8")
            self.assertEqual(lint_tutorials(directory)["errors"], [])

    def test_empty_missing_and_excess_arguments_fail(self):
        with fixture({}) as directory:
            self.assertIn("no tutorials found", lint_tutorials(directory)["errors"])
            for args in [(directory,), (directory / "missing",), (directory, directory)]:
                with self.subTest(args=args):
                    result = cli(*args)
                    self.assertEqual(result.returncode, 1)
                    self.assertTrue(result.stderr)

    def test_cli_works_from_an_unrelated_directory(self):
        with fixture({ROOT: frontmatter([])}) as directory:
            result = cli(directory)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("1 tutorials, 0 prerequisites", result.stdout)

    def test_copied_docs_bundle_needs_no_project_source(self):
        with fixture({}) as directory:
            docs = directory / "standalone docs"
            skill = docs / "skills/basinsync-tutorial-dag"
            shutil.copytree(SCRIPTS.parent, skill, ignore=shutil.ignore_patterns("__pycache__"))
            write(docs / "diy", ROOT, frontmatter([]))
            write(docs / "diy", ROOT + "/0z001/child", frontmatter([ROOT]))
            result = cli(script=skill / "scripts/check_dag.py")
            self.assertEqual(result.returncode, 0, result.stderr)
            (docs / "child").mkdir()
            outside = cli(script=skill / "scripts/check_dag.py")
            self.assertEqual(outside.returncode, 0, outside.stderr)
            (docs / "diy" / ROOT / "child").mkdir()
            supporting = cli(script=skill / "scripts/check_dag.py")
            self.assertEqual(supporting.returncode, 0, supporting.stderr)
            write(docs / "diy", "0z002/child", frontmatter([ROOT]))
            duplicate = cli(script=skill / "scripts/check_dag.py")
            self.assertEqual(duplicate.returncode, 1)
            self.assertIn('duplicate tutorial slug "child"', duplicate.stderr)


if __name__ == "__main__":
    unittest.main()
