#!/usr/bin/env python3
"""Validate tutorials grouped by canonical Lexidecimal height; Python 3.9+."""
import re
import sys
from pathlib import Path

DEFAULT_DIRECTORY = Path(__file__).resolve().parents[3] / "diy"
SLUG = r"[a-z0-9]+(?:-[a-z0-9]+)*"
TUTORIAL = rf"0z[0-9a-f]+/{SLUG}"


def encode_height(height):
    if type(height) is not int or height < 0:
        raise ValueError("height must be a nonnegative integer")
    digits = format(height, "x")
    size = format(len(digits) - 1, "x")
    width = format(len(size) - 1, "x")
    if len(width) != 1:
        raise ValueError("height allocation is too large")
    return f"0z{width}{size}{digits}"


def decode_height(encoded):
    if not re.fullmatch(r"0z[0-9a-f]{3,}", encoded):
        raise ValueError("height must be unsigned Lexidecimal")
    body = encoded[2:]
    end = 2 + int(body[0], 16)
    size, digits = body[1:end], body[end:]
    if not size or not digits or int(size, 16) != len(digits) - 1:
        raise ValueError("incorrect Lexidecimal size")
    height = int(digits, 16)
    if encode_height(height) != encoded:
        raise ValueError("noncanonical Lexidecimal height")
    return height


def prerequisites(text):
    """Only prerequisites: [] or an unquoted block list of diy-relative paths."""
    lines = text.lstrip("\ufeff").replace("\r\n", "\n").split("\n")
    if lines[0] != "---":
        raise ValueError("missing frontmatter")
    try:
        end = lines.index("---", 1)
    except ValueError:
        raise ValueError("unterminated frontmatter") from None
    fields = [line for line in lines[1:end]
              if line.strip() and not line.lstrip().startswith("#")]
    if fields == ["prerequisites: []"]:
        return []
    if not fields or fields[0] != "prerequisites:" or len(fields) == 1:
        raise ValueError("expected prerequisites: [] or prerequisites: with a block list")
    dependencies = []
    for line in fields[1:]:
        item = re.fullmatch(rf"  - ({TUTORIAL})", line)
        if not item:
            raise ValueError("prerequisites must be unquoted <height>/<slug> folder paths relative to diy/")
        dependencies.append(item[1])
    return dependencies


def lint_tutorials(directory):
    directory = Path(directory)
    errors, graph, slugs = [], {}, {}

    def visit(folder, height):
        name = folder.relative_to(directory).as_posix()
        if folder.is_symlink() or not folder.is_dir():
            errors.append(f"{name}: expected a tutorial folder, not a file or symlink")
            return
        if not re.fullmatch(SLUG, folder.name):
            errors.append(f"{name}: expected a <lowercase-slug> folder")
        else:
            slug = folder.name
            if slug in slugs:
                errors.append(f'{name}: duplicate tutorial slug "{slug}" (also {slugs[slug]})')
            else:
                slugs[slug] = name
        # Check the actual spelling even on case-insensitive filesystems.
        readme = next((path for path in folder.iterdir() if path.name == "README.md"), None)
        if readme is None:
            errors.append(f"{name}: missing README.md (case-sensitive)")
            return
        if readme.is_symlink() or not readme.is_file():
            errors.append(f"{name}/README.md: must be a regular file, not a symlink")
            return
        dependencies = []
        try:
            dependencies = prerequisites(readme.read_text(encoding="utf-8"))
        except (ValueError, OSError) as error:
            errors.append(f"{name}: {error}")
        graph[name] = (height, dependencies)

    for folder in sorted(directory.iterdir()):
        if folder.is_symlink():
            errors.append(f"{folder.name}: tutorial height folders must not be symlinks")
            continue
        if not folder.is_dir():
            if folder.suffix.lower() == ".md":
                errors.append(f"{folder.name}: expected <unsigned-Lexidecimal-height>/<lowercase-slug>/README.md")
            continue
        height = None
        try:
            height = decode_height(folder.name)
        except ValueError as error:
            errors.append(f"{folder.name}: {error}")
        for path in sorted(folder.iterdir()):
            if path.is_symlink() or path.is_dir():
                visit(path, height)
            elif path.suffix.lower() == ".md":
                errors.append(f"{folder.name}/{path.name}: expected a tutorial folder containing README.md")
    if not graph:
        errors.append("no tutorials found")

    dependents = {name: [] for name in graph}
    indegree = dict.fromkeys(graph, 0)
    edges = 0
    for name, (height, dependencies) in graph.items():
        seen = set()
        for dependency in dependencies:
            if dependency in seen:
                errors.append(f"{name}: duplicate prerequisite {dependency}")
                continue
            seen.add(dependency)
            if dependency == name:
                errors.append(f"{name}: self prerequisite")
            if dependency not in graph:
                errors.append(f"{name}: missing prerequisite {dependency} (paths are case-sensitive)")
                continue
            edges += 1
            dependents[dependency].append(name)
            indegree[name] += 1
            target_height = graph[dependency][0]
            if height is not None and target_height is not None and height <= target_height:
                errors.append(f"{name}: height {height} must exceed {dependency}'s height {target_height}")
    # Kahn's algorithm detects cycles independently of height validation.
    ready = [name for name, count in indegree.items() if count == 0]
    for dependency in ready:
        for name in dependents[dependency]:
            indegree[name] -= 1
            if indegree[name] == 0:
                ready.append(name)
    blocked = [name for name, count in indegree.items() if count > 0]
    if blocked:
        errors.append(f"prerequisite cycle, or dependent on one: {', '.join(blocked)}")
    return {"errors": errors, "tutorials": len(graph), "edges": edges}


def main(args):
    try:
        if len(args) > 1:
            raise ValueError("usage: check_dag.py [tutorial-directory]")
        result = lint_tutorials(Path(args[0]) if args else DEFAULT_DIRECTORY)
        if result["errors"]:
            print("\n".join(result["errors"]), file=sys.stderr)
            return 1
        print(f"Tutorial DAG OK: {result['tutorials']} tutorials, {result['edges']} prerequisites.")
        return 0
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
