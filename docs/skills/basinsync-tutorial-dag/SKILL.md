---
name: basinsync-tutorial-dag
description: Rename or audit basinsync tutorials grouped in Lexidecimal height folders with prerequisite frontmatter; check dependency integrity after tutorial graph changes.
---

[check_dag]: <scripts/check_dag.py>
[lexidecimal]: <../../glossary/lexidecimal.md>

# Tutorial dependency DAG

Name each tutorial `diy/<height>/<slug>.md`, encoding its nonnegative height in
canonical unsigned *Lexidecimal*[^][lexidecimal]. Height must exceed every direct
prerequisite's height; equal-height branches and height gaps are valid. Use zero
for a new root. Unprefixed slugs must be unique across all heights; height is not
a sequence number.

Put prerequisite paths relative to `diy/` in frontmatter, before reference definitions:

```yaml
---
prerequisites:
  - 0z000/shallow-epilogue-commits.md
---
```

Roots use `prerequisites: []`. This deliberately small schema accepts only that
key, with an empty array or an unquoted block list. Use forward slashes on every
platform. List direct requirements; ordinary Markdown references need not be prerequisites.

When changing dependencies, update affected folders, frontmatter, and reference
labels/targets together. Preserve tutorial scope and allow independent branches.

Run the bundled *Python linter*[^][check_dag]. Python 3.9+ is required, without
third-party packages. It locates `diy/` relative to the skill regardless of working
directory; an optional argument selects another tutorial directory. It rejects
invalid layouts/heights, duplicate slugs, malformed frontmatter,
missing/duplicate/self prerequisites, cycles, and non-increasing edges. It does
not judge whether the stated prerequisites are sufficient.

From this skill directory (`py -3` can replace `python3` on Windows):

```sh
python3 scripts/check_dag.py
python3 scripts/test_check_dag.py
```
