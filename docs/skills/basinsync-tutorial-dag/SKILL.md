---
name: basinsync-tutorial-dag
description: Rename or audit nested basinsync tutorial README folders, Lexidecimal heights, tutorial-slug uniqueness within diy/, and prerequisite graphs.
---

[check_dag]: <scripts/check_dag.py>
[lexidecimal]: <../../glossary/lexidecimal.md>

# Tutorial dependency DAG

Tutorial paths alternate canonical unsigned *Lexidecimal*[^][lexidecimal] height
folders and named folders, ending in `README.md`. Nesting is supported:
`diy/<height>/<unique>/<height>/<distinct>/README.md`. Tutorial slug folder names
must be unique throughout `diy/`; supporting folder names and filenames may repeat.
Each tutorial's height must exceed every direct
prerequisite's height; equal-height branches and height gaps are valid. Use zero
for a new root; height is not a sequence number.

Put prerequisite folder paths relative to `diy/` in frontmatter, before reference definitions:

```yaml
---
prerequisites:
  - 0z000/shallow-epilogue-commits
---
```

Roots use `prerequisites: []`. This deliberately small schema accepts only that
key, with an empty array or an unquoted block list. Use forward slashes on every
platform. List direct requirements; ordinary Markdown references need not be prerequisites.

When changing dependencies, update affected folders, frontmatter, and reference
labels/targets together. Tutorial links use the full
folder path from `docs/` as their reference label, including every nested
height/name pair, with relative destinations ending in `README.md`. Preserve tutorial scope and
allow independent branches.

Run the bundled *Python linter*[^][check_dag]. Python 3.9+ is required, without
third-party packages. It locates `diy/` relative to the skill regardless of working
directory. An optional argument selects another tutorial directory; tutorial-slug
uniqueness is checked only within the selected tutorial tree. It rejects
invalid layouts/heights, repeated tutorial slugs, malformed frontmatter,
missing/duplicate/self prerequisites, cycles, and non-increasing edges. It does
not judge whether the stated prerequisites are sufficient.

From this skill directory (`py -3` can replace `python3` on Windows):

```sh
python3 scripts/check_dag.py
python3 scripts/test_check_dag.py
```
