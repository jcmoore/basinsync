---
name: basinsync-tutorial-dag
description: Rename or audit basinsync tutorials using Lexidecimal depth prefixes and prerequisite frontmatter; check dependency integrity after tutorial graph changes.
---

[check-dag]: <scripts/check-dag.mjs>
[lexidecimal]: <../../glossary/lexidecimal.md>

# Tutorial dependency DAG

Name each `diy/` tutorial `<depth>-<slug>.md`, encoding its nonnegative depth in
canonical unsigned *Lexidecimal*[^][lexidecimal]. Depth must exceed every direct
prerequisite's depth; equal-depth branches and depth gaps are valid. Use zero
for a new root. Slugs identify tutorials; depth is not a sequence number.

Put prerequisite sibling filenames in frontmatter, before reference definitions:

```yaml
---
prerequisites:
  - 0z000-shallow-epilogue-commits.md
---
```

Roots use `prerequisites: []`. This deliberately small schema accepts only that
key, with an empty array or an unquoted block list. List direct requirements;
ordinary Markdown references need not be prerequisites.

When changing dependencies, update affected filenames, frontmatter, and reference
labels/targets together. Preserve tutorial scope and allow independent branches.

Run the bundled *Node.js linter*[^][check-dag]. Node.js 18+ is required on all
platforms. It locates `diy/` relative to the skill regardless of working directory;
an optional argument selects another tutorial directory. It rejects invalid
depths, malformed frontmatter, missing/duplicate/self prerequisites, cycles,
and non-increasing edges. It does not judge whether the stated prerequisites
are sufficient.

From this skill directory:

```sh
node scripts/check-dag.mjs
node --test scripts/check-dag.test.mjs
```
