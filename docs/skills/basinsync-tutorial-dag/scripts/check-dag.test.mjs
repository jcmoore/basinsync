import assert from "node:assert/strict";
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { decodeDepth, encodeDepth, lintTutorials } from "./check-dag.mjs";

const scripts = dirname(fileURLToPath(import.meta.url));
const core = join(scripts, "check-dag.mjs");
const root = "0z000-root.md";
const left = "0z001-left.md";
const right = "0z001-right.md";
const merge = "0z002-merge.md";
const frontmatter = dependencies => `---\n${dependencies.length
  ? `prerequisites:\n${dependencies.map(name => `  - ${name}\n`).join("")}`
  : "prerequisites: []\n"}---\n\n# A tutorial\n`;

function fixture(files, work) {
  const directory = mkdtempSync(join(tmpdir(), "tutorial dag-"));
  try {
    for (const [name, content] of Object.entries(files)) writeFileSync(join(directory, name), content);
    return work(directory);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("known Lexidecimal depths, size boundaries, and arbitrary precision", () => {
  const examples = [
    [0n, "0z000"], [1n, "0z001"], [15n, "0z00f"], [16n, "0z0110"],
    [255n, "0z01ff"], [256n, "0z02100"], [65536n, "0z0410000"],
    [1n << 64n, "0z11010000000000000000"],
    [1n << 128n, `0z1201${"0".repeat(32)}`],
  ];
  for (const [depth, encoded] of examples) {
    assert.equal(encodeDepth(depth), encoded);
    assert.equal(decodeDepth(encoded), depth);
  }
  const huge = (1n << 4096n) + 19n;
  assert.equal(decodeDepth(encodeDepth(huge)), huge);
  for (const invalid of ["0yfff", "0z", "0Z000", "0z00F", "0z1000", "0z0100", "0z0f0", "0z0000"]) {
    assert.throws(() => decodeDepth(invalid), undefined, invalid);
  }
});

test("branches, multiple roots, CRLF/BOM, and nonminimal depths are valid", () => {
  fixture({
    [root]: frontmatter([]),
    [left]: `\uFEFF${frontmatter([root]).replaceAll("\n", "\r\n")}`,
    [right]: frontmatter([root]),
    [merge]: frontmatter([left, right]),
    "0z000-independent.md": frontmatter([]),
    "0z00f-deeper.md": frontmatter([merge]),
  }, directory => assert.deepEqual(lintTutorials(directory), { errors: [], tutorials: 6, edges: 5 }));
});

test("graph comparisons do not truncate depths to machine precision", () => {
  const depth = 1n << 128n;
  const parent = `${encodeDepth(depth)}-parent.md`;
  const child = `${encodeDepth(depth + 1n)}-child.md`;
  fixture({ [parent]: frontmatter([]), [child]: frontmatter([parent]) }, directory => {
    assert.deepEqual(lintTutorials(directory).errors, []);
    writeFileSync(join(directory, parent), frontmatter([child]));
    assert(lintTutorials(directory).errors.some(error => error.includes("must exceed")));
  });
});

for (const [description, files, expected] of [
  ["missing target", { [left]: frontmatter([root]) }, /missing prerequisite/],
  ["duplicate prerequisite", { [root]: frontmatter([]), [left]: frontmatter([root, root]) }, /duplicate prerequisite/],
  ["self edge", { [root]: frontmatter([root]) }, /self prerequisite/],
  ["equal depth", { [left]: frontmatter([right]), [right]: frontmatter([]) }, /must exceed/],
  ["decreasing depth", { [root]: frontmatter([left]), [left]: frontmatter([]) }, /must exceed/],
  ["cycle", { [root]: frontmatter([merge]), [left]: frontmatter([root]), [merge]: frontmatter([left]) }, /prerequisite cycle/],
  ["signed prefix", { "0yfff-root.md": frontmatter([]) }, /unsigned-Lexidecimal/],
  ["noncanonical prefix", { "0z0100-root.md": frontmatter([]) }, /noncanonical/],
  ["uppercase slug", { "0z000-Root.md": frontmatter([]) }, /lowercase-slug/],
  ["case mismatch", { "0z000-Root.md": frontmatter([]), [left]: frontmatter([root]) }, /missing prerequisite/],
  ["missing frontmatter", { [root]: "# No metadata\n" }, /missing frontmatter/],
  ["unclosed frontmatter", { [root]: "---\nprerequisites: []\n" }, /unterminated/],
  ["missing prerequisites", { [root]: "---\ntitle: root\n---\n" }, /expected prerequisites/],
  ["duplicate key", { [root]: "---\nprerequisites: []\nprerequisites: []\n---\n" }, /expected prerequisites/],
  ["null list", { [root]: "---\nprerequisites:\n---\n" }, /expected prerequisites/],
  ["path escape", { [left]: frontmatter(["../outside.md"]) }, /sibling Markdown filenames/],
  ["absolute path", { [left]: frontmatter(["/outside.md"]) }, /sibling Markdown filenames/],
  ["external URL", { [left]: frontmatter(["https://example.com/root.md"]) }, /sibling Markdown filenames/],
  ["wrong YAML indentation", { [left]: "---\nprerequisites:\n - 0z000-root.md\n---\n" }, /sibling Markdown filenames/],
]) {
  test(`rejects ${description}`, () => fixture(files, directory => {
    assert.match(lintTutorials(directory).errors.join("\n"), expected);
  }));
}

test("empty or missing directories fail with a useful CLI exit status", () => {
  fixture({}, directory => {
    assert.match(lintTutorials(directory).errors.join("\n"), /no tutorials/);
    for (const path of [directory, join(directory, "missing")]) {
      const result = spawnSync(process.execPath, [core, path], { encoding: "utf8" });
      assert.equal(result.status, 1);
      assert(result.stderr.length > 0);
    }
  });
});

test("CLI validates a fixture from an unrelated working directory", () => {
  fixture({ [root]: frontmatter([]) }, directory => {
    const result = spawnSync(process.execPath, [core, directory], { cwd: tmpdir(), encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /1 tutorials, 0 prerequisites/);
  });
});

test("a copied docs bundle and a path with spaces need no project source", () => {
  fixture({}, directory => {
    const docs = join(directory, "standalone docs");
    const skill = join(docs, "skills/basinsync-tutorial-dag");
    cpSync(resolve(scripts, ".."), skill, { recursive: true });
    mkdirSync(join(docs, "diy"));
    writeFileSync(join(docs, "diy", root), frontmatter([]));
    const result = spawnSync(process.execPath, [join(skill, "scripts/check-dag.mjs")], { cwd: tmpdir(), encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
  });
});
